//! The 64-bit Task State Segment and its Interrupt Stack Table stacks
//! (`STORY-P1-02-02`).
//!
//! Split the same way [`crate::idt`] and [`crate::fault`] already are:
//! everything here is ordinary data manipulation that compiles and runs on any
//! host, and only [`crate::gdt`]'s `lgdt`/`ltr` need a real x86_64 CPU. The
//! reason is unchanged — the CPU parses this structure as a raw byte stream at
//! architecturally fixed offsets, and a layout error is invisible to the type
//! system while corrupting every field the hardware reads.
//!
//! **Why an IST at all.** Without one, the CPU pushes an exception's frame onto
//! whatever stack is current. If that stack is the reason for the exception —
//! it is unmapped, or `RSP` has been corrupted — the push faults too, and the
//! escalation ends in a triple fault: a silent machine reset under QEMU, a
//! silent hang on a board with no `isa-debug-exit` port. The Interrupt Stack
//! Table is the hardware's answer: a gate carrying an IST index makes the CPU
//! load `RSP` from this structure *unconditionally*, before it pushes anything,
//! chosen by hardware rather than by code the compromised path might have
//! corrupted. That is why this is the safety net under `STORY-P1-02-01`'s net,
//! and why `LE-04` could not be closed by anything softer.
//!
//! **One slot is populated, not seven.** [`IstIndex::DOUBLE_FAULT`] is the only
//! IST this Story wires, because `#DF` is the only vector it gives a handler
//! that needs one. `#MC` (vector 18) is the obvious second consumer and is
//! deliberately left out: there is no Tier 0 way to raise a machine check, so
//! wiring it would put unexercised memory and an unexercised gate in a fault
//! path — the same argument `STORY-P1-02-01` used to refuse an unreachable
//! resume arm.

/// Which Interrupt Stack Table slot a gate uses — `1..=7`.
///
/// A newtype rather than a bare `u8` because the field it lands in is three
/// bits wide: a `9` written into an `ist: u8` would be silently truncated to
/// `1` and would quietly steal the double fault's own stack. `0` is rejected
/// too — it is the encoding for "no IST", which [`crate::idt::Idt::set_handler`]
/// already produces, and accepting it here would give two spellings for one
/// meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IstIndex(u8);

impl IstIndex {
    /// The slot `#DF` uses. Slot 1 by convention (nothing architectural
    /// distinguishes the seven), fixed here so the TSS and the IDT gate cannot
    /// disagree about which stack the double fault gets.
    pub const DOUBLE_FAULT: IstIndex = IstIndex(1);

    /// The valid slot numbers, `1..=7`.
    pub const MIN: u8 = 1;
    /// See [`IstIndex::MIN`].
    pub const MAX: u8 = 7;

    /// A slot number, or `None` for `0` (which means "no IST") and for
    /// anything above 7 (which the descriptor's three-bit field cannot hold).
    pub const fn try_new(index: u8) -> Option<Self> {
        if index >= Self::MIN && index <= Self::MAX {
            Some(IstIndex(index))
        } else {
            None
        }
    }

    /// The raw slot number.
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// The 64-bit Task State Segment (Intel SDM Vol 3A §8.7).
///
/// `#[repr(C, packed)]` and the reserved fields are load-bearing: the CPU reads
/// this at fixed byte offsets with no involvement from Rust's layout rules, so
/// the reserved words are not padding to be optimized away — they are part of
/// the structure hardware indexes through. The host tests pin `size_of` and
/// every offset for exactly that reason.
///
/// In long mode this segment carries no task state at all (hardware task
/// switching does not exist in 64-bit mode). It exists solely as the place the
/// CPU reads `RSP0..2` and `IST1..7` from.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TaskStateSegment {
    reserved_0: u32,
    /// Stack pointer for a transition to CPL 0, 1 and 2 respectively.
    ///
    /// All three stay **zero**, and that is a statement rather than an
    /// omission: they are only consulted on a privilege-level change, and this
    /// kernel has none — everything runs at CPL 0 in one identity-mapped
    /// address space (`STORY-P1-02-01`'s own clause 8). Putting a plausible
    /// address here would advertise a privilege boundary that does not exist.
    privilege_stacks: [u64; 3],
    reserved_1: u64,
    interrupt_stacks: [u64; 7],
    reserved_2: u64,
    reserved_3: u16,
    /// Offset of the I/O permission bitmap, from the base of this segment.
    ///
    /// Set past [`TaskStateSegment::LIMIT`] so the CPU treats this TSS as
    /// having **no** bitmap. A bitmap this kernel never populates but does
    /// advertise would make the CPU read 8 KiB of whatever memory happens to
    /// follow the TSS and treat it as an I/O permission map.
    iomap_base: u16,
}

impl TaskStateSegment {
    /// The `limit` a GDT descriptor for this segment must carry: one less than
    /// its size, per the usual descriptor-limit convention.
    pub const LIMIT: u16 = (core::mem::size_of::<TaskStateSegment>() - 1) as u16;

    /// A TSS with no IST stacks, no privilege stacks, and no I/O bitmap.
    pub const fn new() -> Self {
        TaskStateSegment {
            reserved_0: 0,
            privilege_stacks: [0; 3],
            reserved_1: 0,
            interrupt_stacks: [0; 7],
            reserved_2: 0,
            reserved_3: 0,
            // Past the limit: no bitmap. See the field's own doc comment.
            iomap_base: Self::LIMIT + 1,
        }
    }

    /// Points `index`'s IST slot at `stack_top`.
    ///
    /// `stack_top` is the address the CPU loads into `RSP`, so it is the
    /// **top** (one past the highest usable byte) of a downward-growing stack,
    /// not its base.
    ///
    /// Copies the whole array through a local on the way in and out: the field
    /// lives in a `packed` structure, so Rust will not hand out a reference to
    /// it (nor to one of its elements) at all, and a raw-pointer write here
    /// would be `unsafe` for no gain.
    pub fn set_interrupt_stack(&mut self, index: IstIndex, stack_top: u64) {
        let mut stacks = self.interrupt_stacks;
        stacks[index.get() as usize - 1] = stack_top;
        self.interrupt_stacks = stacks;
    }

    /// The stack top currently recorded for `index`, `0` when unpopulated.
    pub fn interrupt_stack(&self, index: IstIndex) -> u64 {
        let stacks = self.interrupt_stacks;
        stacks[index.get() as usize - 1]
    }

    /// How many of the seven IST slots are populated — the quantity
    /// `TEST-P1-02-02-A` clause 8 bounds at exactly one.
    pub fn populated_interrupt_stacks(&self) -> usize {
        let stacks = self.interrupt_stacks;
        stacks.iter().filter(|top| **top != 0).count()
    }

    /// This segment's base and limit, as a GDT system descriptor needs them —
    /// exposed here (rather than computed at the `lgdt` call site) so the
    /// address arithmetic is host-testable, mirroring [`crate::idt::Idt::pointer`].
    pub fn base_and_limit(&self) -> (u64, u16) {
        (self as *const TaskStateSegment as u64, Self::LIMIT)
    }
}

impl Default for TaskStateSegment {
    fn default() -> Self {
        Self::new()
    }
}

/// Bytes reserved for the `#DF` Interrupt Stack Table stack.
///
/// 16 KiB, and the rationale rather than the number is the point. The double
/// fault handler's whole job is to read a [`crate::fault::FaultFrame`], stamp
/// two spoors, format one line to COM1 and stop — a chain far shallower than
/// the 8 KiB per-task stacks `STORY-P1-02-01`'s fixture already runs its
/// handler on successfully. The doubling on top of that is headroom for this
/// workspace's unoptimized dev profile, which does not reuse stack slots across
/// call layers (the same effect that pushed `boot.rs`'s own boot stack to 1 MiB
/// when `STORY-P0-05-02` passed 4 KiB page tables by value). It is deliberately
/// *not* sized like the boot stack: nothing on this path passes a page table by
/// value, and an oversized known-good stack is memory permanently unavailable
/// to everything else.
pub const IST_STACK_BYTES: usize = 16 * 1024;

/// How many IST stacks this kernel commits storage for — one, for `#DF`. See
/// this module's own doc comment for why `#MC` does not get a second.
pub const IST_STACK_COUNT: usize = 1;

/// The `#DF` stack itself.
///
/// `static mut` for the same reason [`crate::idt`]'s table is: the CPU keeps
/// using whatever address the TSS records, for the rest of this kernel's run,
/// with no further involvement from Rust's borrow tracking. Nothing ever reads
/// or writes this array through Rust — only the CPU does, by loading `RSP` from
/// it — so it exists to reserve the address range and keep it out of every
/// other allocator's reach.
///
/// 16-byte aligned: the CPU aligns `RSP` down to 16 on IST entry anyway, and
/// starting aligned means the whole reserved range is usable rather than the
/// top few bytes being silently discarded.
#[repr(align(16))]
// Never read through Rust — only the CPU ever touches these bytes, by loading
// `RSP` from the TSS. The array exists to reserve the address range, which is
// exactly what the dead-code lint cannot see.
struct IstStack(#[allow(dead_code)] [u8; IST_STACK_BYTES]);

static mut DOUBLE_FAULT_STACK: IstStack = IstStack([0; IST_STACK_BYTES]);

/// The `#DF` IST stack's `(bottom, top)` addresses — `top` is what the TSS
/// records and the CPU loads into `RSP`.
pub fn double_fault_stack_range() -> (u64, u64) {
    let bottom = (&raw const DOUBLE_FAULT_STACK).cast::<u8>() as u64;
    (bottom, bottom + IST_STACK_BYTES as u64)
}

/// Whether `address` lies inside the `#DF` IST stack.
///
/// This is how a fixture proves the double-fault handler ran on the known-good
/// stack rather than merely producing output — see `TEST-P1-02-02-A` clause 6.
/// A handler that reported successfully from a stack that happens to still work
/// would look identical from the outside; this predicate is what tells them
/// apart.
pub fn double_fault_stack_contains(address: u64) -> bool {
    let (bottom, top) = double_fault_stack_range();
    address >= bottom && address < top
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{offset_of, size_of};

    // Clause 1: the CPU indexes this structure at architecturally fixed byte
    // offsets (Intel SDM Vol 3A §8.7). Nothing in the type system checks that.
    #[test]
    fn the_task_state_segment_matches_the_architectural_layout_exactly() {
        assert_eq!(size_of::<TaskStateSegment>(), 104);
        assert_eq!(offset_of!(TaskStateSegment, privilege_stacks), 4, "RSP0 at offset 4");
        assert_eq!(offset_of!(TaskStateSegment, interrupt_stacks), 36, "IST1 at offset 36");
        assert_eq!(offset_of!(TaskStateSegment, iomap_base), 102);
        // IST7 is the last of the seven, 8 bytes each from IST1.
        assert_eq!(offset_of!(TaskStateSegment, interrupt_stacks) + 6 * 8, 84);
        assert_eq!(TaskStateSegment::LIMIT, 103);
    }

    // Clause 1: an advertised-but-unpopulated I/O bitmap would make the CPU
    // read 8KiB of whatever follows this segment and honor it as permissions.
    #[test]
    fn no_io_permission_bitmap_is_advertised() {
        let tss = TaskStateSegment::new();
        let iomap_base = tss.iomap_base;
        assert!(
            iomap_base > TaskStateSegment::LIMIT,
            "iomap_base {iomap_base} must lie past the segment limit {}",
            TaskStateSegment::LIMIT
        );
    }

    // Clause 1: zero, and stated as a claim this kernel cannot back rather
    // than as an oversight.
    #[test]
    fn no_privilege_level_stack_is_claimed() {
        let tss = TaskStateSegment::new();
        // Copied out first: a reference to a field of a packed struct is not
        // something Rust will hand out, and `assert_eq!` would take one.
        let privilege_stacks = tss.privilege_stacks;
        assert_eq!(privilege_stacks, [0; 3]);
    }

    // Clause 3: the descriptor field is three bits wide, so an out-of-range
    // index would truncate — 9 would silently become 1 and steal `#DF`'s own
    // stack. The type is what stops that, not a comment.
    #[test]
    fn an_ist_index_outside_one_through_seven_cannot_be_constructed() {
        assert_eq!(IstIndex::try_new(0), None, "0 means `no IST`, not a slot");
        for index in 1..=7u8 {
            assert_eq!(IstIndex::try_new(index).map(IstIndex::get), Some(index));
        }
        for index in [8u8, 9, 15, 16, 128, 255] {
            assert_eq!(IstIndex::try_new(index), None, "slot {index} does not exist");
        }
    }

    #[test]
    fn the_double_fault_slot_is_a_valid_slot() {
        assert_eq!(IstIndex::try_new(IstIndex::DOUBLE_FAULT.get()), Some(IstIndex::DOUBLE_FAULT));
        assert_eq!(IstIndex::DOUBLE_FAULT.get(), 1);
    }

    #[test]
    fn setting_one_ist_slot_leaves_every_other_slot_zero() {
        let mut tss = TaskStateSegment::new();
        assert_eq!(tss.populated_interrupt_stacks(), 0);
        let slot = IstIndex::try_new(3).expect("3 is a valid slot");
        tss.set_interrupt_stack(slot, 0xffff_8000_dead_0000);
        assert_eq!(tss.interrupt_stack(slot), 0xffff_8000_dead_0000);
        assert_eq!(tss.populated_interrupt_stacks(), 1);
        for other in 1..=7u8 {
            if other == 3 {
                continue;
            }
            let other = IstIndex::try_new(other).expect("a valid slot");
            assert_eq!(tss.interrupt_stack(other), 0, "slot {} must stay zero", other.get());
        }
    }

    // Clause 8: exactly one slot, and the test says so rather than the caller
    // remembering.
    #[test]
    fn the_double_fault_slot_is_the_only_one_this_kernel_populates() {
        let mut tss = TaskStateSegment::new();
        let (_, top) = double_fault_stack_range();
        tss.set_interrupt_stack(IstIndex::DOUBLE_FAULT, top);
        assert_eq!(tss.populated_interrupt_stacks(), IST_STACK_COUNT);
    }

    #[test]
    fn base_and_limit_report_this_segments_own_address_and_size() {
        let tss = TaskStateSegment::new();
        let (base, limit) = tss.base_and_limit();
        assert_eq!(base, &tss as *const TaskStateSegment as u64);
        assert_eq!(limit as usize, size_of::<TaskStateSegment>() - 1);
    }

    // Clause 6: this predicate is the fixture's whole proof that the handler
    // ran on the known-good stack, so its own boundaries are pinned here.
    #[test]
    fn the_stack_range_predicate_is_half_open_and_covers_the_whole_reservation() {
        let (bottom, top) = double_fault_stack_range();
        assert_eq!(top - bottom, IST_STACK_BYTES as u64);
        assert!(double_fault_stack_contains(bottom));
        assert!(double_fault_stack_contains(top - 1));
        // `top` is what the CPU loads into `RSP`, i.e. one past the last
        // usable byte — it is not itself inside the stack.
        assert!(!double_fault_stack_contains(top));
        assert!(!double_fault_stack_contains(bottom - 1));
        assert!(!double_fault_stack_contains(0));
    }
}
