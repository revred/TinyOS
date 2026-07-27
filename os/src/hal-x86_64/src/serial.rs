//! Minimal COM1 (16550-compatible UART) serial driver.
//!
//! Exists solely so a Tier 0 QEMU fixture can report *numeric* evidence
//! (cycle counts, percentiles) off the emulated machine — `qemu_exit`'s
//! isa-debug-exit port only carries a single pass/fail bit, which is enough
//! for boot/interrupt fixtures but not for `STORY-P0-03-01`'s
//! `PERF-D07`/`fixture-pool-bench` measurement fixture, which needs real
//! target-CPU `RDTSC` numbers to leave the VM. QEMU's `-serial file:PATH`
//! flag (see `xtask`'s own doc comment on why it can't just always be on)
//! redirects whatever this driver writes to a host-readable file with no
//! guest-side parsing/formatting infrastructure beyond this module.
//!
//! Written as permanent, reviewable driver code (doc-comment-heavy, a
//! `SAFETY` comment on every `unsafe` block, matching this crate's existing
//! `interrupts.rs`/`qemu_exit.rs` style) rather than a throwaway hack, since
//! any future fixture needing numeric (not just pass/fail) evidence off the
//! emulated machine can reuse it as-is.

use core::fmt;

/// COM1's conventional I/O base port on PC-compatible platforms (including
/// QEMU's `q35` machine type, which `xtask qemu-x86_64` always boots).
const COM1_BASE: u16 = 0x3F8;

const DATA: u16 = COM1_BASE;
const INTERRUPT_ENABLE: u16 = COM1_BASE + 1;
const FIFO_CONTROL: u16 = COM1_BASE + 2;
const LINE_CONTROL: u16 = COM1_BASE + 3;
const MODEM_CONTROL: u16 = COM1_BASE + 4;
const LINE_STATUS: u16 = COM1_BASE + 5;

/// `LINE_STATUS`'s "transmitter holding register empty" bit — set when it is
/// safe to write another byte to `DATA` without overrunning the UART's
/// output FIFO.
const LSR_THR_EMPTY: u8 = 0x20;

/// Reads a byte from `port`.
///
/// # Safety
/// `port` must name an I/O port whose read has no side effect the caller
/// hasn't accounted for — true for every port this module reads (COM1's own
/// registers, always present on the `q35` machine type `xtask` boots).
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: `in`/`out`-family port I/O is unconditionally available on
    // every x86_64 CPU; the caller's own contract (above) covers the only
    // way this could go wrong (reading a port with an unaccounted-for side
    // effect).
    unsafe {
        core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// Writes `value` to `port`.
///
/// # Safety
/// `port` must name an I/O port whose write has no side effect the caller
/// hasn't accounted for — true for every port this module writes (COM1's own
/// registers).
unsafe fn outb(port: u16, value: u8) {
    // SAFETY: same rationale as `inb`.
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}

/// A handle to the COM1 UART, initialized for 8-N-1 at 38400 baud — QEMU's
/// software UART model has no real electrical baud-rate constraint, but a
/// concrete divisor is still required to bring the line-control register
/// into a well-defined state before this driver relies on it.
pub struct SerialPort;

impl SerialPort {
    /// Programs COM1 into a known 8-N-1 configuration with its FIFOs
    /// enabled and interrupts masked (this driver is polled, never
    /// interrupt-driven — nothing in this codebase's IDT routes COM1's IRQ).
    ///
    /// # Safety
    /// Must be called at most once before any other `SerialPort` method, and
    /// only when running under QEMU (or real PC-compatible hardware) where
    /// COM1's registers exist at their conventional port-I/O addresses —
    /// exactly the same precondition `xtask qemu-x86_64` fixtures already
    /// rely on for `qemu_exit::exit_qemu`.
    pub unsafe fn init() -> Self {
        // SAFETY: every `outb` here targets one of COM1's own registers,
        // sequenced per the standard 16550 initialization protocol (mask
        // interrupts, set the divisor-latch bit, program the divisor, drop
        // back to normal register access, enable+clear the FIFOs, assert
        // DTR/RTS) — no step has a side effect beyond configuring this UART,
        // which this function's own contract requires to be safe to do.
        unsafe {
            outb(INTERRUPT_ENABLE, 0x00); // mask all UART-generated interrupts
            outb(LINE_CONTROL, 0x80); // DLAB=1: next two writes set the divisor
            outb(DATA, 0x03); // divisor low byte: 38400 baud (115200 / 3)
            outb(INTERRUPT_ENABLE, 0x00); // divisor high byte
            outb(LINE_CONTROL, 0x03); // DLAB=0, 8 data bits, no parity, 1 stop bit
            outb(FIFO_CONTROL, 0xC7); // enable+clear FIFOs, 14-byte trigger level
            outb(MODEM_CONTROL, 0x0B); // assert DTR, RTS, and OUT2
        }
        SerialPort
    }

    /// Writes a single byte, spin-waiting for the transmitter to be ready.
    /// No bound on the wait beyond the caller's own overall fixture
    /// time budget (`xtask`'s external 15-second QEMU boot timeout) — COM1
    /// under QEMU never actually stalls in practice, unlike real hardware
    /// with an unplugged cable.
    pub fn write_byte(&mut self, byte: u8) {
        loop {
            // SAFETY: reading `LINE_STATUS` has no side effect beyond
            // reporting UART state.
            let ready = unsafe { inb(LINE_STATUS) } & LSR_THR_EMPTY != 0;
            if ready {
                break;
            }
        }
        // SAFETY: the wait above establishes the transmitter-holding
        // register is empty, so this write cannot overrun it.
        unsafe { outb(DATA, byte) };
    }

    /// Writes every byte of `bytes` in order.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_byte(b);
        }
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_bytes(s.as_bytes());
        Ok(())
    }
}
