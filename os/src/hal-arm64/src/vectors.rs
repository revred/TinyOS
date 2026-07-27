//! The AArch64 exception vector table (`TEST-P1-07-02-A` clause 1).
//!
//! **This is not an IDT, and the difference is the whole design of this
//! module.** On x86_64 the table is *data*: 256 sixteen-byte descriptors, so
//! `hal_x86_64::idt::Idt::every_entry_present` can read the array and answer
//! the question directly. On AArch64 the table is *code*: sixteen slots of 128
//! bytes each, and the CPU branches to `VBAR_EL1 + slot × 0x80` without reading
//! any descriptor at all. There is nothing to inspect at run time, so
//! "every entry is present" has to be established two other ways, both of them
//! before the board ever runs:
//!
//! 1. **At assembly time**, by `.org` directives that place every one of the
//!    sixteen slots at its architectural offset. An entry whose code overflows
//!    its 128 bytes moves the next one, and `.org` refuses to move backwards —
//!    so the assembler, not a test, rejects it. The same directives are what
//!    makes clause 1's 128-byte requirement a *build-time* assertion.
//! 2. **On the host**, by this module's [`VectorTable`] model: the sixteen
//!    slots the architecture defines, in order, with the routing each one has
//!    and an assertion that none is unrouted.
//!
//! The residual risk is stated rather than hidden: the model and the assembly
//! are two statements of the same fact, and only the *stride* is machine-checked
//! between them. That is the same residual `hal_x86_64::fault`'s own frame
//! layout carries — `#[repr(C)]` field order versus the stubs' push order — and
//! it is why both live behind a host test that pins the shape.
//!
//! **Alignment is not merely a requirement, it is a silent one.** A `VBAR_EL1`
//! write whose low eleven bits are not zero is *architecturally ignored*: no
//! fault, no error, and the handler simply never runs. That failure presents as
//! "the board said nothing", which is the exact symptom this Story exists to
//! eliminate — arriving through the Story's own front door. Hence
//! [`vbar_is_aligned`] and the `const` assertions below.

/// How many entries the architecture defines. Fixed by the CPU, not a
/// caller-tunable capacity — the same "hardware constant, not a
/// `kernel::capacities` entry" precedent `hal_x86_64::idt::ENTRY_COUNT` set.
pub const ENTRY_COUNT: usize = 16;

/// Bytes between consecutive vector entries: `0x80`.
///
/// The CPU computes each handler's address as `VBAR_EL1 + slot × 0x80`. It is a
/// stride, not a size hint — an entry is permitted to use fewer bytes, never
/// more, and using more silently displaces every entry after it.
pub const ENTRY_STRIDE_BYTES: usize = 0x80;

/// Total size of the table: `ENTRY_COUNT × ENTRY_STRIDE_BYTES` = 2 KiB.
pub const TABLE_SIZE_BYTES: usize = ENTRY_COUNT * ENTRY_STRIDE_BYTES;

/// Alignment `VBAR_EL1` requires of the table base: 2 KiB.
///
/// `VBAR_EL1[10:0]` are `RES0`. Writing a base with any of them set does not
/// fault; the bits are ignored, and the table is then read from an address
/// nobody chose.
pub const TABLE_ALIGNMENT_BYTES: usize = 0x800;

// Build-time, in the Rust half. The assembly half's own `.balign`/`.org`
// directives are in `vector_table` below; these three exist so that a change to
// one of the constants above cannot silently disagree with them.
const _: () = assert!(TABLE_SIZE_BYTES == 2048);
const _: () = assert!(TABLE_ALIGNMENT_BYTES >= TABLE_SIZE_BYTES);
const _: () = assert!(TABLE_ALIGNMENT_BYTES.is_power_of_two());

/// Which of the four architectural entry groups a slot belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntrySource {
    /// Current exception level, using `SP_EL0` (`EL1t`).
    ///
    /// Unreachable in this Feature: [`crate::boot`]'s `eret` selects `EL1h`, so
    /// the four slots in this group should never fire. They are filled anyway —
    /// "should never fire" and "is not routed" are the same silence, and the
    /// first is a claim about code while the second is a property of the table.
    CurrentElSp0,
    /// Current exception level, using `SP_ELx` (`EL1h`) — where this Feature
    /// runs, and therefore where its faults arrive.
    CurrentElSpx,
    /// A lower exception level in AArch64. No `EL0` exists in this Feature.
    LowerElAarch64,
    /// A lower exception level in AArch32. Nothing in TinyOS is AArch32.
    LowerElAarch32,
}

/// Which of the four exception kinds a slot handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// Synchronous — the aborts, traps and `BRK`s this Story decodes.
    Synchronous,
    /// IRQ. Masked throughout this Feature; no GIC exists.
    Irq,
    /// FIQ. Masked throughout this Feature.
    Fiq,
    /// `SError` — asynchronous external abort. Never expected, always terminal.
    SError,
}

/// One of the sixteen architectural vector slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VectorSlot {
    /// Which group the slot belongs to.
    pub source: EntrySource,
    /// Which exception kind the slot handles.
    pub kind: EntryKind,
}

/// What this Story wired a slot to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routing {
    /// The synchronous handler that decodes `ESR_EL1` and reports it
    /// (`TEST-P1-07-02-A` clause 3).
    Decoded,
    /// The one shared fail-closed default: report which slot fired, and halt.
    ///
    /// Exactly as `STORY-P0-04-02`'s default does on x86_64. This Story narrows
    /// the set of faults that are terminal for the whole system; it does not
    /// widen the set that is silent, which stays empty.
    FailClosedDefault,
}

/// The routing of all sixteen slots — the host-side model of the assembly
/// table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VectorTable {
    routed: [Option<Routing>; ENTRY_COUNT],
}

impl VectorSlot {
    /// The sixteen slots, in the architectural order the CPU indexes them.
    ///
    /// Written out rather than generated from the two enums' cross product:
    /// the order is the CPU's and a generated one would be this code's, which
    /// is a difference nothing would notice until a synchronous abort was
    /// reported as an `SError`.
    pub const ALL: [VectorSlot; ENTRY_COUNT] = [
        VectorSlot { source: EntrySource::CurrentElSp0, kind: EntryKind::Synchronous },
        VectorSlot { source: EntrySource::CurrentElSp0, kind: EntryKind::Irq },
        VectorSlot { source: EntrySource::CurrentElSp0, kind: EntryKind::Fiq },
        VectorSlot { source: EntrySource::CurrentElSp0, kind: EntryKind::SError },
        VectorSlot { source: EntrySource::CurrentElSpx, kind: EntryKind::Synchronous },
        VectorSlot { source: EntrySource::CurrentElSpx, kind: EntryKind::Irq },
        VectorSlot { source: EntrySource::CurrentElSpx, kind: EntryKind::Fiq },
        VectorSlot { source: EntrySource::CurrentElSpx, kind: EntryKind::SError },
        VectorSlot { source: EntrySource::LowerElAarch64, kind: EntryKind::Synchronous },
        VectorSlot { source: EntrySource::LowerElAarch64, kind: EntryKind::Irq },
        VectorSlot { source: EntrySource::LowerElAarch64, kind: EntryKind::Fiq },
        VectorSlot { source: EntrySource::LowerElAarch64, kind: EntryKind::SError },
        VectorSlot { source: EntrySource::LowerElAarch32, kind: EntryKind::Synchronous },
        VectorSlot { source: EntrySource::LowerElAarch32, kind: EntryKind::Irq },
        VectorSlot { source: EntrySource::LowerElAarch32, kind: EntryKind::Fiq },
        VectorSlot { source: EntrySource::LowerElAarch32, kind: EntryKind::SError },
    ];

    /// The synchronous slot faults taken at `EL1h` arrive through — the one
    /// slot this Story decodes.
    pub const SYNCHRONOUS_EL1H: VectorSlot =
        VectorSlot { source: EntrySource::CurrentElSpx, kind: EntryKind::Synchronous };

    /// This slot's index, `0..16`.
    pub const fn index(self) -> usize {
        let group = match self.source {
            EntrySource::CurrentElSp0 => 0,
            EntrySource::CurrentElSpx => 1,
            EntrySource::LowerElAarch64 => 2,
            EntrySource::LowerElAarch32 => 3,
        };
        let within = match self.kind {
            EntryKind::Synchronous => 0,
            EntryKind::Irq => 1,
            EntryKind::Fiq => 2,
            EntryKind::SError => 3,
        };
        group * 4 + within
    }

    /// This slot's byte offset from the table base.
    pub const fn byte_offset(self) -> usize {
        self.index() * ENTRY_STRIDE_BYTES
    }

    /// The slot at `index`, or `None` past the sixteenth.
    ///
    /// `None` rather than a wrapped index: the caller of this is the exception
    /// entry point, handed a slot number by assembly, and a slot number that
    /// is out of range means the table and this code disagree. Reporting it as
    /// slot 0 would be a confident, wrong answer about which vector fired.
    pub const fn from_index(index: usize) -> Option<VectorSlot> {
        if index >= ENTRY_COUNT {
            return None;
        }
        Some(VectorSlot::ALL[index])
    }

    /// The short name used in a serial report.
    ///
    /// A fixed string, for the reason
    /// [`crate::esr::ExceptionClass::as_str`] gives.
    pub const fn name(self) -> &'static str {
        match (self.source, self.kind) {
            (EntrySource::CurrentElSp0, EntryKind::Synchronous) => "cur_el_sp0/sync",
            (EntrySource::CurrentElSp0, EntryKind::Irq) => "cur_el_sp0/irq",
            (EntrySource::CurrentElSp0, EntryKind::Fiq) => "cur_el_sp0/fiq",
            (EntrySource::CurrentElSp0, EntryKind::SError) => "cur_el_sp0/serror",
            (EntrySource::CurrentElSpx, EntryKind::Synchronous) => "cur_el_spx/sync",
            (EntrySource::CurrentElSpx, EntryKind::Irq) => "cur_el_spx/irq",
            (EntrySource::CurrentElSpx, EntryKind::Fiq) => "cur_el_spx/fiq",
            (EntrySource::CurrentElSpx, EntryKind::SError) => "cur_el_spx/serror",
            (EntrySource::LowerElAarch64, EntryKind::Synchronous) => "lower_el_a64/sync",
            (EntrySource::LowerElAarch64, EntryKind::Irq) => "lower_el_a64/irq",
            (EntrySource::LowerElAarch64, EntryKind::Fiq) => "lower_el_a64/fiq",
            (EntrySource::LowerElAarch64, EntryKind::SError) => "lower_el_a64/serror",
            (EntrySource::LowerElAarch32, EntryKind::Synchronous) => "lower_el_a32/sync",
            (EntrySource::LowerElAarch32, EntryKind::Irq) => "lower_el_a32/irq",
            (EntrySource::LowerElAarch32, EntryKind::Fiq) => "lower_el_a32/fiq",
            (EntrySource::LowerElAarch32, EntryKind::SError) => "lower_el_a32/serror",
        }
    }
}

impl VectorTable {
    /// A table with nothing routed — the state
    /// [`VectorTable::every_entry_present`] must return `false` for.
    pub const fn empty() -> VectorTable {
        VectorTable { routed: [None; ENTRY_COUNT] }
    }

    /// The table this crate's assembly actually installs.
    ///
    /// One decoded entry and fifteen fail-closed ones. The fifteen are not
    /// filler: an `IRQ` cannot fire (`DAIF` stays masked and no GIC is
    /// programmed), an `EL0` entry cannot fire (there is no `EL0`), and an
    /// AArch32 entry cannot fire (nothing here is AArch32) — but "cannot fire"
    /// is a claim about the rest of the system, and this table is what holds
    /// when that claim turns out to be wrong.
    pub const fn installed() -> VectorTable {
        let mut table = VectorTable::empty();
        let mut index = 0;
        while index < ENTRY_COUNT {
            table.routed[index] = Some(Routing::FailClosedDefault);
            index += 1;
        }
        table.routed[VectorSlot::SYNCHRONOUS_EL1H.index()] = Some(Routing::Decoded);
        table
    }

    /// Routes `slot`.
    pub const fn route(mut self, slot: VectorSlot, routing: Routing) -> VectorTable {
        self.routed[slot.index()] = Some(routing);
        self
    }

    /// What `slot` is routed to, or `None` if it is unrouted.
    pub const fn routing(&self, slot: VectorSlot) -> Option<Routing> {
        self.routed[slot.index()]
    }

    /// Whether every one of the sixteen slots is routed — clause 1's
    /// "all sixteen vectors are present", and the direct analogue of
    /// `hal_x86_64::idt::Idt::every_entry_present`.
    ///
    /// A table this returns `false` for must never be installed: an unrouted
    /// slot means the CPU branches into whatever bytes happen to lie at that
    /// offset, which is an unfilled vector by another name.
    pub fn every_entry_present(&self) -> bool {
        self.routed.iter().all(Option::is_some)
    }
}

/// Whether `base` may be written to `VBAR_EL1`.
///
/// The check exists because the failure it prevents is invisible: a misaligned
/// write is ignored, not rejected. See this module's documentation.
pub const fn vbar_is_aligned(base: u64) -> bool {
    base & (TABLE_ALIGNMENT_BYTES as u64 - 1) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    // Clause 1: all sixteen, and the arithmetic the CPU itself performs.
    #[test]
    fn there_are_exactly_sixteen_slots_at_a_128_byte_stride() {
        assert_eq!(VectorSlot::ALL.len(), ENTRY_COUNT);
        assert_eq!(ENTRY_STRIDE_BYTES, 0x80);
        assert_eq!(TABLE_SIZE_BYTES, 0x800);
        for (index, slot) in VectorSlot::ALL.iter().enumerate() {
            assert_eq!(slot.index(), index);
            assert_eq!(slot.byte_offset(), index * 0x80);
            assert_eq!(VectorSlot::from_index(index), Some(*slot));
        }
        assert_eq!(VectorSlot::from_index(ENTRY_COUNT), None);
        assert_eq!(VectorSlot::ALL[ENTRY_COUNT - 1].byte_offset(), TABLE_SIZE_BYTES - 0x80);
    }

    #[test]
    fn the_sixteen_slots_are_the_architectures_four_groups_of_four_in_order() {
        // The order is the CPU's, not a convention this code chose: group by
        // source, then synchronous/IRQ/FIQ/SError within each. Getting it wrong
        // routes a synchronous abort to the SError entry, which reports the
        // wrong thing with total confidence.
        let expected = [
            (EntrySource::CurrentElSp0, EntryKind::Synchronous),
            (EntrySource::CurrentElSp0, EntryKind::Irq),
            (EntrySource::CurrentElSp0, EntryKind::Fiq),
            (EntrySource::CurrentElSp0, EntryKind::SError),
            (EntrySource::CurrentElSpx, EntryKind::Synchronous),
            (EntrySource::CurrentElSpx, EntryKind::Irq),
            (EntrySource::CurrentElSpx, EntryKind::Fiq),
            (EntrySource::CurrentElSpx, EntryKind::SError),
            (EntrySource::LowerElAarch64, EntryKind::Synchronous),
            (EntrySource::LowerElAarch64, EntryKind::Irq),
            (EntrySource::LowerElAarch64, EntryKind::Fiq),
            (EntrySource::LowerElAarch64, EntryKind::SError),
            (EntrySource::LowerElAarch32, EntryKind::Synchronous),
            (EntrySource::LowerElAarch32, EntryKind::Irq),
            (EntrySource::LowerElAarch32, EntryKind::Fiq),
            (EntrySource::LowerElAarch32, EntryKind::SError),
        ];
        for (index, (source, kind)) in expected.into_iter().enumerate() {
            assert_eq!(VectorSlot::ALL[index], VectorSlot { source, kind }, "slot {index}");
        }
    }

    #[test]
    fn every_slot_renders_a_distinct_name() {
        // The name is the only thing a capture carries about *which* of the
        // sixteen fired, so two slots sharing one would make a report
        // ambiguous exactly where it is most needed.
        for (index, slot) in VectorSlot::ALL.iter().enumerate() {
            assert!(!slot.name().is_empty());
            for other in VectorSlot::ALL.iter().skip(index + 1) {
                assert_ne!(slot.name(), other.name(), "{slot:?} and {other:?} collide");
            }
        }
    }

    // Clause 1: the `Idt::every_entry_present` analogue, including the state it
    // must reject.
    #[test]
    fn a_fresh_table_has_nothing_routed() {
        let table = VectorTable::empty();
        assert!(!table.every_entry_present());
        for slot in VectorSlot::ALL {
            assert_eq!(table.routing(slot), None, "{slot:?}");
        }
    }

    #[test]
    fn every_entry_present_is_true_only_once_all_sixteen_are_routed() {
        let mut table = VectorTable::empty();
        for slot in VectorSlot::ALL {
            assert!(!table.every_entry_present());
            table = table.route(slot, Routing::FailClosedDefault);
        }
        assert!(table.every_entry_present());
    }

    #[test]
    fn leaving_any_single_slot_unrouted_is_caught() {
        // The failure mode is one forgotten entry, not sixteen — so the test
        // that matters is per-slot, not "the empty table is rejected".
        for missing in VectorSlot::ALL {
            let mut table = VectorTable::empty();
            for slot in VectorSlot::ALL {
                if slot != missing {
                    table = table.route(slot, Routing::FailClosedDefault);
                }
            }
            assert!(
                !table.every_entry_present(),
                "an unrouted {missing:?} must not pass as a complete table"
            );
        }
    }

    // Clause 1: what this Story decodes, and that everything else still lands
    // somewhere explicit.
    #[test]
    fn the_installed_table_routes_all_sixteen_and_decodes_only_the_synchronous_el1h_entry() {
        let table = VectorTable::installed();
        assert!(table.every_entry_present());

        let decoded =
            VectorSlot { source: EntrySource::CurrentElSpx, kind: EntryKind::Synchronous };
        assert_eq!(table.routing(decoded), Some(Routing::Decoded));

        for slot in VectorSlot::ALL {
            let routing = table.routing(slot).expect("every slot is routed");
            if slot == decoded {
                continue;
            }
            assert_eq!(
                routing,
                Routing::FailClosedDefault,
                "{slot:?} must reach the shared fail-closed default"
            );
        }
    }

    #[test]
    fn exactly_one_slot_is_decoded_so_the_silent_set_stays_empty() {
        // Clause 1's second paragraph, as arithmetic: one decoded entry,
        // fifteen fail-closed, zero unrouted. A Story that widened the decoded
        // set without saying so would change this count.
        let table = VectorTable::installed();
        let decoded = VectorSlot::ALL
            .iter()
            .filter(|slot| table.routing(**slot) == Some(Routing::Decoded))
            .count();
        let default = VectorSlot::ALL
            .iter()
            .filter(|slot| table.routing(**slot) == Some(Routing::FailClosedDefault))
            .count();
        assert_eq!(decoded, 1);
        assert_eq!(default, ENTRY_COUNT - 1);
        assert_eq!(decoded + default, ENTRY_COUNT);
    }

    // Clause 1: the alignment requirement, and the reason it is not a run-time
    // check on the board.
    #[test]
    fn a_vbar_base_is_rejected_unless_its_low_eleven_bits_are_clear() {
        assert!(vbar_is_aligned(0x0008_0000));
        assert!(vbar_is_aligned(0));
        assert!(vbar_is_aligned(TABLE_ALIGNMENT_BYTES as u64));
        // Every single low bit, one at a time: a mask that was one bit too
        // narrow would accept exactly one of these.
        for bit in 0..11 {
            assert!(
                !vbar_is_aligned(0x0008_0000 | (1 << bit)),
                "bit {bit} set must not pass as aligned"
            );
        }
        // 128-byte aligned is not enough. This is the specific near-miss the
        // check exists for: the entry stride and the table alignment are
        // different numbers, and using the former is the natural mistake.
        assert!(!vbar_is_aligned(0x0008_0000 + ENTRY_STRIDE_BYTES as u64));
    }

    #[test]
    fn the_table_fits_exactly_within_its_own_alignment() {
        // If the table were larger than its required alignment, a correctly
        // aligned base would still let the last entry cross into the next
        // aligned block — which is a silent failure, not a fault.
        // A `const` block, not a run-time assertion: both operands are
        // constants, so a change that broke this should fail to *build* rather
        // than fail a test run. The same reading of
        // `clippy::assertions_on_constants` that `74d3904` applied to
        // `board.rs`.
        const { assert!(TABLE_SIZE_BYTES <= TABLE_ALIGNMENT_BYTES) };
        assert_eq!(TABLE_SIZE_BYTES, ENTRY_COUNT * ENTRY_STRIDE_BYTES);
    }
}
