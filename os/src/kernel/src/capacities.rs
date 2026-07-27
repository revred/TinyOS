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
}
