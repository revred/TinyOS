//! TinyOS kernel — Phase 0 walking skeleton.
//!
//! No scheduler, no memory pools yet (those land in `FEAT-P0-02`/
//! `FEAT-P0-03`). `STORY-P0-04-02` (IDT/local-APIC timer bring-up) closes
//! part of `FEAT-P0-04`; this crate's remaining core job is to prove the
//! build → boot → halt → CI pipeline works end to end, per `STORY-P0-01-01`.

#![no_std]
#![no_main]
#![deny(missing_docs)]

#[cfg(feature = "fixture-context-switch")]
mod context_switch_fixture;
#[cfg(feature = "fixture-idt-apic-timer")]
mod fixture_idt_apic_timer;
#[cfg(feature = "fixture-idt-apic-unrouted")]
mod fixture_idt_apic_unrouted;
#[cfg(feature = "fixture-measure")]
mod fixture_measure;
#[cfg(feature = "fixture-pci-enumeration")]
mod fixture_pci_enumeration;
#[cfg(feature = "fixture-pool-bench")]
mod fixture_pool_bench;

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
// `#[allow(unused_imports)]`'d) under every fixture feature, whose whole
// point is to reach a different path before ever using this — see
// `TEST-P0-01-03-A`, `TEST-P0-02-02-A`, and `TEST-P0-04-02-A`.
#[cfg_attr(
    any(
        feature = "fixture-broken-boot",
        feature = "fixture-context-switch",
        feature = "fixture-idt-apic-timer",
        feature = "fixture-idt-apic-unrouted",
        feature = "fixture-pci-enumeration",
        feature = "fixture-pool-bench",
        feature = "fixture-measure"
    ),
    allow(unused_imports)
)]
use kernel::capacities::MAX_CPUS;

/// Local-APIC timer reload value armed on the real (non-fixture) boot path
/// — see `hal_x86_64::interrupts::configure_timer`'s own doc comment for
/// what this counts down against. No production code reads
/// `hal_x86_64::interrupts::tick_count()` yet (no scheduler dispatch loop
/// exists to consume it) — arming the timer here still closes a real gap
/// (an IDT with a fail-closed default for every vector is now this
/// kernel's actual boot-time state, not just a fixture-only demonstration),
/// while ticking with no consumer is itself inert and safe, matching this
/// codebase's own "wire the primitive, don't invent a speculative consumer"
/// discipline (`STORY-P0-03-02`'s precedent for capacity constants, applied
/// here to a HAL primitive instead).
#[cfg(not(any(
    feature = "fixture-broken-boot",
    feature = "fixture-context-switch",
    feature = "fixture-idt-apic-timer",
    feature = "fixture-idt-apic-unrouted",
    feature = "fixture-pci-enumeration",
    feature = "fixture-pool-bench",
    feature = "fixture-measure"
)))]
const BOOT_TIMER_INITIAL_COUNT: u32 = 1_000_000;

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
        any(
            feature = "fixture-broken-boot",
            feature = "fixture-context-switch",
            feature = "fixture-idt-apic-timer",
            feature = "fixture-idt-apic-unrouted",
            feature = "fixture-pci-enumeration",
            feature = "fixture-pool-bench",
            feature = "fixture-measure"
        ),
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

    #[cfg(feature = "fixture-idt-apic-timer")]
    {
        if fixture_idt_apic_timer::run() {
            exit_qemu(QemuExitCode::Success)
        } else {
            exit_qemu(QemuExitCode::Failure)
        }
    }

    #[cfg(feature = "fixture-idt-apic-unrouted")]
    {
        // Never expected to return — see `fixture_idt_apic_unrouted`'s own
        // doc comment for why reaching this fixture's fail-closed default
        // handler (not this line) is its actual pass condition.
        fixture_idt_apic_unrouted::run()
    }

    #[cfg(feature = "fixture-pci-enumeration")]
    {
        if fixture_pci_enumeration::run() {
            exit_qemu(QemuExitCode::Success)
        } else {
            exit_qemu(QemuExitCode::Failure)
        }
    }

    #[cfg(feature = "fixture-pool-bench")]
    {
        if fixture_pool_bench::run() {
            exit_qemu(QemuExitCode::Success)
        } else {
            exit_qemu(QemuExitCode::Failure)
        }
    }

    #[cfg(feature = "fixture-measure")]
    {
        if fixture_measure::run() {
            exit_qemu(QemuExitCode::Success)
        } else {
            exit_qemu(QemuExitCode::Failure)
        }
    }

    #[cfg(not(any(
        feature = "fixture-broken-boot",
        feature = "fixture-context-switch",
        feature = "fixture-idt-apic-timer",
        feature = "fixture-idt-apic-unrouted",
        feature = "fixture-pci-enumeration",
        feature = "fixture-pool-bench",
        feature = "fixture-measure"
    )))]
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
            Ok(topology) if !topology.is_empty() => {
                // SAFETY: called exactly once, here, before anything else
                // in this boot path depends on interrupts being armed —
                // `hal_x86_64::interrupts::init`'s own documented contract.
                unsafe { hal_x86_64::interrupts::init(BOOT_TIMER_INITIAL_COUNT) };
                // PCI bus-0 discovery (`STORY-P0-04-03`) joins the real boot
                // path's success gate exactly as ACPI topology discovery did
                // for `STORY-P0-04-01`: a regression that stops the walk
                // finding any device is a boot-failure exit code, not a
                // silent no-op. Read-only by construction — see
                // `hal_x86_64::pci`'s module doc.
                //
                // SAFETY: single-CPU boot path with no other config-space
                // user, so exclusive use of the 0xCF8/0xCFC register pair —
                // `PortCam::new`'s documented contract — holds trivially.
                let mut cam = unsafe { hal_x86_64::pci::PortCam::new() };
                let mut devices: hal::device::DeviceTable<{ kernel::capacities::MAX_PCI_DEVICES }> =
                    hal::device::DeviceTable::new();
                match hal_x86_64::pci::enumerate_bus_zero(&mut cam, &mut devices) {
                    Ok(()) if !devices.is_empty() => exit_qemu(QemuExitCode::Success),
                    _ => exit_qemu(QemuExitCode::Failure),
                }
            }
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
