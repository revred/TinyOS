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

/// The sentinel a panicking TinyOS binary emits on COM1 before it stops.
///
/// Versioned like every other machine-read line this system produces
/// (`TOS64-MEAS/2`, `TOS64-RESULT/1`), because a host-side assertion that
/// greps for it is a consumer with a contract, not a debugging convenience.
pub const PANIC_SENTINEL: &str = "TOS64-PANIC/1";

/// Reports a panic on COM1, then stops the machine fail-closed
/// (`TEST-P0-01-04-A` clause 1).
///
/// **Why this exists at all.** Every `#[panic_handler]` in this workspace was
/// a bare `exit_qemu(QemuExitCode::Failure)`, so a TinyOS binary that panicked
/// died in complete silence — and `fixture-broken-boot`, whose entire purpose
/// is to panic, produced an *empty* serial capture. Its CI step could
/// therefore only assert "exit code 1", which every other failure also
/// produces. The gap in the test harness was hiding a gap in the system: a
/// kernel that cannot say why it stopped is not diagnosable on a board either,
/// where there is no exit code at all (`LE-09`).
///
/// **Fail-closed ordering is load-bearing.** The sentinel is written *before*
/// the exit port is touched, because the exit port stops the machine and
/// anything after it is not evidence. Every write is `let _ =` — a UART that
/// will not accept bytes must not be able to prevent termination, which would
/// turn a panic into a hang.
///
/// Best-effort by construction: this runs from a context that may hold
/// arbitrary broken state, so it takes no lock, allocates nothing, and calls
/// back into no subsystem.
pub fn panic_report(info: &core::panic::PanicInfo) -> ! {
    use core::fmt::Write;
    // SAFETY: single-CPU fail-closed path with nothing else able to run.
    // Re-initializing the UART is idempotent (it reprograms the divisor and
    // FIFO), which is what makes this safe to call from a panic that may have
    // interrupted another `SerialPort` user mid-write.
    let mut serial = unsafe { crate::serial::SerialPort::init() };
    // The message first, because it is what an assertion should key on: a
    // location is a line number, and a CI step that greps for one breaks the
    // next time anything above it moves.
    let _ = write!(serial, "{PANIC_SENTINEL} message={} ", info.message());
    match info.location() {
        Some(location) => {
            let _ = write!(serial, "file={} line={}", location.file(), location.line());
        }
        // `PanicInfo::location` is `None` only for panics raised outside any
        // tracked source position; the sentinel still has to be emitted, or
        // the assertion that greps for it fails for the wrong reason.
        None => {
            let _ = write!(serial, "location=unknown");
        }
    }
    let _ = writeln!(serial);
    exit_qemu(QemuExitCode::Failure)
}
