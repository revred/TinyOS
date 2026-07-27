//! x86_64 Interrupt Descriptor Table structure (`STORY-P0-04-02`).
//!
//! Pure, host-testable data-structure manipulation, mirroring `paging.rs`'s
//! own split between "build the table" (this module) and "make the CPU use
//! it" (`interrupts.rs`, which owns the `lidt`/`sti`/ISR-trampoline
//! machinery this module has no dependency on). Every function here only
//! ever reads or writes ordinary memory, never a CPU control register or
//! MSR — the same boundary `paging.rs`'s own doc comment draws for the same
//! reason: keep the part that's provably correct on any host toolchain
//! separate from the part that only means anything under a real x86_64 CPU.
//!
//! Long-mode interrupt/trap gate descriptors are 16 bytes: a 64-bit handler
//! address split across three fields (`offset_low`/`offset_mid`/`offset_high`),
//! the code-segment `selector` the CPU switches to before running the
//! handler, an `ist` (Interrupt Stack Table) index (`0` here — this Story
//! does not yet stand up a TSS/IST; see this module's own doc comment on
//! [`Idt`] for what that leaves open), and `type_attr` (present bit, DPL,
//! gate type).

/// Byte size of one x86_64 long-mode IDT gate descriptor — architecturally
/// fixed by the CPU, not a caller-tunable capacity (the same "hardware
/// constant, not a `kernel::capacities` entry" precedent `paging.rs`'s own
/// `ENTRY_COUNT` already established).
pub const ENTRY_COUNT: usize = 256;

const PRESENT: u8 = 1 << 7;
/// 64-bit interrupt gate (type `0xE`) at DPL 0: `PRESENT | DPL(00) | 0 | 0b1110`.
/// An interrupt gate (as opposed to a trap gate, type `0xF`) clears `IF` on
/// entry, so a nested interrupt cannot recursively fire mid-handler before
/// this module's caller has decided it is safe to re-enable one — matching
/// `agent/CODING_STANDARDS.md`'s fail-safe-over-keep-trying discipline: no
/// handler here is written to tolerate reentrancy, so entry must exclude it.
const INTERRUPT_GATE_TYPE_ATTR: u8 = PRESENT | 0x0E;

/// One 16-byte x86_64 long-mode interrupt/trap gate descriptor.
///
/// `#[repr(C, packed)]`: the CPU reads this table as a tightly packed
/// sequence of 16-byte entries with no compiler-inserted padding — the same
/// requirement `paging.rs`'s `PageTable` has for its own entries, enforced
/// here by packing rather than by relying on natural alignment happening to
/// match (an `IdtEntry`'s largest field is `u32`, which Rust would otherwise
/// align to a 4-byte boundary that already satisfies 16-byte-total sizing by
/// coincidence — packing removes the "by coincidence" and states the layout
/// requirement explicitly).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    /// A not-present entry — the CPU raises `#GP` if this vector is ever
    /// actually delivered, per hardware's own contract for a clear present
    /// bit. [`Idt::new`] starts every one of its 256 slots this way;
    /// `STORY-P0-04-02`'s own acceptance criterion 2 is exactly that no
    /// production [`Idt`] is ever loaded while any slot is still in this
    /// state — see [`Idt::set_handler`] and [`Idt::every_entry_present`].
    const fn missing() -> Self {
        IdtEntry {
            offset_low: 0,
            selector: 0,
            ist: 0,
            type_attr: 0,
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    /// A present, DPL-0, 64-bit interrupt-gate entry pointing at `handler`
    /// (an absolute virtual address) with `code_selector` as the segment the
    /// CPU switches `CS` to before running it.
    fn new(handler: u64, code_selector: u16) -> Self {
        IdtEntry {
            offset_low: handler as u16,
            selector: code_selector,
            ist: 0,
            type_attr: INTERRUPT_GATE_TYPE_ATTR,
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }

    /// Whether this entry's present bit is set — see [`missing`](Self::missing).
    pub const fn present(&self) -> bool {
        self.type_attr & PRESENT != 0
    }

    /// The handler address this entry encodes, reassembled from its three
    /// split fields — the read-back counterpart to [`new`](Self::new),
    /// mirroring `paging.rs::translate`'s own "prove it by reading the
    /// entry back" verification style.
    pub fn handler_address(&self) -> u64 {
        let low = self.offset_low as u64;
        let mid = self.offset_mid as u64;
        let high = self.offset_high as u64;
        low | (mid << 16) | (high << 32)
    }

    /// The code-segment selector this entry encodes.
    pub const fn selector(&self) -> u16 {
        self.selector
    }
}

/// A full 256-entry x86_64 long-mode Interrupt Descriptor Table.
///
/// `#[repr(C, align(16))]`: `lidt` takes a base address with no alignment
/// requirement stricter than the entries' own natural packing, but 16-byte
/// alignment is cheap to guarantee and keeps the table's own start aligned
/// with its 16-byte entry stride, mirroring `paging.rs`'s `PageTable`
/// (`align(4096)`) applying the identical "align the table to its own
/// element boundary" discipline one level down.
///
/// **What loading this table does not yet provide** (named explicitly per
/// this project's "surface the gap, don't silently claim it's solved"
/// discipline): every entry [`Idt::set_handler`] never explicitly assigns
/// is wired, by [`crate::interrupts::init`], to a single shared fail-closed
/// handler that never resumes execution (see that module's own doc comment
/// for why this is safe without per-vector assembly trampolines) — this
/// correctly turns "an unrouted or spurious interrupt" into a reported,
/// attributable failure instead of silent corruption, but it does **not**
/// yet stand up a TSS/Interrupt Stack Table, so a genuine `#DF` (double
/// fault) or `#MC` (machine check) whose own stack is itself invalid can
/// still fault a second time while the CPU is trying to push that vector's
/// interrupt frame — a real, general, and unresolved hardware limitation of
/// any kernel without IST-backed known-good stacks for those two vectors
/// specifically, tracked as a named follow-up rather than assumed solved.
#[repr(C, align(16))]
pub struct Idt {
    entries: [IdtEntry; ENTRY_COUNT],
}

impl Idt {
    /// An IDT with every one of its 256 entries not-present.
    pub const fn new() -> Self {
        Idt { entries: [IdtEntry::missing(); ENTRY_COUNT] }
    }

    /// Wires `vector` to `handler` (an absolute virtual address), using
    /// `code_selector` as the segment the CPU switches to before running it.
    pub fn set_handler(&mut self, vector: u8, handler: u64, code_selector: u16) {
        self.entries[vector as usize] = IdtEntry::new(handler, code_selector);
    }

    /// The entry currently wired for `vector`.
    pub fn entry(&self, vector: u8) -> IdtEntry {
        self.entries[vector as usize]
    }

    /// Whether every one of the 256 entries is present — the structural
    /// half of `STORY-P0-04-02` acceptance criterion 2 ("spurious/unrouted
    /// interrupts are handled explicitly ... never silently ignored"):
    /// an `Idt` this returns `false` for still has at least one vector that
    /// would raise `#GP` on delivery instead of reaching an explicit,
    /// documented handler, and must never be [`load`](crate::interrupts::load)ed.
    pub fn every_entry_present(&self) -> bool {
        self.entries.iter().all(IdtEntry::present)
    }

    /// Raw base address and byte length `lidt` needs — exposed here (rather
    /// than computed inline in `interrupts.rs`) so the address arithmetic
    /// itself is host-testable, even though only `interrupts.rs` ever
    /// executes `lidt` with it.
    pub fn pointer(&self) -> (u64, u16) {
        (self as *const Idt as u64, (ENTRY_COUNT * core::mem::size_of::<IdtEntry>() - 1) as u16)
    }
}

impl Default for Idt {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The CPU reads this table as a raw byte stream at this exact stride —
    // any drift here is a silent memory-corruption bug the type system
    // can't otherwise catch, since `lidt` takes a bare pointer/length.
    #[test]
    fn idt_entry_is_exactly_sixteen_bytes() {
        assert_eq!(core::mem::size_of::<IdtEntry>(), 16);
    }

    #[test]
    fn a_fresh_idt_has_no_present_entries() {
        let idt = Idt::new();
        assert!(!idt.every_entry_present());
        for vector in 0..=255u8 {
            assert!(!idt.entry(vector).present(), "vector {vector} should not be present yet");
        }
    }

    #[test]
    fn set_handler_marks_the_entry_present_with_the_given_address_and_selector() {
        let mut idt = Idt::new();
        idt.set_handler(0x30, 0xffff_8000_1234_5678, 0x08);
        let entry = idt.entry(0x30);
        assert!(entry.present());
        assert_eq!(entry.handler_address(), 0xffff_8000_1234_5678);
        assert_eq!(entry.selector(), 0x08);
    }

    #[test]
    fn setting_one_vector_leaves_every_other_vector_untouched() {
        let mut idt = Idt::new();
        idt.set_handler(0x30, 0x1000, 0x08);
        assert!(idt.entry(0x30).present());
        assert!(!idt.entry(0x2f).present());
        assert!(!idt.entry(0x31).present());
        assert!(!idt.every_entry_present());
    }

    #[test]
    fn every_entry_present_is_true_only_once_all_256_vectors_are_set() {
        let mut idt = Idt::new();
        for vector in 0..=255u8 {
            assert!(!idt.every_entry_present());
            idt.set_handler(vector, 0x1000, 0x08);
        }
        assert!(idt.every_entry_present());
    }

    #[test]
    fn handler_address_round_trips_a_full_64_bit_address() {
        let mut idt = Idt::new();
        // Exercises every byte lane (low/mid/high) with distinct nonzero
        // bits, not just a value that happens to fit in the low 16 or 32.
        idt.set_handler(0, 0xdead_beef_1122_3344, 0x08);
        assert_eq!(idt.entry(0).handler_address(), 0xdead_beef_1122_3344);
    }

    #[test]
    fn pointer_reports_the_full_table_byte_length_minus_one() {
        let idt = Idt::new();
        let (base, limit) = idt.pointer();
        assert_eq!(base, &idt as *const Idt as u64);
        assert_eq!(limit as usize, ENTRY_COUNT * 16 - 1);
    }
}
