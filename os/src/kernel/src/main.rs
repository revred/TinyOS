//! TinyOS kernel — Phase 0 walking skeleton.
//!
//! No scheduler, no memory pools, no drivers yet (those land in
//! `FEAT-P0-02`/`FEAT-P0-03`/`FEAT-P0-04`). This crate's only job right now
//! is to prove the build → boot → halt → CI pipeline works end to end, per
//! `STORY-P0-01-01`.

#![no_std]
#![no_main]
#![deny(missing_docs)]

#[cfg(feature = "fixture-context-switch")]
mod context_switch_fixture;

// `boot` (the PVH entry glue) is only referenced by the linker (via
// `ENTRY(_start)`/`KEEP(...)` in `targets/x86_64-tinyos.ld`), never by Rust
// code in this crate — hence `as _`, so importing it doesn't trip an unused
// import lint while still pulling `hal_x86_64`'s rlib (and thus `boot`'s
// object code) into this binary's link. See `hal_x86_64::boot`'s doc
// comment for why this glue moved out of `kernel` in `STORY-P0-05-02`.
#[allow(unused_imports)]
// linked for its `global_asm!` side effect only, per its own doc comment
use hal_x86_64::boot as _;
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
// `kernel::capacities::MAX_CPUS` (`STORY-P0-03-02`) — the boot-time ACPI
// topology discovery capacity bound, previously a local `const` here, now
// this crate's single reviewable capacities location. Unused (and
// `#[allow(unused_imports)]`'d) under the `fixture-broken-boot` and
// `fixture-context-switch` features, whose whole point is to reach a
// different path before ever using this — see `TEST-P0-01-03-A` and
// `TEST-P0-02-02-A`.
#[cfg_attr(
    any(feature = "fixture-broken-boot", feature = "fixture-context-switch"),
    allow(unused_imports)
)]
use kernel::capacities::MAX_CPUS;

/// Entry point reached from [`boot`]'s 64-bit long-mode transition.
///
/// `start_info_paddr` is the physical address of the PVH `hvm_start_info`
/// struct, handed to this function in `RDI` by `boot.rs` (originally `EBX`
/// at `_start`, per the PVH protocol).
///
/// Per `TEST-P0-01-01-A`: produces no unexpected output and reaches a halt
/// (here, a QEMU debug-exit with a distinguishable success code) with no
/// panic on the way. `STORY-P0-04-01`'s Tier 0 verification additionally
/// requires this to succeed only when real ACPI table parsing against
/// QEMU's own `q35` tables succeeds — a parsing regression here is
/// therefore a boot-failure exit code, not a silent no-op.
#[no_mangle]
extern "C" fn kernel_main(
    #[cfg_attr(
        any(feature = "fixture-broken-boot", feature = "fixture-context-switch"),
        allow(unused_variables)
    )]
    start_info_paddr: u64,
) -> ! {
    #[cfg(feature = "fixture-broken-boot")]
    panic!("fixture-broken-boot: deliberate panic for TEST-P0-01-03-A");

    #[cfg(feature = "fixture-context-switch")]
    {
        if context_switch_fixture::run() {
            exit_qemu(QemuExitCode::Success)
        } else {
            exit_qemu(QemuExitCode::Failure)
        }
    }

    #[cfg(not(any(feature = "fixture-broken-boot", feature = "fixture-context-switch")))]
    {
        // SAFETY: `start_info_paddr` is the physical address the PVH
        // bootloader handed this kernel in `EBX` (preserved into `RDI` by
        // `boot.rs`'s mode transition), and every ACPI table it
        // transitively points to — plus the classic BIOS-era EBDA/ROM
        // fallback search area `discover_topology` uses when
        // `hvm_start_info.rsdp_paddr` is zero — lies within the first
        // 1GiB `boot.rs` identity-maps before calling `kernel_main`.
        let topology = unsafe { hal_x86_64::acpi::discover_topology::<MAX_CPUS>(start_info_paddr) };
        match topology {
            Ok(topology) if !topology.is_empty() => exit_qemu(QemuExitCode::Success),
            _ => exit_qemu(QemuExitCode::Failure),
        }
    }
}

/// `panic!` is not error handling in RT-path code (per
/// `agent/CODING_STANDARDS.md#real-time-discipline-kernel-and-driver-code`);
/// this last-resort handler reports failure to the QEMU harness rather than
/// looping silently, so a boot-time panic is distinguishable in CI.
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    exit_qemu(QemuExitCode::Failure)
}
