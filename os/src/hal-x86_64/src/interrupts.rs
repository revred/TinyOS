//! IDT loading, legacy-PIC retirement, and local-APIC timer bring-up
//! (`STORY-P0-04-02`).
//!
//! Everything in this module only means anything on a real x86_64 CPU —
//! `lidt`, `sti`, `rdmsr`/`wrmsr`, port I/O, and raw MMIO writes — so it is
//! gated the same way `boot`/`qemu_exit` already are (see this crate's
//! `lib.rs` doc comment for why that gate is `not(target_os = "windows")`
//! rather than `not(test)`). [`idt`](crate::idt) stays ungated and
//! host-testable precisely so the part that's provably correct on any
//! toolchain (gate-descriptor bit-packing) isn't dragged behind this gate
//! along with the part that isn't.
//!
//! **Design choice this module leans on throughout:** every vector this
//! module does not explicitly service (every CPU exception, every
//! unrouted/spurious source other than the one legitimate spurious vector,
//! every one of the 253 vectors nothing in this kernel uses) is wired
//! straight to [`unhandled_interrupt_handler`] — an ordinary
//! `extern "C" fn() -> !`, not an assembly trampoline. This is safe *only*
//! because that function never returns: the CPU's interrupt-entry frame
//! (and, for the ten vectors that push one, a hardware error code) is left
//! on the stack completely unexamined and unpopped, which would corrupt a
//! resumed caller's state if we ever executed `iretq` — but we never do.
//! [`timer_interrupt_handler`] and [`spurious_interrupt_handler`], the two
//! vectors this kernel *does* resume execution after, are therefore the
//! only two wired through a real save-everything/`iretq` assembly stub
//! (`timer_isr_stub`/`spurious_isr_stub`) below.
//!
//! **Named, not silently solved:** this module stands up no TSS/Interrupt
//! Stack Table, so a `#DF`/`#MC` whose own stack is already invalid can
//! still fault a second time while the CPU pushes that vector's frame —
//! see [`crate::idt::Idt`]'s own doc comment for the full statement of this
//! gap. Local-APIC timer-tick delivery is armed by [`init`] but nothing
//! outside this module consumes a tick yet (no scheduler dispatch loop
//! reads [`tick_count`]) — the same "primitive exists, no production
//! consumer wired to it yet" honesty this codebase has applied to every
//! prior Story's own HAL primitive.

use crate::idt::Idt;
use crate::qemu_exit::{exit_qemu, QemuExitCode};
use core::sync::atomic::{AtomicU32, Ordering};

/// The code-segment selector `boot.rs`'s own GDT installs (its second
/// entry, index 1, RPL 0) — every IDT gate this module builds runs the CPU
/// in that segment.
pub const CODE_SELECTOR: u16 = 0x08;

/// The vector this module wires to the local-APIC timer. Deliberately
/// outside `0x20..0x30` — the legacy PIC's remap target
/// ([`remap_and_mask_pic`]) — even though the PIC is masked immediately
/// after remapping and should never actually deliver anything: keeping the
/// two vector ranges disjoint means a remap/mask bug can never alias a real
/// PIC-sourced interrupt onto this vector's meaning.
pub const TIMER_VECTOR: u8 = 0x30;

/// The local APIC's own spurious-interrupt vector (Intel SDM Vol 3A
/// §11.9). `0xFF`: the conventional choice, and (on some older parts) the
/// only fully compatible one — Intel recommends the low nibble be all-ones.
pub const SPURIOUS_VECTOR: u8 = 0xFF;

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;
const PIC_ICW1_INIT_ICW4: u8 = 0x11;
const PIC_ICW4_8086_MODE: u8 = 0x01;
const PIC1_VECTOR_OFFSET: u8 = 0x20;
const PIC2_VECTOR_OFFSET: u8 = 0x28;
const PIC_UNUSED_PORT: u16 = 0x80;

const IA32_APIC_BASE_MSR: u32 = 0x1B;
const APIC_GLOBAL_ENABLE: u64 = 1 << 11;

/// The local APIC's architectural default MMIO base (Intel SDM Vol 3A
/// §11.4.1). Software *can* relocate this via `IA32_APIC_BASE`'s address
/// field, but QEMU's own APIC device model does not honor that relocation
/// — empirically confirmed during this Story's own bring-up: writing a
/// relocated base and then reading any local-APIC register back (including
/// the read-only APIC ID register, which hardware always answers) returned
/// all-zero, meaning the access was landing on ordinary backing memory
/// rather than being intercepted by the APIC at all. This kernel therefore
/// targets the real, non-relocated default instead — `boot.rs`'s own
/// `boot_pd_gib3` identity-maps the `0xC0000000..0xFFFFFFFF` GiB
/// specifically so this address is reachable at all (it sits well outside
/// `boot.rs`'s original first-1GiB map).
const LAPIC_BASE_PHYS: u64 = 0x0000_0000_FEE0_0000;

const LAPIC_EOI: u32 = 0x0B0;
const LAPIC_SVR: u32 = 0x0F0;
const LAPIC_LVT_TIMER: u32 = 0x320;
const LAPIC_TIMER_INITIAL_COUNT: u32 = 0x380;
const LAPIC_TIMER_DIVIDE_CONFIG: u32 = 0x3E0;
const APIC_SOFTWARE_ENABLE: u32 = 1 << 8;
const TIMER_MODE_PERIODIC: u32 = 1 << 17;
/// Divide the APIC timer's input clock by 16 — a coarse, simple divisor
/// giving a slow-enough count rate that a caller-chosen `initial_count`
/// (see [`init`]) can select a measurable period without needing a huge
/// count value.
const TIMER_DIVIDE_BY_16: u32 = 0x3;

core::arch::global_asm!(
    r#"
    .section .text

    .global timer_isr_stub
timer_isr_stub:
        push rax
        push rcx
        push rdx
        push rbx
        push rbp
        push rsi
        push rdi
        push r8
        push r9
        push r10
        push r11
        push r12
        push r13
        push r14
        push r15

        call timer_interrupt_handler

        pop r15
        pop r14
        pop r13
        pop r12
        pop r11
        pop r10
        pop r9
        pop r8
        pop rdi
        pop rsi
        pop rbp
        pop rbx
        pop rdx
        pop rcx
        pop rax
        iretq

    .global spurious_isr_stub
spurious_isr_stub:
        push rax
        push rcx
        push rdx
        push rbx
        push rbp
        push rsi
        push rdi
        push r8
        push r9
        push r10
        push r11
        push r12
        push r13
        push r14
        push r15

        call spurious_interrupt_handler

        pop r15
        pop r14
        pop r13
        pop r12
        pop r11
        pop r10
        pop r9
        pop r8
        pop rdi
        pop rsi
        pop rbp
        pop rbx
        pop rdx
        pop rcx
        pop rax
        iretq
    "#
);

unsafe extern "C" {
    fn timer_isr_stub();
    fn spurious_isr_stub();
}

/// Every local-APIC timer tick serviced since [`init`], incremented by
/// [`timer_interrupt_handler`] — `pub` so a Tier 0 fixture can observe it
/// without this module exposing its own internal storage.
static TICK_COUNT: AtomicU32 = AtomicU32::new(0);

/// The current value of [`TICK_COUNT`].
pub fn tick_count() -> u32 {
    TICK_COUNT.load(Ordering::SeqCst)
}

/// Services one local-APIC timer interrupt: records the tick, then signals
/// end-of-interrupt so the local APIC delivers the next one.
///
/// # Safety
/// Reached only via `timer_isr_stub`, itself only ever installed as
/// [`TIMER_VECTOR`]'s handler by [`init`] — by the time this can run, the
/// local APIC has already been relocated to [`LAPIC_BASE_PHYS`] and
/// enabled, so that address is live, mapped MMIO for as long as interrupts
/// stay enabled.
#[no_mangle]
extern "C" fn timer_interrupt_handler() {
    TICK_COUNT.fetch_add(1, Ordering::SeqCst);
    // SAFETY: see this function's own doc comment.
    unsafe { lapic_write(LAPIC_EOI, 0) };
}

/// Services the local APIC's own spurious-interrupt vector.
///
/// Per Intel SDM Vol 3A §11.9: a spurious interrupt requires **no** EOI —
/// sending one would incorrectly signal completion of an interrupt that was
/// never actually placed in-service, corrupting the local APIC's own
/// in-service tracking for whatever real interrupt (if any) is concurrently
/// active. Doing nothing here and returning is the documented, correct
/// handling, not an oversight.
#[no_mangle]
extern "C" fn spurious_interrupt_handler() {}

/// The fail-closed default every vector [`init`] does not explicitly
/// service is wired to.
///
/// Never returns — see this module's own doc comment for why that's what
/// makes pointing an IDT gate directly at a bare `extern "C" fn` (no
/// assembly trampoline, no register save, no `iretq`) sound. Reuses
/// `qemu_exit::exit_qemu` — the identical fail-closed action every
/// `panic_handler` in this workspace already takes — rather than inventing
/// a distinct "real hardware" behavior this project has no target board to
/// define one for yet.
#[no_mangle]
extern "C" fn unhandled_interrupt_handler() -> ! {
    exit_qemu(QemuExitCode::Failure)
}

/// Loads `idt` via `lidt`.
///
/// # Safety
/// `idt` must remain at a fixed, valid address (never moved, never
/// deallocated) for as long as it stays the CPU's active IDT — the CPU
/// dereferences the pointer `lidt` was given on every subsequent interrupt,
/// with no further involvement from Rust's own borrow tracking.
unsafe fn load(idt: &Idt) {
    #[repr(C, packed)]
    struct IdtPointer {
        limit: u16,
        base: u64,
    }
    let (base, limit) = idt.pointer();
    let pointer = IdtPointer { limit, base };
    // SAFETY: `pointer` is a validly-laid-out `IdtPointer` on this
    // function's own stack, live for the duration of the `lidt` instruction
    // that reads it; `idt.pointer()`'s own base address is this function's
    // caller's responsibility to keep valid afterward, per this function's
    // doc comment.
    unsafe {
        core::arch::asm!("lidt [{0}]", in(reg) &pointer, options(readonly, nostack, preserves_flags));
    }
}

/// # Safety
/// Port 0x80 is the conventional unused "POST diagnostic" port — writing to
/// it is defined to have no effect beyond consuming a bus cycle, the
/// standard technique for giving the (comparatively slow) 8259 time to
/// process the previous command before the next one.
unsafe fn io_wait() {
    unsafe { outb(PIC_UNUSED_PORT, 0) };
}

/// # Safety
/// `port` must be a port whose device accepts an 8-bit write with no
/// side effect this function's caller hasn't already accounted for.
unsafe fn outb(port: u16, value: u8) {
    // SAFETY: see this function's own doc comment.
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}

/// # Safety
/// `msr` must name a model-specific register this CPU implements.
unsafe fn rdmsr(msr: u32) -> u64 {
    let (high, low): (u32, u32);
    // SAFETY: see this function's own doc comment.
    unsafe {
        core::arch::asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags));
    }
    ((high as u64) << 32) | low as u64
}

/// # Safety
/// `msr` must name a writable model-specific register this CPU implements,
/// and `value` must be one that register accepts — an ill-formed write to
/// `IA32_APIC_BASE` (this module's only caller) can disable interrupt
/// delivery entirely or relocate the local APIC over live memory.
unsafe fn wrmsr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    // SAFETY: see this function's own doc comment.
    unsafe {
        core::arch::asm!("wrmsr", in("ecx") msr, in("eax") low, in("edx") high, options(nomem, nostack, preserves_flags));
    }
}

/// # Safety
/// The local APIC must already be relocated to [`LAPIC_BASE_PHYS`] and
/// enabled (see [`enable_and_relocate_local_apic`]), and `offset` must be a
/// valid 4-byte-aligned local-APIC register offset.
unsafe fn lapic_write(offset: u32, value: u32) {
    let ptr = (LAPIC_BASE_PHYS + offset as u64) as *mut u32;
    // SAFETY: see this function's own doc comment; `boot.rs`'s
    // `boot_pd_gib3` identity map covers `LAPIC_BASE_PHYS` with a
    // read/write huge page.
    unsafe { core::ptr::write_volatile(ptr, value) };
}

/// Remaps the legacy 8259 PIC's two controllers off the CPU-exception
/// vector range (their power-on-default IRQ0-7/8-15 mapping collides with
/// vectors 0x08-0x0F, the middle of the CPU's own exception range) and then
/// masks every line on both controllers.
///
/// This kernel drives interrupts entirely through the local APIC — the PIC
/// is never meant to deliver anything — but an *unmapped* (never remapped)
/// PIC left enabled would still be capable of asserting IRQ0 (the legacy
/// timer) or IRQ1 (keyboard) at its power-on-default vectors 0x08/0x09,
/// landing squarely on `#DF`/`#NX` in the CPU's exception range if it ever
/// fired — remap-then-mask is the standard, required sequence to retire it
/// safely, not an optional cleanup step.
///
/// Public because a Tier 0 *measurement* fixture needs exactly this step and
/// nothing else from this module (`STORY-P1-01-01`): its measured code
/// switches into task contexts whose initial `rflags` has `IF` set (see
/// `kernel::context::Context::new`), so a legacy IRQ0 arriving mid-measurement
/// would either perturb the sample or — on a fixture boot path with no IDT
/// loaded at all — fault. Calling [`init`] instead would quiesce the PIC but
/// also arm the local-APIC timer, injecting ticks into the very region being
/// measured; a measurement fixture needs the quiescing without the ticking.
///
/// # Safety
/// Must run before [`init`] enables interrupts (`sti`) — remapping while
/// interrupts are live could let a stale in-flight PIC interrupt land at
/// whichever vector the remap sequence has reached at that instant. A caller
/// that never enables interrupts at all (a measurement fixture) satisfies this
/// trivially.
pub unsafe fn remap_and_mask_pic() {
    // SAFETY: this is the documented 8259 initialization sequence (ICW1
    // through ICW4) applied to both the master and slave controller, per
    // this function's own doc comment.
    unsafe {
        outb(PIC1_CMD, PIC_ICW1_INIT_ICW4);
        io_wait();
        outb(PIC2_CMD, PIC_ICW1_INIT_ICW4);
        io_wait();
        outb(PIC1_DATA, PIC1_VECTOR_OFFSET);
        io_wait();
        outb(PIC2_DATA, PIC2_VECTOR_OFFSET);
        io_wait();
        outb(PIC1_DATA, 0x04); // ICW3 (master): a slave PIC lives on IRQ2
        io_wait();
        outb(PIC2_DATA, 0x02); // ICW3 (slave): this PIC's own cascade identity
        io_wait();
        outb(PIC1_DATA, PIC_ICW4_8086_MODE);
        io_wait();
        outb(PIC2_DATA, PIC_ICW4_8086_MODE);
        io_wait();

        outb(PIC1_DATA, 0xFF); // mask every line — the PIC never fires again
        outb(PIC2_DATA, 0xFF);
    }
}

/// Sets the global enable bit in `IA32_APIC_BASE` (leaving its address
/// field untouched — [`LAPIC_BASE_PHYS`]'s own doc comment states why this
/// module targets the hardware default rather than relocating), then
/// software-enables the local APIC and arms [`SPURIOUS_VECTOR`] via the
/// Spurious Interrupt Vector Register — both enable bits (`IA32_APIC_BASE`'s
/// global enable and the SVR's software enable) are required; either alone
/// leaves the local APIC inert.
///
/// # Safety
/// Must run before [`init`]'s `sti` and before [`configure_timer`], and
/// [`LAPIC_BASE_PHYS`] must already be mapped read/write (`boot.rs`'s
/// `boot_pd_gib3`).
unsafe fn enable_and_relocate_local_apic() {
    // SAFETY: see this function's own doc comment.
    unsafe {
        let base = rdmsr(IA32_APIC_BASE_MSR) | APIC_GLOBAL_ENABLE;
        wrmsr(IA32_APIC_BASE_MSR, base);

        lapic_write(LAPIC_SVR, APIC_SOFTWARE_ENABLE | SPURIOUS_VECTOR as u32);
    }
}

/// Programs the local APIC timer for periodic delivery on [`TIMER_VECTOR`],
/// dividing its input clock by 16 and reloading from `initial_count` each
/// period — the caller-chosen knob [`init`] exposes for how coarse or fine
/// a tick this kernel wants, deliberately not hardcoded here so a Tier 0
/// fixture measuring the resulting interval can choose a count matched to
/// its own bounded test budget.
///
/// # Safety
/// The local APIC must already be relocated and enabled (see
/// [`enable_and_relocate_local_apic`]).
unsafe fn configure_timer(initial_count: u32) {
    // SAFETY: see this function's own doc comment.
    unsafe {
        lapic_write(LAPIC_TIMER_DIVIDE_CONFIG, TIMER_DIVIDE_BY_16);
        lapic_write(LAPIC_LVT_TIMER, TIMER_MODE_PERIODIC | TIMER_VECTOR as u32);
        lapic_write(LAPIC_TIMER_INITIAL_COUNT, initial_count);
    }
}

/// This kernel's one, module-owned IDT — `static mut` (rather than
/// stack-local) because it must outlive `init` itself: the CPU keeps
/// dereferencing whatever `lidt` last pointed it at on every subsequent
/// interrupt for the rest of this kernel's run, exactly as
/// `exec::address_space::AddressSpace`'s own callers must keep its backing
/// storage alive for as long as a loaded page-table tree stays active.
static mut IDT: Idt = Idt::new();

/// Brings interrupt handling up end to end: builds this module's [`Idt`]
/// (every vector defaulting to [`unhandled_interrupt_handler`],
/// [`TIMER_VECTOR`] and [`SPURIOUS_VECTOR`] wired to their own stubs),
/// loads it, retires the legacy PIC, brings up the local APIC, arms its
/// timer at `initial_count`, and enables interrupts (`sti`).
///
/// `STORY-P0-04-02` acceptance criterion 2 is enforced structurally before
/// `load` ever runs: `debug_assert!(idt.every_entry_present())` — every one
/// of the 256 vectors is present, so no interrupt this kernel could ever
/// receive reaches an unpopulated gate.
///
/// # Safety
/// Must be called at most once, before any other code in this kernel
/// depends on interrupts being masked, and only on the real (or QEMU
/// `q35`-emulated) local APIC/PIC hardware this module's register-offset
/// and MSR constants assume.
#[allow(static_mut_refs, clippy::deref_addrof)]
pub unsafe fn init(initial_count: u32) {
    // SAFETY: see this function's own doc comment; `&raw mut IDT` mirrors
    // this workspace's own established pattern for a `'static mut` a single
    // caller owns for a whole kernel run (e.g.
    // `exec::fixture_shared_memory_main`'s `GRANT_REGISTRY`).
    unsafe {
        let idt = &mut *(&raw mut IDT);
        let default_handler = unhandled_interrupt_handler as *const () as u64;
        for vector in 0..=255u8 {
            idt.set_handler(vector, default_handler, CODE_SELECTOR);
        }
        idt.set_handler(TIMER_VECTOR, timer_isr_stub as *const () as u64, CODE_SELECTOR);
        idt.set_handler(SPURIOUS_VECTOR, spurious_isr_stub as *const () as u64, CODE_SELECTOR);
        debug_assert!(idt.every_entry_present());

        load(idt);
        remap_and_mask_pic();
        enable_and_relocate_local_apic();
        configure_timer(initial_count);
        core::arch::asm!("sti", options(nomem, nostack, preserves_flags));
    }
}
