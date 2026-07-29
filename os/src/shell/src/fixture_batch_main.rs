//! `TEST-P2-07-01-A`'s Tier 0 QEMU fixture: the parity `.TCB` on the real target.
//!
//! Runs the *same* seeded world and script as the host golden test
//! (`shell::parity`), streaming the transcript over COM1. In-guest assertions —
//! exactly one denial (the withheld verb), no truncation — gate the
//! `isa-debug-exit` code, so the harness gets two independent signals: the
//! byte-compared transcript and the exit verdict (`timing.rs`'s discipline).
//!
//! Boot glue on the `exec-fixture` pattern; the `shell` *library* stays
//! `forbid(unsafe_code)` — this binary is its own compilation unit.

#![no_std]
#![no_main]

#[allow(unused_imports)]
// linked for its `global_asm!` side effect only, per its own doc comment
use hal_x86_64::boot as _;
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use hal_x86_64::serial::SerialPort;
use shell::{batch, parity};

#[no_mangle]
extern "C" fn kernel_main(_start_info_paddr: u64) -> ! {
    // SAFETY-adjacent precondition: COM1 exists under the QEMU q35 fixture
    // machine, the same precondition every serial-using fixture states.
    let mut serial = unsafe { SerialPort::init() };

    let mut world = parity::world();
    let outcome = batch::run(&mut world, parity::SCRIPT, &mut serial);

    let ok = match outcome {
        Ok(stats) => stats.denials == parity::expected_denials() && !stats.truncated,
        Err(_) => false,
    };
    exit_qemu(if ok { QemuExitCode::Success } else { QemuExitCode::Failure })
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    hal_x86_64::qemu_exit::panic_report(info)
}
