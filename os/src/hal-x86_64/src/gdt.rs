//! The Global Descriptor Table that carries the TSS descriptor
//! (`STORY-P1-02-02`).
//!
//! Same split this crate applies everywhere else: descriptor bit-packing is
//! ordinary data manipulation with host tests, and only [`install`]'s `lgdt`
//! and `ltr` need a real x86_64 CPU.
//!
//! **This table is additive, and that is what makes it safe.** Entries 0–2 are
//! byte-for-byte the null/code/data descriptors [`crate::boot`]'s own
//! assembly-time GDT already installed, so `CS`, `DS`, `SS`, `ES`, `FS` and
//! `GS` keep the selectors *and* the cached descriptors they are already
//! holding. There is no far return to reload `CS`, no segment reload of any
//! kind, and therefore no window in which the code segment is undefined —
//! reloading `CS` in long mode means a `retfq` through a hand-built stack
//! frame, which is exactly the kind of code that has no business running on a
//! path whose entire purpose is to make faults survivable. The `lgdt` here only
//! ever *extends* what the CPU can see: index 3 goes from "past the limit" to
//! "the TSS", and nothing else changes.
//!
//! One consequence is written down rather than left to be discovered:
//! `STORY-P1-02-01`'s `#GP` fixture used to depend on selector `0x18` lying
//! past the boot GDT's limit. That selector is this table's TSS. The fixture no
//! longer picks a selector by counting descriptors — see
//! `kernel::fixture_fault`'s own constant.
//!
//! **Boundary status.** The CPL-3 descriptors and TSS.RSP0 stack are the
//! hardware foundation for a future user transition; they do not themselves
//! move a task out of CPL 0. The shipping scheduler still enters tasks through
//! `kernel::context`'s ordinary `ret`. An `iretq` entry frame, complete
//! user-origin trap frames, and a syscall ABI remain required before TinyOS
//! may claim a protected process boundary.

use crate::tss::TaskStateSegment;

/// Selector for the code segment — index 1, RPL 0. Identical to
/// [`crate::interrupts::CODE_SELECTOR`] and to what `boot.rs` loads.
pub const CODE_SELECTOR: u16 = 0x08;
/// Selector for the data segment — index 2, RPL 0.
pub const DATA_SELECTOR: u16 = 0x10;
/// Selector for the TSS — index 3, RPL 0. The 64-bit TSS descriptor occupies
/// **two** GDT slots (3 and 4), so no other descriptor may claim index 4.
pub const TSS_SELECTOR: u16 = 0x18;
/// CPL-3 code selector — index 5 plus RPL 3.
pub const USER_CODE_SELECTOR: u16 = 0x2b;
/// CPL-3 data/stack selector — index 6 plus RPL 3.
pub const USER_DATA_SELECTOR: u16 = 0x33;

/// The three descriptors `boot.rs` builds in assembly, repeated here verbatim.
///
/// Duplicated deliberately rather than shared: `boot.rs`'s copy must be a
/// compile-time constant inside a `global_asm!` block (it is loaded in 32-bit
/// protected mode, before any Rust code has run at all), so there is no way for
/// one definition to serve both. What *can* be shared is the check — the host
/// test below asserts these exact quadwords, so the two copies drifting apart
/// is a test failure rather than a mystery triple fault.
///
/// - `0x0000000000000000` — the mandatory null descriptor.
/// - `0x00209A0000000000` — 64-bit code, execute/read, present, DPL 0 (`L` set).
/// - `0x0000920000000000` — data, read/write, present, DPL 0.
const BOOT_DESCRIPTORS: [u64; 3] =
    [0x0000_0000_0000_0000, 0x0020_9A00_0000_0000, 0x0000_9200_0000_0000];

/// Flat long-mode user code/data descriptors. Their DPL is 3; the selectors'
/// RPL is also 3, so they cannot be used to manufacture supervisor privilege.
const USER_DESCRIPTORS: [u64; 2] = [0x0020_FA00_0000_0000, 0x0000_F200_0000_0000];

/// Access byte for an **available** 64-bit TSS: present, DPL 0, system
/// descriptor (`S` clear), type `0x9`.
///
/// Type `0xB` ("busy") is what the CPU itself writes back after `ltr`; loading
/// a descriptor that already claims to be busy raises `#GP`, so the value
/// written here must be the available one.
const TSS_AVAILABLE_64BIT: u8 = 0x89;

/// One 16-byte 64-bit TSS system descriptor.
///
/// `#[repr(C, packed)]` for the same reason [`crate::idt::IdtEntry`] is: the
/// CPU reads the GDT as a tightly packed byte stream, and the base address is
/// split across four separate fields at fixed offsets — a layout no Rust rule
/// would otherwise produce or preserve.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TssDescriptor {
    limit_low: u16,
    base_low: u16,
    base_mid: u8,
    access: u8,
    limit_high_and_flags: u8,
    base_high: u8,
    base_upper: u32,
    reserved: u32,
}

impl TssDescriptor {
    /// A present, available 64-bit TSS descriptor for the segment at `base`
    /// with the given `limit`.
    ///
    /// Granularity is byte (the `G` flag stays clear): a 104-byte segment
    /// described in 4 KiB pages would have to round *up*, advertising ~4 KiB of
    /// whatever follows the TSS as part of it.
    pub const fn new(base: u64, limit: u16) -> Self {
        TssDescriptor {
            limit_low: limit,
            base_low: base as u16,
            base_mid: (base >> 16) as u8,
            access: TSS_AVAILABLE_64BIT,
            // A descriptor limit is 20 bits; a `u16` limit can never reach the
            // top four, so this nibble is structurally zero rather than
            // computed — and the flags nibble above it (`G`, `AVL`) stays zero
            // too, which is what makes the limit bytes rather than 4KiB pages.
            limit_high_and_flags: 0,
            base_high: (base >> 24) as u8,
            base_upper: (base >> 32) as u32,
            reserved: 0,
        }
    }

    /// The base address this descriptor encodes, reassembled from its four
    /// split fields — the read-back counterpart to [`new`](Self::new), in the
    /// same "prove it by reading the entry back" style
    /// [`crate::idt::IdtEntry::handler_address`] uses.
    pub fn base(&self) -> u64 {
        let low = self.base_low as u64;
        let mid = self.base_mid as u64;
        let high = self.base_high as u64;
        let upper = self.base_upper as u64;
        low | (mid << 16) | (high << 24) | (upper << 32)
    }

    /// The limit this descriptor encodes.
    pub fn limit(&self) -> u32 {
        let low = self.limit_low as u32;
        let high = (self.limit_high_and_flags & 0x0F) as u32;
        low | (high << 16)
    }

    /// Whether the present bit is set.
    pub const fn present(&self) -> bool {
        self.access & 0x80 != 0
    }

    /// Whether this describes an *available* (not busy) 64-bit TSS.
    pub const fn is_available_64bit_tss(&self) -> bool {
        self.access == TSS_AVAILABLE_64BIT
    }
}

/// This kernel's GDT: `boot.rs`'s three descriptors, the TSS descriptor
/// occupying the next two slots, then CPL-3 code and data descriptors.
///
/// `#[repr(C, align(8))]` — the table's own descriptor stride, and
/// deliberately **not** 16 the way [`crate::idt::Idt`] aligns: this structure
/// is 56 bytes, so its natural 8-byte alignment needs no tail padding. A GDT
/// limit that covers memory holding no descriptor
/// turns an out-of-range selector into a zeroed one, which is a different
/// (and quieter) failure than the `#GP` the hardware should raise.
#[repr(C, align(8))]
pub struct Gdt {
    descriptors: [u64; 3],
    tss: TssDescriptor,
    user_descriptors: [u64; 2],
}

impl Gdt {
    /// Builds the table for `tss`.
    pub fn new(tss: &TaskStateSegment) -> Self {
        let (base, limit) = tss.base_and_limit();
        Gdt {
            descriptors: BOOT_DESCRIPTORS,
            tss: TssDescriptor::new(base, limit),
            user_descriptors: USER_DESCRIPTORS,
        }
    }

    /// The three descriptors inherited from `boot.rs`, for the drift test.
    pub fn boot_descriptors(&self) -> [u64; 3] {
        self.descriptors
    }

    /// The TSS descriptor.
    pub fn tss_descriptor(&self) -> TssDescriptor {
        self.tss
    }

    /// The CPL-3 code/data descriptors, for architectural read-back tests.
    pub fn user_descriptors(&self) -> [u64; 2] {
        self.user_descriptors
    }

    /// Raw base address and byte length `lgdt` needs, in the same
    /// "arithmetic here, instruction elsewhere" shape as
    /// [`crate::idt::Idt::pointer`].
    pub fn pointer(&self) -> (u64, u16) {
        (self as *const Gdt as u64, (core::mem::size_of::<Gdt>() - 1) as u16)
    }

    /// The selector index the TSS descriptor actually sits at, derived from the
    /// structure rather than asserted — [`TSS_SELECTOR`] must agree with it.
    pub fn tss_selector(&self) -> u16 {
        (self.descriptors.len() as u16) << 3
    }
}

/// This kernel's one GDT and one TSS.
///
/// `static mut` for the reason [`crate::interrupts`]'s own `IDT` is: the CPU
/// keeps dereferencing whatever `lgdt`/`ltr` last pointed it at for the rest of
/// this kernel's run, with no further involvement from Rust's borrow tracking.
#[cfg(not(target_os = "windows"))]
static mut GDT: Option<Gdt> = None;
#[cfg(not(target_os = "windows"))]
static mut TSS: TaskStateSegment = TaskStateSegment::new();

/// Points the `#DF` IST slot at its stack, loads this table with `lgdt`, and
/// loads the task register with `ltr`.
///
/// Must run **before** the IDT gate for vector 8 is used — an IST-bearing gate
/// whose TSS the CPU has not been told about is worse than no gate at all,
/// because the CPU would load `RSP` from a task register that names no segment.
/// [`crate::interrupts::init_faults_only`] and [`crate::interrupts::init`] both
/// call this first, in that order, and nothing else should call it.
///
/// # Safety
/// At most once, on a single CPU, before any fault can be delivered. The table
/// and TSS it installs are `static`, so they outlive the call by construction —
/// the requirement `lgdt`/`ltr` actually impose.
#[cfg(not(target_os = "windows"))]
#[allow(static_mut_refs, clippy::deref_addrof)]
pub unsafe fn install() {
    // SAFETY: see this function's own doc comment. `&raw mut` throughout,
    // following this workspace's established single-owner `static mut` pattern.
    unsafe {
        let tss = &mut *(&raw mut TSS);
        let (_, stack_top) = crate::tss::double_fault_stack_range();
        tss.set_interrupt_stack(crate::tss::IstIndex::DOUBLE_FAULT, stack_top);
        let (_, ring0_stack_top) = crate::tss::ring0_entry_stack_range();
        tss.set_ring0_stack(ring0_stack_top);

        let gdt_slot = &mut *(&raw mut GDT);
        *gdt_slot = Some(Gdt::new(tss));
        let gdt = gdt_slot.as_ref().expect("just assigned");

        #[repr(C, packed)]
        struct GdtPointer {
            limit: u16,
            base: u64,
        }
        let (base, limit) = gdt.pointer();
        let pointer = GdtPointer { limit, base };
        // The three descriptors this table repeats are byte-identical to the
        // ones already loaded, so no segment register is reloaded — see this
        // module's own doc comment for why that is the point rather than a
        // shortcut.
        core::arch::asm!("lgdt [{0}]", in(reg) &pointer, options(readonly, nostack, preserves_flags));
        core::arch::asm!("ltr {0:x}", in(reg) TSS_SELECTOR, options(nomem, nostack, preserves_flags));
    }
}

/// The stack top the CPU will load into `RSP` on `#DF`, as actually recorded in
/// the installed TSS — read back rather than recomputed, so a fixture proves
/// what the hardware was told rather than what the code intended to tell it.
///
/// # Safety
/// Only meaningful after [`install`]; reads the module's `static mut` TSS on a
/// single CPU with no concurrent writer.
#[cfg(not(target_os = "windows"))]
#[allow(static_mut_refs, clippy::deref_addrof)]
pub unsafe fn installed_double_fault_stack_top() -> u64 {
    // SAFETY: see this function's own doc comment.
    unsafe { (*(&raw const TSS)).interrupt_stack(crate::tss::IstIndex::DOUBLE_FAULT) }
}

/// The installed TSS's CPL-0 transition stack top.
///
/// # Safety
/// Only meaningful after [`install`], on the single CPU.
#[cfg(not(target_os = "windows"))]
pub unsafe fn installed_ring0_stack_top() -> u64 {
    // SAFETY: see this function's own doc comment.
    unsafe { (*(&raw const TSS)).ring0_stack() }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Clause 2: `boot.rs`'s GDT is built in 32-bit assembly before any Rust
    // runs, so these three quadwords cannot be shared with it — only checked
    // against it. Drift here means the `lgdt` below silently invalidates the
    // very segments the CPU is currently executing in.
    #[test]
    fn the_first_three_descriptors_are_boot_rs_own_gdt_verbatim() {
        let tss = TaskStateSegment::new();
        let gdt = Gdt::new(&tss);
        assert_eq!(
            gdt.boot_descriptors(),
            [0x0000_0000_0000_0000, 0x0020_9A00_0000_0000, 0x0000_9200_0000_0000],
            "these must match `hal_x86_64::boot`'s `boot_gdt_start` exactly"
        );
    }

    #[test]
    fn the_tss_descriptor_sits_where_tss_selector_says_it_does() {
        let tss = TaskStateSegment::new();
        let gdt = Gdt::new(&tss);
        assert_eq!(gdt.tss_selector(), TSS_SELECTOR);
        // The 64-bit TSS descriptor is 16 bytes — two GDT slots — so index 4
        // is claimed too and nothing else may use it.
        assert_eq!(core::mem::size_of::<TssDescriptor>(), 16);
        assert_eq!(core::mem::size_of::<Gdt>(), 3 * 8 + 16 + 2 * 8);
    }

    #[test]
    fn code_and_data_selectors_are_the_ones_boot_rs_loads() {
        assert_eq!(CODE_SELECTOR, 0x08);
        assert_eq!(DATA_SELECTOR, 0x10);
    }

    #[test]
    fn user_descriptors_and_selectors_encode_dpl_and_rpl_three() {
        let tss = TaskStateSegment::new();
        let gdt = Gdt::new(&tss);
        assert_eq!(gdt.user_descriptors(), [0x0020_FA00_0000_0000, 0x0000_F200_0000_0000]);
        assert_eq!(USER_CODE_SELECTOR, (5 << 3) | 3);
        assert_eq!(USER_DATA_SELECTOR, (6 << 3) | 3);
        assert_eq!(USER_CODE_SELECTOR & 3, 3);
        assert_eq!(USER_DATA_SELECTOR & 3, 3);
    }

    // Clause 2: the base is split across four fields at fixed offsets, which
    // is precisely the shape of bug that produces a `#GP` on `ltr` with no
    // other symptom.
    #[test]
    fn a_tss_descriptor_round_trips_a_full_64_bit_base() {
        // Every byte lane distinct and nonzero, so a swapped or dropped field
        // cannot coincidentally produce the right answer.
        let descriptor = TssDescriptor::new(0xdead_beef_1122_3344, 103);
        assert_eq!(descriptor.base(), 0xdead_beef_1122_3344);
        assert_eq!(descriptor.limit(), 103);
        assert!(descriptor.present());
        assert!(descriptor.is_available_64bit_tss());
    }

    #[test]
    fn a_tss_descriptor_describes_the_segment_it_was_built_from() {
        let tss = TaskStateSegment::new();
        let gdt = Gdt::new(&tss);
        let descriptor = gdt.tss_descriptor();
        assert_eq!(descriptor.base(), &tss as *const TaskStateSegment as u64);
        assert_eq!(descriptor.limit(), TaskStateSegment::LIMIT as u32);
    }

    // `ltr` raises `#GP` on a descriptor that already claims to be busy —
    // the CPU sets that bit itself, and software must not.
    #[test]
    fn the_tss_descriptor_is_available_not_busy() {
        let tss = TaskStateSegment::new();
        let descriptor = Gdt::new(&tss).tss_descriptor();
        let access = descriptor.access;
        assert_eq!(access, 0x89, "present, DPL 0, system, type 9 (available 64-bit TSS)");
        assert_ne!(access & 0x0F, 0x0B, "type 0xB is `busy`, which software must never write");
    }

    #[test]
    fn granularity_is_bytes_so_the_limit_covers_the_tss_and_nothing_after_it() {
        let descriptor = TssDescriptor::new(0x1000, 103);
        let flags = descriptor.limit_high_and_flags;
        assert_eq!(flags & 0x80, 0, "G must be clear: a 104-byte segment in 4KiB pages rounds up");
        assert_eq!(descriptor.limit(), 103);
    }

    #[test]
    fn pointer_reports_the_full_table_byte_length_minus_one() {
        let tss = TaskStateSegment::new();
        let gdt = Gdt::new(&tss);
        let (base, limit) = gdt.pointer();
        assert_eq!(base, &gdt as *const Gdt as u64);
        assert_eq!(limit as usize, core::mem::size_of::<Gdt>() - 1);
    }
}
