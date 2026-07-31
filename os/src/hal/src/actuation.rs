//! The arch-neutral output boundary an actuation command crosses
//! (`STORY-P1-06-01`).
//!
//! `G-PA-1`'s path ends at *actuation* — the point where a computed command
//! leaves the OS and reaches something outside it. This trait is that point,
//! and it lives here for exactly the reason [`crate::time::CycleSource`] does:
//! the Raspberry Pi 5 slice tracked as `LE-09` must be able to supply its own
//! backend without the kernel-side path, the fixtures, or the measurement
//! harness changing a line. Nothing in this module names a port, a register or
//! an instruction.
//!
//! **Why a byte and not a word.** A command is one byte because the Tier 0
//! stand-in for an actuator line is a real byte-wide ISA port write (see
//! `hal_x86_64::actuation`), and widening the command would mean touching the
//! adjacent port — which on a PC is a real device register, not spare space. A
//! stand-in that quietly writes hardware nobody chose is not a stand-in. Real
//! actuator interfaces that need more than eight bits will need a wider trait;
//! inventing one now, with no implementor that could honour it, would be
//! speculative (`agent/CODING_STANDARDS.md`, "wire the primitive, don't invent
//! a speculative consumer").

/// A bounded, single-command output boundary — under Tier 0 a measurable
/// I/O-port write standing in for a real actuator line.
///
/// # Contract every implementor must honour
///
/// - **Bounded and unconditional.** The write takes a bounded number of
///   instructions, never allocates, never takes a lock, never blocks and never
///   retries. It is called from an RT path with interrupts masked, so anything
///   that can wait is forbidden outright — `agent/CODING_STANDARDS.md`'s
///   real-time discipline, and `README.md` Non-Negotiable #5's fail-safe rule:
///   a stalled actuator write must not become an unbounded retry against a
///   deadline.
/// - **No decision of its own.** An implementor never inspects, filters,
///   rate-limits or refuses a command. *Whether* a command may be emitted is
///   decided upstream by `kernel::actuation::ActuationPort`, which owns the
///   authority and deadline checks; a line that could also refuse would put the
///   same decision in two places and let them disagree.
/// - **Observable exactly once per call.** One call is one actuation. An
///   implementor that coalesced, buffered or replayed writes would break the
///   only property the whole path is measured against.
pub trait OutputLine {
    /// Which line this is, as it appears in evidence — the same role
    /// `hal::time::CycleSource`'s backend name plays in a `TOS64-MEAS/2`
    /// envelope. An unnamed output boundary produces measurements nobody can
    /// attribute to a device.
    const NAME: &'static str;

    /// Writes one command to the line.
    fn write_command(&mut self, command: u8);
}
