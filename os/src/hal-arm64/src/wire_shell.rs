//! The command runner seam (`STORY-P1-09-18`).
//!
//! [`crate::tos64_cmd`] classifies an admitted frame and reports the line its
//! `SHELL` row needs run. **It does not run it**, and that is the property its
//! whole containment argument stands on: the module forbids `unsafe`, takes no
//! device in any signature, and would stop being able to say either of those
//! things the moment it grew a handler.
//!
//! So the run crosses a crate boundary, and it crosses it the same way every
//! other inverted dependency on this board does — a `#[no_mangle] extern "C"`
//! symbol, exactly as `crate::spoor` reaches `kernel`'s spoor stream. The
//! reason is the same shape and worth writing down rather than inferring:
//!
//! - The runner is `shell` — `TINYCMD`'s verb core, the labelled RAM volume
//!   and the DOS front-end.
//! - `shell` depends on `kernel`, and on AArch64 the dependency runs
//!   `kernel` → `hal-arm64`. A direct call from here would be a cycle, and
//!   Cargo would refuse it.
//! - So the implementor is the **composition root**, `pi5-image`, which is the
//!   one crate that already sees the whole graph. It supplies the grant set,
//!   the seeded volume and the session; this file supplies only the call.
//!
//! # Why the signature carries arrays and not pointers
//!
//! `&[u8; N]` and `&mut [u8; N]` are thin pointers and are FFI-safe, so the
//! seam is expressible without a raw pointer on either side. That buys two
//! things a `*const u8`/`*mut u8` pair would not:
//!
//! 1. **The implementor needs no `unsafe` at all.** `pi5-image` keeps
//!    `#![forbid(unsafe_code)]`, and so does `shell`. The claim "a wire verb
//!    cannot reach a register" therefore stays compiler-enforced across all
//!    three crates on this path rather than becoming a review promise at the
//!    boundary.
//! 2. **The two sides cannot disagree about a width without failing to
//!    compile.** Both name the same constants from [`crate::tos64_cmd`], so a
//!    changed capacity is a type error in `pi5-image` rather than a buffer
//!    overrun discovered on a bench. `LE-122` is the whole argument for
//!    preferring that.
//!
//! The length is passed beside the array rather than encoded in it because the
//! command line is the argument field *trimmed*, and re-deriving the trim on
//! the far side would put the same rule in two crates — the way a rule drifts.

/// Stages `line` into the fixed-width field the seam carries, and returns how
/// many octets of it are meaningful.
///
/// **Arch-neutral and tested on the host, deliberately** (`LE-66`). The three
/// arithmetic decisions on this path — how much of the line is copied, what
/// happens to the rest of the field, and what a length means on the far side —
/// used to live inside the `cfg(target_arch = "aarch64")` block below, where no
/// host test could reach them and the only guard was the comment saying so.
/// Writing about an untestable seam is not the same as removing it, so the
/// arithmetic moved out here and the `cfg` block kept only the `extern` call it
/// cannot be without.
///
/// The tail is zeroed rather than left as the caller found it: the field is a
/// fixed width on the wire and a short line must not be able to show the far
/// side whatever the previous command left in the buffer.
#[must_use]
pub fn stage_line(line: &[u8], field: &mut [u8; crate::tos64_cmd::ARGUMENT_BYTES]) -> usize {
    let len = line.len().min(field.len());
    field[..len].copy_from_slice(&line[..len]);
    field[len..].fill(0);
    len
}

/// Clamps a length the far side of the seam reported to what the buffer can
/// actually hold.
///
/// Separate from [`stage_line`] and named rather than inlined because it is the
/// one place this crate declines to trust the implementor. `pi5-image` clamps
/// too, and that is not redundancy: a seam whose safety depends on the other
/// side keeping a promise is a seam nobody re-reads, and this one is reached
/// through an `extern "C"` ABI where the type system stops helping.
#[must_use]
pub const fn clamp_written(written: usize, capacity: usize) -> usize {
    if written < capacity {
        written
    } else {
        capacity
    }
}

#[cfg(target_arch = "aarch64")]
extern "C" {
    /// `pi5_image::tinyos_wire_shell_run` — runs one command line through the
    /// image's wire shell.
    ///
    /// Returns how many octets of output were produced, capped at
    /// [`crate::tos64_cmd::SHELL_OUTPUT_CAPACITY`]. Zero is a legitimate
    /// answer (the shell printed nothing) and is rendered as `out=none`, not
    /// as a failure — this seam has no error channel because there is no
    /// failure it could report that the shell would not have already printed.
    ///
    /// `line_len` is a length **this crate** computed from a fixed-width
    /// field, not one taken from the wire. The implementor clamps it
    /// regardless, because a seam whose safety depends on its caller is a seam
    /// nobody re-reads.
    fn tinyos_wire_shell_run(
        line: &[u8; crate::tos64_cmd::ARGUMENT_BYTES],
        line_len: usize,
        out: &mut [u8; crate::tos64_cmd::SHELL_OUTPUT_CAPACITY],
    ) -> usize;
}

/// Runs `line` and returns the output length written into `out`.
///
/// Called from exactly one place — the park loop's bounded answer slot — so a
/// command costs the beat it was always going to cost and cannot be made to
/// run more often than the answer rate `SEC-20` already bounds.
#[cfg(target_arch = "aarch64")]
#[must_use]
pub fn run(line: &[u8], out: &mut [u8; crate::tos64_cmd::SHELL_OUTPUT_CAPACITY]) -> usize {
    let mut field = [0u8; crate::tos64_cmd::ARGUMENT_BYTES];
    let len = stage_line(line, &mut field);
    // SAFETY: the symbol is provided by `pi5-image`, which links this crate;
    // both arguments are references to arrays this frame owns, and the callee
    // is a safe Rust function whose only unsafety is the `extern "C"` ABI it
    // is reached through. Single core, called once per park beat from ordinary
    // code — never from an interrupt handler.
    let written = unsafe { tinyos_wire_shell_run(&field, len, out) };
    clamp_written(written, out.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tos64_cmd::{ARGUMENT_BYTES, SHELL_OUTPUT_CAPACITY};

    #[test]
    fn a_line_shorter_than_the_field_is_staged_whole_and_the_tail_is_zeroed() {
        let mut field = [0xAAu8; ARGUMENT_BYTES];
        let len = stage_line(b"DIR", &mut field);
        assert_eq!(len, 3);
        assert_eq!(&field[..3], b"DIR");
        assert!(
            field[3..].iter().all(|octet| *octet == 0),
            "a short line must not show the far side what the last command left behind"
        );
    }

    #[test]
    fn a_line_exactly_the_field_width_is_staged_whole_with_no_tail() {
        let line = [b'X'; ARGUMENT_BYTES];
        let mut field = [0u8; ARGUMENT_BYTES];
        assert_eq!(stage_line(&line, &mut field), ARGUMENT_BYTES);
        assert_eq!(field, line);
    }

    #[test]
    fn a_line_longer_than_the_field_is_truncated_rather_than_panicking() {
        // The clamp that had no test. `copy_from_slice` panics on a length
        // mismatch, so an unclamped copy here is a board that dies on a frame
        // a peer chose the width of.
        let line = [b'Z'; ARGUMENT_BYTES * 3];
        let mut field = [0u8; ARGUMENT_BYTES];
        assert_eq!(stage_line(&line, &mut field), ARGUMENT_BYTES);
        assert!(field.iter().all(|octet| *octet == b'Z'));
    }

    #[test]
    fn an_empty_line_stages_nothing_and_zeroes_everything() {
        let mut field = [0xFFu8; ARGUMENT_BYTES];
        assert_eq!(stage_line(b"", &mut field), 0);
        assert!(field.iter().all(|octet| *octet == 0));
    }

    #[test]
    fn a_written_length_is_clamped_even_when_the_far_side_lies() {
        assert_eq!(clamp_written(0, SHELL_OUTPUT_CAPACITY), 0);
        assert_eq!(clamp_written(7, SHELL_OUTPUT_CAPACITY), 7);
        assert_eq!(
            clamp_written(SHELL_OUTPUT_CAPACITY, SHELL_OUTPUT_CAPACITY),
            SHELL_OUTPUT_CAPACITY
        );
        assert_eq!(
            clamp_written(SHELL_OUTPUT_CAPACITY + 1, SHELL_OUTPUT_CAPACITY),
            SHELL_OUTPUT_CAPACITY
        );
        assert_eq!(
            clamp_written(usize::MAX, SHELL_OUTPUT_CAPACITY),
            SHELL_OUTPUT_CAPACITY,
            "the implementor is not trusted to have kept its own contract"
        );
    }
}
