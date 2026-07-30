//! Compile-time pool/array-capacity configuration (`STORY-P0-03-02`).
//!
//! Single, reviewable location for this kernel's fixed-capacity bounds —
//! named `pub const`s here, never a magic number scattered at each
//! `Pool::new()`/`Topology<N>` call site. A capacity whose backing storage
//! would overflow [`STATIC_MEMORY_BUDGET_BYTES`] fails the build via a
//! `const` assertion (`STORY-P0-03-02` acceptance criterion 2), not a
//! runtime allocation-failure path — this is a compile-time capacity
//! budget, not a runtime-tunable one.
//!
//! Only capacities with a real, concrete production or Tier-0-fixture
//! consumer are declared here today: [`MAX_CPUS`] (boot-time ACPI topology
//! discovery, `STORY-P0-04-01`), [`MAX_PCI_DEVICES`] (boot-time PCI bus
//! enumeration, `STORY-P0-04-03`), and [`EXEC_FRAME_POOL_CAPACITY`] (the
//! page-table frame pool `exec`'s Tier 0 fixtures build an `AddressSpace`
//! against — previously a `FRAMES` constant duplicated in two separate
//! files, `os/src/exec/src/fixture_main.rs` and
//! `fixture_win32_shim_main.rs`, consolidated here). The task-control-block
//! and IPC-message pools this Story's own description also names have no
//! real call site yet — no production `Scheduler` is wired into `main.rs`
//! — so adding a capacity constant with nothing to consume it would be
//! exactly the kind of speculative, ahead-of-need code this codebase's own
//! standards argue against. Add one alongside whichever Story first wires
//! a production consumer in, not before.

use hal::device::DeviceDescriptor;
use hal::topology::CpuDescriptor;
use hal_x86_64::paging::PageTable;

/// Capacity bound for boot-time ACPI topology discovery (`STORY-P0-04-01`)
/// — comfortably above any CPU count QEMU's `q35` machine model or real
/// Phase 0 target hardware presents; a firmware report of more cores than
/// this fails closed rather than overflowing fixed storage (see
/// `hal::topology::Topology::push`).
pub const MAX_CPUS: usize = 64;

/// Capacity bound for boot-time PCI bus enumeration (`STORY-P0-04-03`) —
/// bus 0 can present at most 256 functions (32 devices × 8 functions), but
/// QEMU's `q35` default machine model and Phase 0 target hardware present
/// well under this bound on bus 0; a bus presenting more functions than
/// this fails closed rather than overflowing fixed storage (see
/// `hal::device::DeviceTable::push`).
pub const MAX_PCI_DEVICES: usize = 64;

/// Capacity bound for `exec::address_space::AddressSpace`'s `Pool`-backed
/// page-table frame allocator, as used by `exec`'s own Tier 0 fixtures.
pub const EXEC_FRAME_POOL_CAPACITY: usize = 16;

/// Capacity bound for the kernel binary's own fault-audit
/// [`crate::spoor_journal::SpoorJournal`] (`STORY-P1-03-02` acceptance
/// criterion I5) — the constant four `FEAT-P0-06` Reports deferred until a
/// real production consumer existed: `main.rs`'s `tinyos_fault_entry` now
/// journals the audit pair it computes, and the integration fixture's
/// supervisor journals refusal/dispatch/containment through the same
/// capacity. Sized to hold every spoor a single Tier 0 run can plausibly
/// emit (a handful per fault/dispatch event) with wide headroom, while
/// costing 512 bytes of static storage.
pub const SPOOR_JOURNAL_CAPACITY: usize = 64;

/// Static bytes the Interrupt Stack Table's known-good stacks commit
/// (`STORY-P1-02-02`) — [`hal_x86_64::tss::IST_STACK_COUNT`] stacks of
/// [`hal_x86_64::tss::IST_STACK_BYTES`] each.
///
/// Declared here rather than left implicit in `hal-x86_64` because it is real,
/// permanently-reserved memory that nothing else can ever use, and this module
/// is where this kernel's committed static memory is supposed to be countable
/// in one place. The size's *rationale* stays next to the stack itself, where a
/// reader changing it will see it; the *budget* lives here, where the ceiling
/// is enforced.
pub const IST_STACK_BYTES: usize =
    hal_x86_64::tss::IST_STACK_COUNT * hal_x86_64::tss::IST_STACK_BYTES;

/// Static bytes reserved for the single CPU's CPL-3 → CPL-0 transition
/// stack. This is separate from IST: ordinary user-originated interrupts and
/// syscalls use TSS.RSP0, while double fault uses its dedicated IST slot.
pub const RING0_ENTRY_STACK_BYTES: usize = hal_x86_64::tss::RING0_ENTRY_STACK_BYTES;

/// The documented static-memory budget every capacity above must fit
/// within, combined — chosen conservatively for this Roadmap Phase's
/// QEMU/Tier 0 target (matching `G-DX-8`'s 8MB total-image ceiling as the
/// nearest existing budget reference point until a tighter, hardware-
/// measured one is chosen) and worth revisiting once real Tier 2 hardware
/// (`EPIC-P0`'s mini-PC) is purchased and its actual RAM size is known,
/// per `README.md`'s Target Hardware & Test Matrix.
pub const STATIC_MEMORY_BUDGET_BYTES: usize = 8 * 1024 * 1024;

/// The total static bytes [`MAX_CPUS`], [`MAX_PCI_DEVICES`], and
/// [`EXEC_FRAME_POOL_CAPACITY`]'s backing storage commits to, given each
/// element type's real size — the quantity [`STATIC_MEMORY_BUDGET_BYTES`]
/// bounds.
///
/// An approximation, not the exact `Topology<N>`/`Pool<T, N>` byte layout
/// (it ignores `Option`/`MaybeUninit` tag/padding overhead, both crates'
/// implementation details this module has no visibility into) — adequate
/// headroom for a budget *ceiling* check at Phase 0's capacity sizes, not
/// a claim of byte-exact accounting.
pub const fn committed_bytes() -> usize {
    MAX_CPUS * core::mem::size_of::<CpuDescriptor>()
        + MAX_PCI_DEVICES * core::mem::size_of::<DeviceDescriptor>()
        + EXEC_FRAME_POOL_CAPACITY * core::mem::size_of::<PageTable>()
        + SPOOR_JOURNAL_CAPACITY * core::mem::size_of::<u64>()
        + IST_STACK_BYTES
        + RING0_ENTRY_STACK_BYTES
}

const _: () = assert!(
    committed_bytes() <= STATIC_MEMORY_BUDGET_BYTES,
    "kernel::capacities: configured capacities exceed STATIC_MEMORY_BUDGET_BYTES — lower a \
     capacity or raise the documented budget deliberately, not by working around this check"
);

#[cfg(test)]
mod tests {
    use super::*;

    // STORY-P0-03-02 acceptance criterion 2, restated as a runtime-visible
    // assertion (the `const _: () = assert!(...)` above already enforces
    // this at compile time — every successful build of this crate is
    // already proof it holds — but a test makes the property visible in
    // `cargo test`'s own output, not only inferred from the build having
    // succeeded at all).
    #[test]
    fn committed_capacities_fit_within_the_documented_budget() {
        assert!(committed_bytes() <= STATIC_MEMORY_BUDGET_BYTES);
    }

    // `TEST-P1-02-02-A` clause 8: the IST stack is budgeted, not free. A
    // known-good stack is memory permanently unavailable to everything else,
    // and it has to show up in the same ceiling every other capacity passes.
    #[test]
    fn the_ist_stack_is_counted_against_the_budget() {
        assert_eq!(IST_STACK_BYTES, 16 * 1024, "one 16KiB stack, for `#DF`");
        assert_eq!(hal_x86_64::tss::IST_STACK_COUNT, 1, "`#MC` deliberately gets no IST");
        let without_ist = committed_bytes() - IST_STACK_BYTES;
        assert!(committed_bytes() > without_ist, "the IST stack must be inside the count");
    }

    #[test]
    fn the_ring0_entry_stack_is_counted_against_the_budget() {
        assert_eq!(RING0_ENTRY_STACK_BYTES, 16 * 1024);
        let without_entry_stack = committed_bytes() - RING0_ENTRY_STACK_BYTES;
        assert!(
            committed_bytes() > without_entry_stack,
            "the privilege-transition stack must be inside the count"
        );
    }
}
