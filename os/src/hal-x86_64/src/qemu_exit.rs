//! QEMU's `isa-debug-exit` device: a single I/O port that shuts QEMU down
//! and reports an exit code, so `xtask`'s success/failure verification works
//! from the QEMU process exit code alone — no serial parsing, no
//! "unexpected output" on the console.
//!
//! Lives in `hal-x86_64` (moved from `kernel` by `STORY-P0-05-02`) so every
//! `no_std`/`no_main` binary that boots under `xtask qemu-x86_64` — not just
//! `kernel`'s own — can report its result the same way; see `boot.rs`'s doc
//! comment for the full rationale.

const ISA_DEBUG_EXIT_PORT: u16 = 0xf4;

/// Exit codes written to the `isa-debug-exit` port.
///
/// QEMU's own exit code is `(value << 1) | 1`, so `Success` (0x10) surfaces
/// as process exit code 33 and `Failure` (0x11) as 35 — both distinguishable
/// from QEMU's own crash/signal exit codes (0/1/2), which `xtask` treats as
/// "test harness broke", not "kernel boot failed".
#[repr(u32)]
pub enum QemuExitCode {
    /// The booted binary reached its intended success path.
    Success = 0x10,
    /// The booted binary reached a failure/fault path.
    Failure = 0x11,
}

/// Writes `code` to the isa-debug-exit port, halting QEMU.
///
/// # Safety
/// Valid only when running under QEMU with `-device isa-debug-exit,iobase=0xf4,iosize=0x04`,
/// which `xtask`'s `qemu-x86_64` command always passes — this function must
/// never be reachable on real hardware, where port 0xf4 is unspecified.
pub fn exit_qemu(code: QemuExitCode) -> ! {
    // SAFETY: 0xf4 is the isa-debug-exit port `xtask` configures QEMU with;
    // writing to it is defined by QEMU's device model to terminate the VM
    // and is the documented way to report a pass/fail exit code from guest
    // code with no host-side serial parsing required.
    unsafe {
        core::arch::asm!(
            "out dx, eax",
            in("dx") ISA_DEBUG_EXIT_PORT,
            in("eax") code as u32,
            options(nomem, nostack, preserves_flags)
        );
    }
    // QEMU has already exited by this point; this is an unreachable fallback
    // for the (never expected) case where the exit device is absent.
    loop {
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)) };
    }
}
