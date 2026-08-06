//! Exception capture and the bounded fault report (`STORY-P1-07-02`).
//!
//! Split exactly the way [`crate::boot`] is, and for the same reason: the part
//! that decides *what the board says* is pure, generic over the
//! [`Mmio`](crate::pl011::Mmio) seam and host-tested against a double that
//! accumulates the wire; the part that reads system registers and installs
//! `VBAR_EL1` is assembly, compiled only for AArch64.
//!
//! **No registers are saved, nothing is restored, and the entry path touches
//! no memory at all.** The first two are the decision `hal_x86_64::fault`'s
//! stubs make, for the same reason: there is no resume path in this Story, so
//! the general-purpose registers are dead the moment the fault is taken.
//!
//! The third is an AArch64 improvement on that precedent, and it is a *safety*
//! one. An entry that built a frame on the stack would have to write to the
//! stack — while one of the things that can put a system here is a stack that
//! is no longer valid. AArch64 has enough argument registers to avoid the
//! question entirely: each entry loads the slot index and the four describing
//! registers into `x0`-`x4` and branches — not calls — into
//! [`tinyos_arm64_exception_entry`], which takes them as ordinary `extern "C"`
//! arguments. No store, no `sub sp`, no `#[repr(C)]` layout invariant between
//! assembly and Rust, and no raw pointer to dereference.
//!
//! **The frame is evidence, never authority** (`TEST-P1-07-02-A` clause 4,
//! `PD-12`, `BND-04`-shaped). `ESR_EL1`, `FAR_EL1`, `ELR_EL1` and `SPSR_EL1`
//! all describe execution that has just violated an invariant. They are
//! decoded, printed, and consulted by nothing. The disposition on this board is
//! terminal for every slot and every syndrome, and the host tests below pin
//! that it does not vary with anything the frame carries — including through
//! `kernel::fault::Disposition`, the x86_64 policy, run unmodified against an
//! AArch64 frame.
//!
//! **Nothing here has executed.** `STORY-P1-07-02`'s clause 2 needs a board and
//! a deliberately-triggered fault, and there is no version of this Story that
//! passes without one.

use crate::esr::Esr;
use crate::pl011::{hex_u64, Mmio, Pl011, Pl011Error};
use crate::vectors::{VectorSlot, ENTRY_COUNT};

/// Every line a fault report emits carries this prefix, so a capture can be
/// separated from the boot report (`TOS64-BOOT/1`) and from the firmware's own
/// console output on the same wire.
pub const TAG: &str = "TOS64-FAULT/1 ";

/// One captured exception.
///
/// An ordinary Rust struct: it crosses no ABI boundary, because the vector
/// entries hand [`tinyos_arm64_exception_entry`] five registers rather than a
/// pointer to a stack frame (see this module's documentation). What that
/// removes is a layout invariant no compiler checks —
/// `hal_x86_64::fault::FaultFrame` has to pin `size_of` and every field offset
/// against its stubs' push order, and this one has nothing to pin.
///
/// What remains is the *argument order*, which is the assembly's register
/// choice restated in Rust:
///
/// | register | field | source |
/// |---|---|---|
/// | `x0` | `slot` | the vector entry's own index, an immediate |
/// | `x1` | `esr` | `mrs x1, esr_el1` |
/// | `x2` | `far` | `mrs x2, far_el1` |
/// | `x3` | `elr` | `mrs x3, elr_el1` |
/// | `x4` | `spsr` | `mrs x4, spsr_el1` |
///
/// [`FaultFrame::from_entry`] is where that order is written down once, and the
/// host tests pin it there — a transposition of two `u64` arguments is
/// invisible to the type system and produces a report that is entirely
/// plausible and entirely wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultFrame {
    /// Which of the sixteen vector slots the CPU branched to.
    pub slot: u64,
    /// `ESR_EL1` as read on entry.
    pub esr: u64,
    /// `FAR_EL1` as read on entry — meaningful only for the classes that
    /// update it; see [`FaultFrame::faulting_address`].
    pub far: u64,
    /// `ELR_EL1`: the address execution would return to, i.e. the faulting
    /// instruction for a synchronous abort.
    pub elr: u64,
    /// `SPSR_EL1`: the saved processor state.
    pub spsr: u64,
}

impl FaultFrame {
    /// Assembles a frame from the five values a vector entry supplies, in the
    /// register order it supplies them.
    ///
    /// The single place the assembly's `x0`-`x4` choice becomes Rust field
    /// names. [`tinyos_arm64_exception_entry`] does nothing but forward its
    /// arguments here, so this function — and its host test — is the whole
    /// defence against a transposition.
    pub const fn from_entry(slot: u64, esr: u64, far: u64, elr: u64, spsr: u64) -> FaultFrame {
        FaultFrame { slot, esr, far, elr, spsr }
    }

    /// The captured `ESR_EL1`, wrapped for decoding.
    pub const fn esr(&self) -> Esr {
        Esr::new(self.esr)
    }

    /// Which vector slot fired, or `None` for an index outside `0..16`.
    ///
    /// `None` means the table and this code disagree about the slot numbering,
    /// which is a fault-handling failure rather than a fall-through — the same
    /// judgement `hal_x86_64::fault::FaultVector::from_raw` makes about a
    /// vector it never wired.
    pub const fn slot(&self) -> Option<VectorSlot> {
        if self.slot >= ENTRY_COUNT as u64 {
            return None;
        }
        VectorSlot::from_index(self.slot as usize)
    }

    /// The faulting address, or `None` when `FAR_EL1` does not hold one.
    ///
    /// `FAR_EL1` is a *register*, not a value pushed with the frame: it holds
    /// whatever the last exception that updated it left there. Reporting it for
    /// a class that does not update it reports a stale address from an
    /// unrelated earlier event with total confidence — the mistake
    /// `hal_x86_64::fault::FaultFrame::faulting_address` refuses to make about
    /// `CR2`, restated here because a second architecture is where an invariant
    /// like that either holds or turns out to have been arch-shaped.
    pub const fn faulting_address(&self) -> Option<u64> {
        if self.esr().far_is_meaningful() {
            return Some(self.far);
        }
        None
    }
}

/// Writes the fault report: which slot, the decoded `ESR_EL1`, the
/// class-specific `ISS`, the addresses, and that the system stopped.
///
/// Bounded and allocation-free (`TEST-P1-07-02-A` clause 5). Every loop inside
/// it is over a fixed-length array; the only wait is
/// [`Pl011`]'s own bounded poll, and a wedged UART makes this return an error
/// rather than stall — an unbounded retry inside a fault handler is the hang
/// this Story exists to eliminate, wearing a different hat.
pub fn report<M: Mmio>(uart: &Pl011<M>, frame: &FaultFrame) -> Result<(), Pl011Error> {
    let esr = frame.esr();

    // 1. Which of the sixteen vectors the CPU branched to. First, because it is
    //    the one field that is true even if every decode below is wrong.
    uart.write_str(TAG)?;
    uart.write_str("slot=")?;
    match frame.slot() {
        Some(slot) => {
            uart.write_str(slot.name())?;
            uart.write_str(" index=")?;
            uart.write_bytes(&hex_u8(slot.index() as u8))?;
        }
        None => {
            uart.write_str("unknown index=")?;
            write_hex(uart, frame.slot)?;
        }
    }
    uart.write_str("\n")?;

    // 2. The raw register, then the decode. In that order deliberately: the
    //    register is the evidence and the decode is a reading of it.
    uart.write_str(TAG)?;
    uart.write_str("esr=")?;
    write_hex(uart, esr.raw())?;
    uart.write_str(" class=")?;
    uart.write_str(esr.class().as_str())?;
    uart.write_str(" ec=")?;
    uart.write_bytes(&hex_u8(esr.ec()))?;
    uart.write_str(if esr.instruction_length_is_32_bit() { " il=32\n" } else { " il=16\n" })?;

    // 3. The class-specific syndrome — or the raw `ISS` for a class whose
    //    syndrome this Story does not decode. Never a data abort's field names
    //    over another class's bits.
    uart.write_str(TAG)?;
    if let Some(abort) = esr.data_abort() {
        uart.write_str("status=")?;
        uart.write_str(abort.status.as_str())?;
        write_level(uart, abort.status.level())?;
        uart.write_str(if abort.write { " wnr=write" } else { " wnr=read" })?;
        match abort.access_size_bytes() {
            Some(bytes) => {
                uart.write_str(" isv=yes size=")?;
                uart.write_bytes(&[b'0' + bytes])?;
            }
            None => uart.write_str(" isv=no size=unknown")?,
        }
        uart.write_str(if abort.stage1_table_walk { " s1ptw=yes" } else { " s1ptw=no" })?;
    } else if let Some(abort) = esr.instruction_abort() {
        uart.write_str("status=")?;
        uart.write_str(abort.status.as_str())?;
        write_level(uart, abort.status.level())?;
        uart.write_str(if abort.stage1_table_walk { " s1ptw=yes" } else { " s1ptw=no" })?;
    } else {
        // Not "no syndrome": a `BRK`'s comment lives in this field, and
        // dropping it would lose the one thing that tells two deliberate
        // faults apart.
        uart.write_str("iss=")?;
        uart.write_bytes(&hex_u64(esr.iss() as u64)[8..])?;
    }
    uart.write_str("\n")?;

    // 4. The addresses. `far=invalid` rather than a number whenever the class
    //    does not update `FAR_EL1` — see `FaultFrame::faulting_address`.
    uart.write_str(TAG)?;
    uart.write_str("far=")?;
    match frame.faulting_address() {
        Some(address) => write_hex(uart, address)?,
        None => uart.write_str("invalid")?,
    }
    uart.write_str(" elr=")?;
    write_hex(uart, frame.elr)?;
    uart.write_str(" spsr=")?;
    write_hex(uart, frame.spsr)?;
    uart.write_str("\n")?;

    // 5. That the system stopped, so no reader infers a resume that does not
    //    exist (`TEST-P1-07-02-A` clause 5).
    uart.write_str(TAG)?;
    uart.write_str("halted reason=no-resume-path\n")
}

/// Writes what was requested of `VBAR_EL1` and what the register read back.
///
/// A misaligned `VBAR_EL1` write is architecturally ignored — no fault, no
/// error, and the handler simply never runs. The alignment itself is asserted
/// at build time (see [`crate::vectors`]), so this is not the defence; it is
/// the *second* one, and it costs three words on a wire that would otherwise
/// have to be trusted. On a bring-up, a claim that can be read back should be.
pub fn report_vbar<M: Mmio>(
    uart: &Pl011<M>,
    requested: u64,
    readback: u64,
) -> Result<(), Pl011Error> {
    uart.write_str(TAG)?;
    uart.write_str("vbar=")?;
    write_hex(uart, requested)?;
    uart.write_str(" readback=")?;
    write_hex(uart, readback)?;
    uart.write_str(if requested == readback { " match=yes\n" } else { " match=no\n" })
}

/// Writes a `u64` as sixteen hex digits.
fn write_hex<M: Mmio>(uart: &Pl011<M>, value: u64) -> Result<(), Pl011Error> {
    uart.write_bytes(&hex_u64(value))
}

/// Capacity of the canvas fault transcript — five report lines with room to
/// spare; overflow is truncated, never wrapped.
pub const TRANSCRIPT_CAPACITY: usize = 512;

/// An always-ready [`Mmio`] sink that accumulates data-register writes.
///
/// The bridge that lets [`report`] paint the canvas without a second
/// formatter: the same generic code that drives the PL011 drives this, so
/// the pixels and the wire can never disagree (`STORY-P1-07-03` clause 6's
/// evidence rides the screen — serial has never produced a byte on this
/// bench, `LE-47`).
pub struct TranscriptSink {
    state: core::cell::RefCell<([u8; TRANSCRIPT_CAPACITY], usize)>,
}

impl TranscriptSink {
    /// An empty transcript.
    #[must_use]
    pub const fn new() -> Self {
        TranscriptSink { state: core::cell::RefCell::new(([0; TRANSCRIPT_CAPACITY], 0)) }
    }

    /// Calls `visit` once per completed line, in order, with the line's row
    /// number and its bytes — line endings stripped, because the canvas has
    /// rows instead of a framer.
    pub fn for_each_line<F: FnMut(usize, &[u8])>(&self, mut visit: F) {
        let state = self.state.borrow();
        let (buffer, len) = (&state.0, state.1);
        let mut row = 0;
        for raw in buffer[..len].split(|&byte| byte == b'\n') {
            let line = match raw.split_last() {
                Some((b'\r', head)) => head,
                _ => raw,
            };
            if line.is_empty() {
                continue;
            }
            visit(row, line);
            row += 1;
        }
    }
}

impl Default for TranscriptSink {
    fn default() -> Self {
        TranscriptSink::new()
    }
}

impl Mmio for TranscriptSink {
    fn read_u32(&self, _offset: usize) -> u32 {
        // Always ready: no flag is ever set, so the driver's bounded poll
        // passes immediately and the sink can never wedge a fault report.
        0
    }

    fn write_u32(&self, offset: usize, value: u32) {
        if offset != crate::pl011::register::DR {
            return;
        }
        let mut state = self.state.borrow_mut();
        let len = state.1;
        if len < TRANSCRIPT_CAPACITY {
            state.0[len] = value as u8;
            state.1 = len + 1;
        }
    }
}

/// Writes ` level=N`, or ` level=none` for a status that names no translation
/// level.
///
/// `none` rather than an omitted field: a reader diffing two captures should
/// see the field either way, and a missing field reads as a truncated line.
fn write_level<M: Mmio>(uart: &Pl011<M>, level: Option<u8>) -> Result<(), Pl011Error> {
    uart.write_str(" level=")?;
    match level {
        Some(level) => uart.write_bytes(&[b'0' + level]),
        None => uart.write_str("none"),
    }
}

/// Renders a byte as two uppercase hex digits.
const fn hex_u8(value: u8) -> [u8; 2] {
    const DIGITS: &[u8; 16] = b"0123456789ABCDEF";
    [DIGITS[(value >> 4) as usize], DIGITS[(value & 0xF) as usize]]
}

// The exception vector table. (A `//` comment, not a doc comment: rustdoc does
// not document macro invocations, and `global_asm!` is one.)
//
// Sixteen entries at a 128-byte stride, 2 KiB-aligned. Both properties are
// asserted **at assembly time**, which is what `TEST-P1-07-02-A` clause 1 asks
// for and is the only place they can be asserted at all:
//
// - `.balign 0x800` gives the base the alignment `VBAR_EL1` requires. A
//   misaligned base is architecturally *ignored* rather than rejected, so the
//   symptom would be that no handler ever runs — the exact silence this Story
//   exists to eliminate.
// - `.org tinyos_vector_table + 0x80 * \index` places each entry at its
//   architectural offset. If an entry's code overflows its 128 bytes, the next
//   `.org` would have to move backwards, and the assembler refuses — so an
//   over-long entry is a build failure, not a table whose second half is
//   silently displaced by one slot.
//
// Each entry does the least that is possible: put its own index in `x0`, read
// the four registers that describe the exception into `x1`-`x4`, and **branch**
// — not call — into the Rust entry point, whose `extern "C"` signature takes
// them in exactly that order.
//
// Six instructions, no memory access, no stack adjustment. Nothing is saved
// because nothing is restored (clause 5: there is no resume path), and nothing
// is stored because the stack is one of the things that can be the reason
// execution arrived here at all.
//
// Unverified. Never executed; see this module's documentation.
#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(
    ".section .text.vectors",
    ".balign 0x800",
    ".global tinyos_vector_table",
    "tinyos_vector_table:",
    ".macro TINYOS_VECTOR_ENTRY index",
    ".org tinyos_vector_table + 0x80 * \\index",
    "    mov  x0, #\\index",
    "    mrs  x1, esr_el1",
    "    mrs  x2, far_el1",
    "    mrs  x3, elr_el1",
    "    mrs  x4, spsr_el1",
    "    b    {entry}",
    ".endm",
    "TINYOS_VECTOR_ENTRY 0",
    "TINYOS_VECTOR_ENTRY 1",
    "TINYOS_VECTOR_ENTRY 2",
    "TINYOS_VECTOR_ENTRY 3",
    "TINYOS_VECTOR_ENTRY 4",
    // Slot 5 — cur_el_spx/irq, `STORY-P1-07-04`'s tick — is the ONE entry
    // with a resume path, so it is the one entry that saves state: every
    // AAPCS caller-saved register plus the link register, on the stack the
    // interrupt arrived on. `ELR_EL1`/`SPSR_EL1` need no save because the
    // handler is not re-entered: `PSTATE.I` is set on exception entry and
    // stays set until the `eret`. 24 instructions, inside the 32 the stride
    // allows — the `.org` for slot 6 is the assembler-enforced proof.
    ".org tinyos_vector_table + 0x80 * 5",
    "    sub  sp, sp, #0xA0",
    "    stp  x0, x1, [sp, #0x00]",
    "    stp  x2, x3, [sp, #0x10]",
    "    stp  x4, x5, [sp, #0x20]",
    "    stp  x6, x7, [sp, #0x30]",
    "    stp  x8, x9, [sp, #0x40]",
    "    stp  x10, x11, [sp, #0x50]",
    "    stp  x12, x13, [sp, #0x60]",
    "    stp  x14, x15, [sp, #0x70]",
    "    stp  x16, x17, [sp, #0x80]",
    "    stp  x18, x30, [sp, #0x90]",
    "    bl   {irq}",
    "    ldp  x0, x1, [sp, #0x00]",
    "    ldp  x2, x3, [sp, #0x10]",
    "    ldp  x4, x5, [sp, #0x20]",
    "    ldp  x6, x7, [sp, #0x30]",
    "    ldp  x8, x9, [sp, #0x40]",
    "    ldp  x10, x11, [sp, #0x50]",
    "    ldp  x12, x13, [sp, #0x60]",
    "    ldp  x14, x15, [sp, #0x70]",
    "    ldp  x16, x17, [sp, #0x80]",
    "    ldp  x18, x30, [sp, #0x90]",
    "    add  sp, sp, #0xA0",
    "    eret",
    "TINYOS_VECTOR_ENTRY 6",
    "TINYOS_VECTOR_ENTRY 7",
    "TINYOS_VECTOR_ENTRY 8",
    "TINYOS_VECTOR_ENTRY 9",
    "TINYOS_VECTOR_ENTRY 10",
    "TINYOS_VECTOR_ENTRY 11",
    "TINYOS_VECTOR_ENTRY 12",
    "TINYOS_VECTOR_ENTRY 13",
    "TINYOS_VECTOR_ENTRY 14",
    "TINYOS_VECTOR_ENTRY 15",
    // The sixteenth entry's own 128 bytes, so the table is a full 2 KiB and
    // nothing else in this section can be placed inside it.
    ".org tinyos_vector_table + 0x800",
    entry = sym tinyos_arm64_exception_entry,
    irq = sym tinyos_arm64_irq_entry,
);

/// Where slot 5 — and only slot 5 — lands: the tick
/// ([`crate::vectors::Routing::Tick`], `STORY-P1-07-04`).
///
/// Bounded and allocation-free (`SEC-20`): one `GICC_IAR` read, at most one
/// interval record + one `TVAL` write + one `EOIR` write, no loops. A claim
/// of [`crate::gic::SPURIOUS_INTID`] is returned from without an `EOIR`, per
/// the GIC architecture. An unexpected INTID is retired and counted, never
/// fatal — the count reaches the report through the tick state.
///
/// **Unverified.** Never executed; see this module's documentation.
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn tinyos_arm64_irq_entry() {
    // SAFETY: `GICC_BASE` is the BCM2712 GIC-400 CPU-interface window
    // transcribed in `crate::board`; single core, and the fault path cannot
    // preempt this (both run with `PSTATE.I` set).
    let gicc = unsafe { crate::pl011::VolatileMmio::new(crate::board::GICC_BASE) };
    let claimed = crate::gic::acknowledge(&gicc);
    let intid = claimed & 0x3FF;
    if intid == crate::gic::SPURIOUS_INTID {
        return;
    }
    if intid == crate::gic::VIRTUAL_TIMER_INTID {
        use crate::timer::VirtualCounter;
        crate::tick::record_tick(crate::timer::SystemRegisters.count());
        crate::timer::rearm_virtual_timer(crate::tick::TICK_INTERVAL_TICKS);
    } else {
        crate::tick::record_unexpected();
    }
    crate::gic::end_of_interrupt(&gicc, claimed);
}

/// The measurement fixture's escape hook (`STORY-P1-07-06`): a synchronous
/// `EL1h` fault taken while the hook is installed is handed to the fixture,
/// which records its sample and context-switches away — the same
/// escape-switch pattern the x86_64 `fixture_measure` uses to survive a
/// deliberate fault. Zero means "no hook"; anything else is the handler's
/// address.
#[cfg(all(target_arch = "aarch64", feature = "fixture-measure"))]
static MEASURE_FAULT_HOOK: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

/// Installs (or, with `None`, removes) the fixture's fault hook. Fixture
/// only, single core; the production fault path is unreachable while a hook
/// is installed **for synchronous `EL1h` faults only** — every other slot
/// still reports and parks.
#[cfg(all(target_arch = "aarch64", feature = "fixture-measure"))]
pub fn install_measure_hook(hook: Option<extern "C" fn() -> !>) {
    MEASURE_FAULT_HOOK.store(hook.map_or(0, |f| f as usize), core::sync::atomic::Ordering::Release);
}

/// Where every one of the sixteen vector entries lands.
///
/// Takes the slot index and the four registers that describe the exception, in
/// the order the entries load them into `x0`-`x4`, reports them, and parks. It
/// does not return, and there is no path by which it could:
/// [`Routing::Decoded`](crate::vectors::Routing::Decoded) and
/// [`Routing::FailClosedDefault`](crate::vectors::Routing::FailClosedDefault)
/// differ in what the *report* says, never in whether execution continues.
/// (The one exception is the measurement fixture's installed escape hook,
/// which never returns here either — see [`install_measure_hook`].)
///
/// **Non-reentrant by construction** (`TEST-P1-07-02-A` clause 5). `PSTATE.DAIF`
/// is set by the architecture on exception entry and this function never clears
/// it, so no interrupt can arrive mid-report. A *synchronous* fault raised
/// inside this function is **not survivable and is not claimed to be**: it would
/// re-enter at the same slot with `ELR_EL1` and `SPSR_EL1` already overwritten,
/// and there is no AArch64 counterpart of `STORY-P1-02-02`'s IST work in this
/// Feature.
///
/// It is a **safe** function, and that is not an accident: taking five `u64`s
/// rather than a pointer to a stack frame means there is nothing here to
/// dereference. Target-only clippy is what surfaced the alternative — the
/// pointer-taking version this replaced tripped
/// `clippy::not_unsafe_ptr_arg_deref`, which the host lint job cannot see
/// (`LE-12`).
///
/// **Unverified.** Never executed; see this module's documentation.
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn tinyos_arm64_exception_entry(
    slot: u64,
    esr: u64,
    far: u64,
    elr: u64,
    spsr: u64,
) -> ! {
    // The fixture's escape hatch, consulted first and only for the slot the
    // fixture deliberately faults through — see `install_measure_hook`.
    #[cfg(feature = "fixture-measure")]
    if slot == crate::vectors::VectorSlot::SYNCHRONOUS_EL1H.index() as u64 {
        let hook = MEASURE_FAULT_HOOK.load(core::sync::atomic::Ordering::Acquire);
        if hook != 0 {
            // SAFETY: the value was stored from a real
            // `extern "C" fn() -> !` by `install_measure_hook` and can only
            // be that or zero; single core, so no tear.
            let hook: extern "C" fn() -> ! = unsafe { core::mem::transmute(hook) };
            hook();
        }
    }

    let frame = FaultFrame::from_entry(slot, esr, far, elr, spsr);

    // SAFETY: `DEBUG_UART_BASE` is the BCM2712 `uart10` window transcribed in
    // `crate::board`, and cores 1-3 are parked in `_start`, so nothing else on
    // this machine is programming it.
    let uart =
        Pl011::new(unsafe { crate::pl011::VolatileMmio::new(crate::board::DEBUG_UART_BASE) });

    // Deliberately **not** reconfigured. `Pl011::configure` disables the device
    // and drains it first, which on a UART wedged by whatever caused this fault
    // is a bounded wait this path does not need to take — and if the fault
    // happened before `crate::boot` configured the UART, there was never a
    // channel to lose. Report on whatever device state exists.
    let _ = report(&uart, &frame);

    // The same report, painted (`STORY-P1-07-03`): the serial line has never
    // produced a byte on this bench (`LE-47`), and the canvas is the proven
    // text channel. Rendered through the identical generic path via
    // [`TranscriptSink`], so screen and wire cannot disagree.
    //
    // `LE-98`: this is the ONE canvas in the tree painted without display
    // evidence, and the exception is named at its constructor rather than
    // assumed here. The park loop refuses to paint when the firmware reports
    // no display; a fault report cannot, because it may run before the splash
    // has asked and because serial has never produced a byte on this bench
    // (`LE-47`) — a fault that paints nothing is a board that hangs with no
    // symptom. The trade is argued in `Canvas::last_resort_for_fault_report`.
    let sink = TranscriptSink::new();
    let sink_uart = Pl011::new(&sink);
    let _ = report(&sink_uart, &frame);
    let mut console = crate::canvas::Canvas::last_resort_for_fault_report();
    crate::canvas::draw_frame(&mut console);
    sink.for_each_line(|row, line| {
        crate::canvas::draw_line(
            &mut console,
            crate::canvas::REPORT_Y + row as u32 * 40,
            line,
            crate::canvas::ALERT,
        );
    });

    park()
}

/// Installs the vector table and reads `VBAR_EL1` back.
///
/// Returns `(requested, readback)` for [`report_vbar`] rather than reporting
/// itself, so the decision about *what the board says* stays with the pure,
/// host-tested half of this module.
///
/// **Unverified.** Never executed; see this module's documentation.
///
/// # Safety
///
/// Must be called once, on the boot core, at `EL1`. Installing a vector table
/// is a system-wide change: from the instruction after the `isb`, every
/// exception on this core lands in [`tinyos_arm64_exception_entry`] on
/// whatever stack is then current.
#[cfg(target_arch = "aarch64")]
pub unsafe fn install() -> (u64, u64) {
    let requested: u64;
    let readback: u64;
    // SAFETY: the caller established `EL1` and a stack. `VBAR_EL1` is writable
    // at `EL1`, the address written is this crate's own 2 KiB-aligned table
    // (see the `.balign` above), and the `isb` orders the write before any
    // subsequent exception can be taken.
    unsafe {
        core::arch::asm!(
            "adrp {requested}, tinyos_vector_table",
            "add  {requested}, {requested}, :lo12:tinyos_vector_table",
            "msr  vbar_el1, {requested}",
            "isb",
            "mrs  {readback}, vbar_el1",
            requested = out(reg) requested,
            readback = out(reg) readback,
            options(nostack, preserves_flags),
        );
    }
    (requested, readback)
}

/// Raises a `BRK` — the deliberate synchronous exception `TEST-P1-07-02-A`
/// clause 2 requires.
///
/// `BRK` rather than a wild pointer dereference on purpose: it is
/// architecturally guaranteed to trap to `EL1` synchronously, it writes nothing
/// and corrupts nothing, and its `ESR_EL1` carries a comment field so two
/// deliberate faults are distinguishable in a capture. Nothing in this crate
/// calls it — the fixture that does is an image, and building one is
/// `STORY-P1-07-05`.
///
/// **Unverified, and this is the Story's Green.** Clause 2 has no version that
/// passes without a board: a claim that failure is visible, tested only against
/// code that does not fail, is not a test.
///
/// # Safety
///
/// Must be called after [`install`], or the exception has no handler and the
/// board goes silent — which is the state this Story exists to end, reached
/// through the Story's own front door.
#[cfg(target_arch = "aarch64")]
pub unsafe fn deliberate_breakpoint() -> ! {
    // SAFETY: `BRK` at `EL1` with a vector table installed takes a synchronous
    // exception to `EL1`. It touches no memory and no register.
    unsafe { core::arch::asm!("brk #0", options(nomem, nostack, preserves_flags)) };
    park()
}

/// Raises an alignment fault against `address` — the other deliberate fault
/// clause 2 can use, and the one that exercises the data-abort decode path.
///
/// With `SCTLR_EL1.M` clear every access is Device-nGnRnE, and an unaligned
/// access to Device memory takes an **alignment fault** rather than merely
/// running slowly (`session/hand-2026-07-28/23-bcm2712-divergence-record.md`
/// §5). That made this the representative fault while the MMU was off.
/// **Since `STORY-P1-07-03` the premise is narrower**: RAM is Normal memory
/// and unaligned accesses to it simply complete, so this fault now requires
/// an address in a *Device-mapped* region — and the representative deliberate
/// fault became the translation fault against an unmapped address
/// (`fixture-mmu-fault`), which exercises [`crate::esr::DataAbortIss`] plus
/// the walk level and `FAR_EL1` in one capture.
///
/// **Unverified.** Never executed.
///
/// # Safety
///
/// `address` must be a deliberately misaligned pointer into memory this core
/// owns — the caller chooses it, so that this function never invents an
/// address. Must be called after [`install`], for [`deliberate_breakpoint`]'s
/// reason.
#[cfg(target_arch = "aarch64")]
pub unsafe fn deliberate_alignment_fault(address: *mut u64) -> ! {
    // SAFETY: the store faults before it commits — an alignment fault is taken
    // on the access, so nothing at `address` is written. The caller's contract
    // is what makes the address one this core may name at all.
    unsafe {
        core::arch::asm!(
            "str {value}, [{address}]",
            value = in(reg) 0u64,
            address = in(reg) address,
            options(nostack, preserves_flags),
        )
    };
    park()
}

/// The safe terminal state: park with interrupts masked.
///
/// Not a halt loop out of laziness. With no scheduler and no resume path, a
/// safe state is the only correct terminal state, and
/// `agent/CODING_STANDARDS.md` resolves this as fail-safe over keep-trying.
#[cfg(target_arch = "aarch64")]
fn park() -> ! {
    loop {
        // SAFETY: `wfe` is architecturally permitted at every exception level
        // and has no effect on any state this code owns.
        unsafe { core::arch::asm!("wfe", options(nomem, nostack, preserves_flags)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::esr::FaultStatus;
    use crate::pl011::{register, Pl011};
    use crate::vectors::{EntryKind, EntrySource};
    use core::cell::RefCell;

    /// An always-ready MMIO double that accumulates what reached the wire —
    /// the same double `crate::boot`'s tests use, kept local rather than shared
    /// so neither module's tests can be changed by an edit to the other's.
    struct Wire {
        bytes: RefCell<Vec<u8>>,
    }

    impl Wire {
        fn new() -> Self {
            Wire { bytes: RefCell::new(Vec::new()) }
        }

        fn captured(&self) -> String {
            String::from_utf8(self.bytes.borrow().clone()).expect("the report is ASCII")
        }
    }

    impl Mmio for Wire {
        fn read_u32(&self, _offset: usize) -> u32 {
            0
        }

        fn write_u32(&self, offset: usize, value: u32) {
            if offset == register::DR {
                self.bytes.borrow_mut().push(value as u8);
            }
        }
    }

    /// A data abort at `EL1h`: alignment fault, write, `FAR` valid — what an
    /// unaligned store to Device-nGnRnE memory produces while the MMU is off.
    fn alignment_fault() -> FaultFrame {
        FaultFrame {
            slot: VectorSlot::SYNCHRONOUS_EL1H.index() as u64,
            esr: 0x9600_0061,
            far: 0x0000_0000_0008_F123,
            elr: 0x0000_0000_0008_0244,
            spsr: 0x0000_0000_0000_03C5,
        }
    }

    fn captured_report(frame: &FaultFrame) -> String {
        let wire = Wire::new();
        let uart = Pl011::new(&wire);
        report(&uart, frame).expect("a ready device");
        wire.captured()
    }

    // ---- the canvas transcript (`STORY-P1-07-03` clause 6's channel) -------

    #[test]
    fn the_transcript_sink_yields_the_same_lines_the_wire_carries() {
        let frame = FaultFrame::from_entry(0, 0x9600_0005, 0x20_0000_0000, 0x8_1000, 0x3C5);
        let sink = TranscriptSink::new();
        let uart = Pl011::new(&sink);
        report(&uart, &frame).expect("the sink is always ready");
        let mut lines: Vec<String> = Vec::new();
        sink.for_each_line(|row, line| {
            assert_eq!(row, lines.len(), "rows arrive in order");
            lines.push(String::from_utf8(line.to_vec()).expect("ASCII"));
        });
        // The identical formatting path: the wire capture split into lines
        // must equal what the sink hands the canvas.
        let capture = captured_report(&frame);
        let wire: Vec<&str> = capture.lines().filter(|line| !line.is_empty()).collect();
        assert_eq!(lines, wire);
        assert_eq!(lines.len(), 5, "five report lines, every one painted");
        assert!(lines[0].starts_with("TOS64-FAULT/1 slot="));
    }

    #[test]
    fn the_transcript_sink_truncates_overflow_rather_than_wrapping() {
        let sink = TranscriptSink::new();
        for _ in 0..(TRANSCRIPT_CAPACITY + 100) {
            sink.write_u32(register::DR, u32::from(b'A'));
        }
        sink.write_u32(register::DR, u32::from(b'\n'));
        let mut total = 0;
        sink.for_each_line(|_, line| total += line.len());
        assert_eq!(total, TRANSCRIPT_CAPACITY, "beyond capacity is dropped, not wrapped");
    }

    // The vector entries load `x0`-`x4` in one order and this function names
    // them in another set of words. Five `u64` arguments are mutually
    // substitutable to the type system, so a transposition compiles, runs, and
    // reports a fault that never happened at an address that means nothing.
    #[test]
    fn the_five_entry_registers_land_in_the_fields_the_assembly_names() {
        let frame = FaultFrame::from_entry(1, 2, 3, 4, 5);
        assert_eq!(frame.slot, 1, "x0 is the slot index");
        assert_eq!(frame.esr, 2, "x1 is ESR_EL1");
        assert_eq!(frame.far, 3, "x2 is FAR_EL1");
        assert_eq!(frame.elr, 4, "x3 is ELR_EL1");
        assert_eq!(frame.spsr, 5, "x4 is SPSR_EL1");
    }

    #[test]
    fn a_frame_built_from_entry_reports_the_same_way_one_built_by_hand_does() {
        // `tinyos_arm64_exception_entry` does nothing but forward its arguments
        // into `from_entry`, so this is the whole of the AArch64 entry point's
        // behaviour, exercised on the host.
        let by_hand = alignment_fault();
        let from_entry = FaultFrame::from_entry(
            by_hand.slot,
            by_hand.esr,
            by_hand.far,
            by_hand.elr,
            by_hand.spsr,
        );
        assert_eq!(from_entry, by_hand);
        assert_eq!(captured_report(&from_entry), captured_report(&by_hand));
    }

    #[test]
    fn a_slot_index_outside_the_table_is_reported_as_a_disagreement_not_as_slot_zero() {
        for slot in [ENTRY_COUNT as u64, 16, 99, u64::MAX] {
            let frame = FaultFrame { slot, ..alignment_fault() };
            assert_eq!(frame.slot(), None, "slot {slot}");
        }
        assert_eq!(alignment_fault().slot(), Some(VectorSlot::SYNCHRONOUS_EL1H));
    }

    // Clause 2's shape, host-side: the report names the class, the address and
    // the decoded syndrome. The *evidence* for clause 2 is a board capture and
    // nothing here substitutes for it.
    #[test]
    fn a_data_abort_reports_its_slot_class_syndrome_and_addresses() {
        let captured = captured_report(&alignment_fault());
        assert!(captured.contains("slot=cur_el_spx/sync"), "got: {captured}");
        assert!(captured.contains("class=data-abort"));
        assert!(captured.contains("ec=25"));
        assert!(captured.contains("il=32"));
        assert!(captured.contains("status=alignment"));
        assert!(captured.contains("wnr=write"));
        assert!(captured.contains("far=000000000008F123"));
        assert!(captured.contains("elr=0000000000080244"));
        assert!(captured.contains("spsr=00000000000003C5"));
    }

    #[test]
    fn the_raw_register_is_quoted_beside_the_decode() {
        // `crate::boot::report_entry` quotes raw `CurrentEL` beside the decoded
        // level for this reason, and a fault report needs it more: a wrong
        // decode has to stay diagnosable from the capture alone, without a
        // second session on a board that may be hard to get back to.
        assert!(captured_report(&alignment_fault()).contains("esr=0000000096000061"));
    }

    #[test]
    fn a_class_this_decoder_does_not_name_is_reported_as_unknown_with_its_raw_ec() {
        // Clause 3's second paragraph, on the wire rather than in the decoder:
        // an unnamed class must not borrow a named one's report.
        let frame = FaultFrame { esr: (0x3F << 26) | (1 << 25), ..alignment_fault() };
        let captured = captured_report(&frame);
        assert!(captured.contains("class=unknown-class"), "got: {captured}");
        assert!(captured.contains("ec=3F"));
        // And the architecture's own "unknown reason" is a different line.
        let unknown_reason = FaultFrame { esr: 1 << 25, ..alignment_fault() };
        assert!(captured_report(&unknown_reason).contains("class=unknown-reason"));
    }

    #[test]
    fn a_stale_far_is_reported_as_invalid_rather_than_as_an_address() {
        // The clause-4 invariant made visible: `FAR_EL1` for an `SP` alignment
        // fault holds whatever an earlier, unrelated abort left there. A
        // capture that printed it would send a reader to an address that has
        // nothing to do with the fault.
        let frame = FaultFrame {
            esr: (0x26 << 26) | (1 << 25),
            far: 0xDEAD_BEEF_DEAD_BEEF,
            ..alignment_fault()
        };
        let captured = captured_report(&frame);
        assert!(captured.contains("far=invalid"), "got: {captured}");
        assert!(!captured.contains("DEADBEEF"), "a stale FAR must not reach the wire");
        assert_eq!(frame.faulting_address(), None);
    }

    #[test]
    fn an_abort_with_fnv_set_is_the_same_refusal() {
        let frame = FaultFrame { esr: 0x9600_0061 | (1 << 10), ..alignment_fault() };
        assert_eq!(frame.faulting_address(), None);
        assert!(captured_report(&frame).contains("far=invalid"));
    }

    #[test]
    fn an_instruction_abort_reports_its_own_syndrome() {
        let frame = FaultFrame {
            esr: (0x21 << 26) | (1 << 25) | 0b00_0111,
            far: 0x0000_0000_0008_0000,
            ..alignment_fault()
        };
        let captured = captured_report(&frame);
        assert!(captured.contains("class=instruction-abort"), "got: {captured}");
        assert!(captured.contains("status=translation"));
        assert!(captured.contains("level=3"));
        // A data abort's write bit has no meaning in this syndrome and must
        // not be reported as though it did.
        assert!(!captured.contains("wnr="), "got: {captured}");
    }

    #[test]
    fn a_class_with_no_class_specific_syndrome_reports_the_raw_iss() {
        // A `BRK` carries its comment in the ISS. This Story does not decode
        // that field, so the raw value is what goes on the wire — reporting
        // nothing would lose the one thing that distinguishes two `BRK`s.
        let frame = FaultFrame { esr: (0x3C << 26) | (1 << 25) | 0x1234, ..alignment_fault() };
        let captured = captured_report(&frame);
        assert!(captured.contains("class=brk64"), "got: {captured}");
        assert!(captured.contains("iss=00001234"));
    }

    #[test]
    fn every_one_of_the_sixteen_slots_reports_a_distinct_name() {
        // Clause 1, from the report's side: a fault arriving at any slot has to
        // be attributable to that slot from the capture alone.
        for slot in VectorSlot::ALL {
            let frame = FaultFrame { slot: slot.index() as u64, ..alignment_fault() };
            let captured = captured_report(&frame);
            assert!(captured.contains(&format!("slot={}", slot.name())), "{slot:?}");
        }
    }

    #[test]
    fn an_unknown_slot_index_still_produces_a_report() {
        // The worst case: the table and this code disagree. The report must
        // still name what it can rather than falling back to silence, which is
        // the failure mode this whole Story is against.
        let frame = FaultFrame { slot: 99, ..alignment_fault() };
        let captured = captured_report(&frame);
        assert!(captured.contains("slot=unknown"), "got: {captured}");
        assert!(captured.contains("class=data-abort"));
    }

    // Clause 5: bounded. A wedged UART makes the report fail rather than stall.
    #[test]
    fn a_wedged_uart_makes_the_report_fail_rather_than_stall() {
        struct Wedged;
        impl Mmio for Wedged {
            fn read_u32(&self, _offset: usize) -> u32 {
                crate::pl011::flag::TXFF
            }
            fn write_u32(&self, _offset: usize, _value: u32) {}
        }
        let uart = Pl011::new(Wedged);
        assert_eq!(report(&uart, &alignment_fault()), Err(Pl011Error::TransmitTimeout));
    }

    #[test]
    fn no_reported_line_contains_a_carriage_return_the_framer_would_double() {
        // The defect `TEST-P1-07-01-A` records: `write_str("\r\n")` frames the
        // LF and puts `\r\r\n` on the wire. Invisible on most terminals, and
        // therefore exactly the thing that survives review and lands inside a
        // quoted capture offered as evidence.
        assert!(!captured_report(&alignment_fault()).contains("\r\r"));
    }

    #[test]
    fn the_report_says_that_the_system_stopped() {
        // Clause 5's second paragraph: a fault inside the fault handler is not
        // survivable by this Story and must not be claimed to be. The capture
        // says the system halted, so nobody reading it infers a resume that
        // does not exist.
        let captured = captured_report(&alignment_fault());
        assert!(captured.contains("halted"), "got: {captured}");
    }

    // Clause 4: the frame is evidence, never authority. Two frames differing in
    // every reported field reach the same terminal outcome.
    #[test]
    fn nothing_in_the_frame_changes_what_the_handler_does() {
        // Stated as a property of this module's surface: `report` returns only
        // a transmit result, `faulting_address` returns an address or nothing,
        // and there is no function here that turns a frame into a decision.
        // The test that would fail if one were added is the next one, which
        // runs the kernel's own policy against these frames.
        let frames = [
            alignment_fault(),
            FaultFrame { esr: 0, far: 0, elr: 0, spsr: 0, ..alignment_fault() },
            FaultFrame { esr: u64::MAX, far: u64::MAX, ..alignment_fault() },
        ];
        for frame in frames {
            let wire = Wire::new();
            let uart = Pl011::new(&wire);
            assert_eq!(report(&uart, &frame), Ok(()));
            assert!(wire.captured().contains("halted"));
        }
    }

    // Clause 4, against the real policy: `kernel::fault` is the x86_64 Story's
    // disposition code, and it is run here unmodified against AArch64 frames.
    // If the invariant "the disposition depends only on where the fault
    // happened" were arch-shaped, this is where it would show.
    #[test]
    fn the_kernels_disposition_policy_consumes_an_aarch64_frame_unmodified() {
        use kernel::fault::{Disposition, FaultReport, FaultingContext};

        // Every context this Feature can have is kernel context: there are no
        // tasks on this board, and `FEAT-P1-07` §6 keeps it that way. The
        // task-contained arm is exercised by `kernel::fault`'s own tests.
        let baseline = Disposition::of(&FaultReport {
            vector: VectorSlot::SYNCHRONOUS_EL1H.index() as u64,
            context: FaultingContext::Kernel,
        });
        assert_eq!(baseline, Disposition::HaltSystem);
        assert!(!baseline.system_survives());

        for slot in VectorSlot::ALL {
            for esr in [0u64, 0x9600_0061, 0x9600_0021, (0x3F << 26) | 0x1FF_FFFF, u64::MAX] {
                let frame = FaultFrame {
                    slot: slot.index() as u64,
                    esr,
                    far: esr.rotate_left(17),
                    elr: esr.rotate_left(29),
                    spsr: 0x3C5,
                };
                // The only field of the frame that reaches the policy is the
                // slot, and it reaches it as a record, not as an input to a
                // branch. Everything decodable stays on the serial side.
                let disposition = Disposition::of(&FaultReport {
                    vector: frame.slot,
                    context: FaultingContext::Kernel,
                });
                assert_eq!(disposition, baseline, "{slot:?} with ESR {esr:#x}");
            }
        }
    }

    // Clause 6: every fault is a spoor, and the spoor carries no register
    // content and no faulting address (`PD-12`).
    #[test]
    fn a_fault_spoor_carries_the_context_and_nothing_the_frame_decoded() {
        use kernel::fault::{audit, Disposition, FaultReport, FaultingContext};
        use kernel::spoor::{Action, Actor, Category, Outcome};

        let report = FaultReport {
            vector: VectorSlot::SYNCHRONOUS_EL1H.index() as u64,
            context: FaultingContext::Kernel,
        };
        let [captured, decided] = audit(&report, Disposition::of(&report));

        assert_eq!(captured.category(), Category::Fault);
        assert_eq!(captured.who(), Actor::Kernel);
        assert_eq!(captured.action(), Action::Fault);
        assert_eq!(captured.outcome(), Outcome::Failed);
        assert_eq!(decided.action(), Action::Terminate);
        // Nothing was contained: this board has no task to contain a fault to.
        assert_eq!(decided.outcome(), Outcome::Failed);

        // The atom has exactly two payload fields. The slot index is one of
        // them; the other is the context. There is nowhere for an `ESR_EL1` or
        // a `FAR_EL1` to hide, and this is what pins that.
        assert_eq!(captured.cost(), VectorSlot::SYNCHRONOUS_EL1H.index() as u32);
        assert_eq!(captured.target(), 0);
    }

    #[test]
    fn two_faults_at_one_slot_audit_identically_however_different_their_frames() {
        // The strongest statement of clause 6 available on the host: a spoor
        // that varied with `ESR_EL1` or `FAR_EL1` would be carrying register
        // content, whatever its field names said.
        use kernel::fault::{audit, Disposition, FaultReport, FaultingContext};

        let report = FaultReport {
            vector: VectorSlot::SYNCHRONOUS_EL1H.index() as u64,
            context: FaultingContext::Kernel,
        };
        let first = audit(&report, Disposition::of(&report));
        assert_eq!(first, audit(&report, Disposition::of(&report)));

        // Two frames that share a slot and agree on nothing else.
        let a = alignment_fault();
        let b = FaultFrame { esr: u64::MAX, far: u64::MAX, elr: 1, spsr: 2, ..a };
        assert_eq!(a.slot, b.slot);
        assert_ne!(captured_report(&a), captured_report(&b), "the serial report does differ");
        // ...and the audit atoms do not.
        let report_a = FaultReport { vector: a.slot, context: FaultingContext::Kernel };
        let report_b = FaultReport { vector: b.slot, context: FaultingContext::Kernel };
        assert_eq!(
            audit(&report_a, Disposition::of(&report_a)),
            audit(&report_b, Disposition::of(&report_b))
        );
    }

    // The `VBAR_EL1` read-back report.
    #[test]
    fn a_matching_vbar_readback_is_reported_as_a_match() {
        let wire = Wire::new();
        let uart = Pl011::new(&wire);
        report_vbar(&uart, 0x0008_0800, 0x0008_0800).expect("ready");
        let captured = wire.captured();
        assert!(captured.contains("vbar=0000000000080800"), "got: {captured}");
        assert!(captured.contains("readback=0000000000080800"));
        assert!(captured.contains("match=yes"));
    }

    #[test]
    fn a_vbar_the_hardware_did_not_take_is_reported_as_a_mismatch() {
        // The failure this exists to catch: `VBAR_EL1[10:0]` are `RES0`, so a
        // misaligned base is *ignored*, and the symptom is that the handler
        // never runs. The alignment is asserted at build time; this is the
        // second line of defence, and it costs three words on the wire.
        let wire = Wire::new();
        let uart = Pl011::new(&wire);
        report_vbar(&uart, 0x0008_0880, 0x0008_0800).expect("ready");
        assert!(wire.captured().contains("match=no"), "got: {}", wire.captured());
    }

    #[test]
    fn a_byte_renders_as_two_uppercase_hex_digits() {
        assert_eq!(hex_u8(0x00), *b"00");
        assert_eq!(hex_u8(0x25), *b"25");
        assert_eq!(hex_u8(0x3F), *b"3F");
        assert_eq!(hex_u8(0xFF), *b"FF");
    }

    // A guard on the decoders this report reaches for, so a class added to
    // `crate::esr` without a report arm shows up here rather than on a board.
    #[test]
    fn every_named_class_produces_a_report_naming_it() {
        for ec in 0u8..64 {
            let frame = FaultFrame { esr: ((ec as u64) << 26) | (1 << 25), ..alignment_fault() };
            let class = frame.esr().class();
            let captured = captured_report(&frame);
            assert!(
                captured.contains(&format!("class={}", class.as_str())),
                "EC {ec:#04x} ({class:?}) is not named in its own report: {captured}"
            );
            assert!(
                captured.contains(&format!("ec={}", core::str::from_utf8(&hex_u8(ec)).unwrap()))
            );
        }
    }

    #[test]
    fn a_data_abort_status_reaches_the_wire_for_every_status_this_decoder_names() {
        for (bits, expected) in [
            (0b000100u32, FaultStatus::Translation(0)),
            (0b001111, FaultStatus::Permission(3)),
            (0b010000, FaultStatus::ExternalAbort),
            (0b100001, FaultStatus::Alignment),
            (0b110000, FaultStatus::TlbConflict),
            (0b111111, FaultStatus::Unrecognised(0b111111)),
        ] {
            let frame =
                FaultFrame { esr: (0x25 << 26) | (1 << 25) | bits as u64, ..alignment_fault() };
            let captured = captured_report(&frame);
            assert!(
                captured.contains(&format!("status={}", expected.as_str())),
                "status {bits:#08b}: {captured}"
            );
            match expected.level() {
                Some(level) => assert!(captured.contains(&format!("level={level}"))),
                None => assert!(captured.contains("level=none"), "got: {captured}"),
            }
        }
    }

    #[test]
    fn the_entry_group_is_reported_even_for_slots_that_should_never_fire() {
        // `EL0` and AArch32 entries cannot fire in this Feature. If one does,
        // the capture has to say so plainly rather than reporting a plausible
        // `EL1h` fault — that would be a confident, wrong answer at the exact
        // moment the system is least understood.
        let unreachable = VectorSlot { source: EntrySource::LowerElAarch32, kind: EntryKind::Fiq };
        let frame = FaultFrame { slot: unreachable.index() as u64, ..alignment_fault() };
        assert!(captured_report(&frame).contains("slot=lower_el_a32/fiq"));
    }
}
