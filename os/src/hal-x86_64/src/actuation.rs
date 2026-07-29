//! The x86_64 [`OutputLine`] backend: a real ISA port write standing in for an
//! actuator line (`STORY-P1-06-01`).
//!
//! **What this is, stated before any number derived from it is quoted.** It is
//! a genuine `out` instruction to a real I/O port — the same class of bus
//! access a memory-mapped GPIO or a PWM register write would be, retired by the
//! CPU rather than optimised away, and therefore a real cost to measure. It is
//! **not** an actuator: nothing moves. `FEAT-P1-06` states the distinction as
//! its own exit criterion — *a QEMU-measured bound is the mechanism's proof,
//! the boards' numbers are the product's* — and this module is the mechanism
//! half of it.
//!
//! **Why port `0x80`.** It is the PC's POST/diagnostic port: byte-wide,
//! write-only in practice, claimed by no device in QEMU's `q35` model, and
//! already the canonical target for an I/O delay on this architecture — which
//! is precisely why writing to it is known-harmless on real hardware too. The
//! alternatives were each worse in a specific way: an unassigned high port is
//! unassigned *today*, `0x81` upward is the DMA page-register file, and a
//! memory write to a static would not retire a bus access at all, which would
//! make the measured path cheaper than the one it stands in for.
//!
//! Deliberately **not** `#[cfg(not(target_os = "windows"))]`-gated, for the same
//! reason `tsc` is not: the `asm!` here is plain port I/O with no ELF-specific
//! content, so it assembles under a COFF host assembler and the kernel-side
//! code that depends on this type stays host-testable on a Windows dev machine.
//! The instruction itself is privileged and is never *executed* off the target.

use hal::actuation::OutputLine;

/// The POST/diagnostic port this backend writes. See the module doc for why
/// this port and not another.
pub const ACTUATION_PORT: u16 = 0x80;

/// The Tier 0 actuator stand-in: one `out` to [`ACTUATION_PORT`].
///
/// A unit struct rather than a handle, because the port is a fixed
/// architectural address with no state to own. Exclusive use is not enforced
/// here and does not need to be: authority over *who may actuate* belongs to
/// `kernel::actuation::ActuationPort`, which is the single owner of this type
/// in any image that has one — putting a second gate here would let the two
/// disagree about the same decision.
#[derive(Debug, Default, Clone, Copy)]
pub struct PortLine;

impl OutputLine for PortLine {
    const NAME: &'static str = "isa-port-0x80";

    fn write_command(&mut self, command: u8) {
        // SAFETY: `0x80` is the PC POST/diagnostic port — byte-wide, claimed by
        // no device in QEMU's `q35` model and architecturally harmless on real
        // hardware, where it is the standard I/O-delay target. The write has no
        // effect on any memory this program owns (`nomem`, `nostack`) and
        // cannot fault at CPL0, which is the only ring this kernel runs in.
        unsafe {
            core::arch::asm!(
                "out dx, al",
                in("dx") ACTUATION_PORT,
                in("al") command,
                options(nomem, nostack, preserves_flags)
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The port is a fixed architectural address, and it is the one the module
    // doc argues for. Pinned as a test rather than left as a comment, because
    // "which port" is a hardware-safety decision and a silent edit to it is a
    // write to whatever device happens to live at the new address.
    #[test]
    fn the_actuation_port_is_the_post_diagnostic_port() {
        assert_eq!(ACTUATION_PORT, 0x80);
        assert_eq!(PortLine::NAME, "isa-port-0x80");
    }
}
