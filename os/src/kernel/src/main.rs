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
#[cfg(feature = "fixture-double-fault")]
mod fixture_double_fault;
#[cfg(feature = "fixture-fault")]
mod fixture_fault;
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
#[cfg(feature = "fixture-preempt")]
mod fixture_preempt;
#[cfg(feature = "fixture-priority-inversion")]
mod fixture_priority_inversion;
#[cfg(any(
    feature = "fixture-wcet-restart",
    feature = "fixture-wcet-degrade",
    feature = "fixture-wcet-trip"
))]
mod fixture_wcet;

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
        feature = "fixture-measure",
        feature = "fixture-fault",
        feature = "fixture-double-fault",
        feature = "fixture-preempt",
        feature = "fixture-priority-inversion",
        feature = "fixture-wcet-restart",
        feature = "fixture-wcet-degrade",
        feature = "fixture-wcet-trip"
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
    feature = "fixture-measure",
    feature = "fixture-fault",
    feature = "fixture-double-fault",
    feature = "fixture-preempt",
    feature = "fixture-priority-inversion",
    feature = "fixture-wcet-restart",
    feature = "fixture-wcet-degrade",
    feature = "fixture-wcet-trip"
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
            feature = "fixture-measure",
            feature = "fixture-fault",
            feature = "fixture-double-fault",
            feature = "fixture-preempt",
            feature = "fixture-priority-inversion",
            feature = "fixture-wcet-restart",
            feature = "fixture-wcet-degrade",
            feature = "fixture-wcet-trip"
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

    #[cfg(feature = "fixture-fault")]
    {
        if fixture_fault::run() {
            exit_qemu(QemuExitCode::Success)
        } else {
            exit_qemu(QemuExitCode::Failure)
        }
    }

    #[cfg(feature = "fixture-double-fault")]
    {
        // A successful run never reaches this `if` at all — it ends inside the
        // fixture's own `#DF` handler, which exits QEMU directly. Reaching here
        // means `run` returned, i.e. the escalation did not happen.
        if fixture_double_fault::run() {
            exit_qemu(QemuExitCode::Success)
        } else {
            exit_qemu(QemuExitCode::Failure)
        }
    }

    #[cfg(feature = "fixture-preempt")]
    {
        if fixture_preempt::run() {
            exit_qemu(QemuExitCode::Success)
        } else {
            exit_qemu(QemuExitCode::Failure)
        }
    }

    #[cfg(feature = "fixture-priority-inversion")]
    {
        if fixture_priority_inversion::run() {
            exit_qemu(QemuExitCode::Success)
        } else {
            exit_qemu(QemuExitCode::Failure)
        }
    }

    #[cfg(any(
        feature = "fixture-wcet-restart",
        feature = "fixture-wcet-degrade",
        feature = "fixture-wcet-trip"
    ))]
    {
        // The `wcet-trip` build never reaches this `if`: its whole claim is
        // that the system stops, so it exits from inside the tick hook with a
        // failure code, which is that fixture's documented pass condition.
        // Reaching here under that feature means the trip did *not* happen.
        if fixture_wcet::run() {
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
        feature = "fixture-measure",
        feature = "fixture-fault",
        feature = "fixture-double-fault",
        feature = "fixture-preempt",
        feature = "fixture-priority-inversion",
        feature = "fixture-wcet-restart",
        feature = "fixture-wcet-degrade",
        feature = "fixture-wcet-trip"
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
/// The default `#UD`/`#GP`/`#PF` entry point for every build except the fault
/// fixture (`STORY-P1-02-01`) and the measurement fixture, both of which
/// supply their own so a deliberate fault can be survived and re-triggered
/// instead of halting the run (`LE-17`, `fixture_measure::tinyos_fault_entry`).
///
/// The `hal_x86_64::fault` stubs call this symbol the same way `boot.rs` calls
/// `kernel_main`: the HAL declares it, the binary defines it, so the HAL needs
/// no dependency on the kernel's fault policy.
///
/// On this path there is no task to contain a fault to — nothing here has
/// switched into one — so `kernel::fault`'s policy returns `HaltSystem`, and
/// this handler reports the frame over COM1 before exiting fail-closed. That
/// report is the whole point: before this Story, the identical situation was a
/// silent triple fault with no diagnostic at all, which cost
/// `STORY-P1-01-01` two debugging cycles.
///
/// # Safety
/// Called only by the fault stubs, with `frame` pointing at the `FaultFrame`
/// they just built on the faulting stack.
#[cfg(not(any(
    feature = "fixture-fault",
    feature = "fixture-double-fault",
    feature = "fixture-measure"
)))]
#[no_mangle]
extern "C" fn tinyos_fault_entry(frame: *const hal_x86_64::fault::FaultFrame) -> ! {
    use core::fmt::Write;
    use kernel::fault::{Disposition, FaultReport, FaultingContext};

    // SAFETY: the stubs pass a pointer to a fully-initialized `FaultFrame` on
    // the current stack, live for this call.
    let frame = unsafe { *frame };
    // SAFETY: this handler never returns, so re-initializing COM1 here cannot
    // race any other user of it; `init`'s own contract is satisfied by there
    // being no concurrent execution on this single-CPU boot path.
    let mut serial = unsafe { hal_x86_64::serial::SerialPort::init() };

    let mnemonic = match frame.kind() {
        Some(vector) => vector.mnemonic(),
        // The IDT and `hal_x86_64::fault` disagree about which vectors are
        // wired — reported as itself rather than decoded as one of the three.
        None => "unwired-vector",
    };
    let _ = writeln!(
        serial,
        "tinyos fault {mnemonic} vector={} error_code={:#x} rip={:#x} rflags={:#x} rsp={:#x} cr2={:#x}",
        frame.vector,
        frame.error_code,
        frame.rip,
        frame.rflags,
        frame.rsp,
        // `0` rather than a stale `CR2` for anything that is not a `#PF`.
        frame.faulting_address().unwrap_or(0)
    );

    let report = FaultReport { vector: frame.vector, context: FaultingContext::Kernel };
    let disposition = Disposition::of(&report);
    // Spoor's first production call site (`STORY-P1-03-02` acceptance
    // criterion I5): the audit pair is journaled, not discarded — closing
    // the sentence four `FEAT-P0-06` Reports repeated ("no production call
    // site or capacity constant added yet"). The journal is a static in the
    // shipping binary, sized by `kernel::capacities::SPOOR_JOURNAL_CAPACITY`;
    // on this halting path its contents are summarized over COM1, which is
    // all the persistence this kernel has until a storage Story lands.
    static mut FAULT_JOURNAL: kernel::spoor_journal::SpoorJournal<
        { kernel::capacities::SPOOR_JOURNAL_CAPACITY },
    > = kernel::spoor_journal::SpoorJournal::new();
    // SAFETY: single-CPU kernel, and this handler never returns — no
    // concurrent access to the journal static is possible.
    //
    // `&mut *(&raw mut STATIC)` is this workspace's established single-owner
    // pattern for a `static mut` one caller owns for a whole kernel run, and
    // it carries the same narrow allow every other user of it does
    // (`fixture_fault`, `os`'s own `main.rs`, `hal_x86_64::interrupts::init`).
    // Clippy's suggested simplification is `&mut STATIC`, which is exactly the
    // `static_mut_refs` the raw-pointer form exists to avoid — taking the
    // suggestion would make this worse, not better.
    #[allow(clippy::deref_addrof)]
    let journal_len = unsafe {
        let journal = &mut *(&raw mut FAULT_JOURNAL);
        for spoor in kernel::fault::audit(&report, disposition) {
            journal.append(spoor);
        }
        journal.len()
    };
    let _ = writeln!(
        serial,
        "tinyos fault disposition={disposition:?} spoor_journal_len={journal_len} — halting"
    );

    exit_qemu(QemuExitCode::Failure)
}

/// The default `#DF` entry point for every build except the double-fault
/// fixture, which supplies its own (`STORY-P1-02-02`).
///
/// Terminal but reporting, and terminal in a way no other handler in this
/// kernel is: a double fault means the fault path itself failed while the CPU
/// was delivering a fault, so there is no context left worth trusting and
/// nothing to contain the fault *to*. `kernel::fault::Disposition` is
/// deliberately not consulted — see `audit_double_fault` for why a double fault
/// is not a disposition question.
///
/// Runs on the IST stack the TSS names, which is the whole reason this function
/// can exist at all. Before `STORY-P1-02-02` this situation produced a silent
/// QEMU reset with no output whatsoever.
///
/// # Safety
/// Called only by `df_fault_stub`, with `frame` pointing at the `FaultFrame` it
/// just built on the IST stack.
#[cfg(not(feature = "fixture-double-fault"))]
#[no_mangle]
extern "C" fn tinyos_double_fault_entry(frame: *const hal_x86_64::fault::FaultFrame) -> ! {
    use core::fmt::Write;

    // SAFETY: the stub passes a pointer to a fully-initialized `FaultFrame` on
    // the IST stack, live for this call.
    let frame = unsafe { *frame };
    // SAFETY: this handler never returns, so re-initializing COM1 here cannot
    // race any other user of it on this single-CPU path.
    let mut serial = unsafe { hal_x86_64::serial::SerialPort::init() };

    let on_ist_stack = {
        let probe = 0u64;
        hal_x86_64::tss::double_fault_stack_contains(&probe as *const u64 as u64)
    };
    let _ = writeln!(
        serial,
        "tinyos double fault #DF vector={} error_code={:#x} rip={:#x} rflags={:#x} faulting_rsp={:#x} on_ist_stack={on_ist_stack}",
        frame.vector, frame.error_code, frame.rip, frame.rflags, frame.rsp
    );
    // Audited on the same path the fixture's is, so an audit change can never
    // apply to one and not the other. There is no journal here to keep it in.
    let _ = kernel::fault::audit_double_fault(kernel::fault::FaultingContext::Kernel);
    let _ = writeln!(serial, "tinyos double fault: the fault path itself failed — halting");

    exit_qemu(QemuExitCode::Failure)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    exit_qemu(QemuExitCode::Failure)
}
