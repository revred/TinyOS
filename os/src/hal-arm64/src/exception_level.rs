//! `CurrentEL` decoding — pure, and deliberately separate from the `mrs` that
//! reads the register (`TEST-P1-07-01-A` clause 3).
//!
//! The register read lives in [`crate::boot`], which is the one place in this
//! crate allowed to be architecture-specific about the boot path. Everything
//! that *interprets* the value is here, so that the decision the plan calls its
//! second-highest risk — "entered at `EL2`, code assumes `EL1`" — is decided by
//! a function with tests rather than by a constant somebody chose.

/// An AArch64 exception level, as decoded from `CurrentEL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionLevel {
    /// Unprivileged. Not a level firmware can hand over at.
    El0,
    /// Kernel. Where this slice intends to run.
    El1,
    /// Hypervisor. Where the Raspberry Pi firmware is expected to hand over.
    El2,
    /// Secure monitor.
    El3,
}

impl ExceptionLevel {
    /// Decodes `CurrentEL`.
    ///
    /// The level is bits `[3:2]`; every other bit of the register is `RES0`.
    /// This takes the whole register rather than pre-shifted bits precisely so
    /// that the masking is tested here once instead of at each call site — a
    /// caller that shifted wrongly would produce a confident, wrong answer with
    /// no symptom until an `eret` lands somewhere unexpected.
    ///
    /// Returns `Option` because the same decoder is used on values that did not
    /// come from the register (a reported level being checked), not because any
    /// register contents are undecodable — two bits enumerate four levels
    /// exhaustively.
    pub const fn decode(current_el: u64) -> Option<ExceptionLevel> {
        match (current_el >> 2) & 0b11 {
            0 => Some(ExceptionLevel::El0),
            1 => Some(ExceptionLevel::El1),
            2 => Some(ExceptionLevel::El2),
            3 => Some(ExceptionLevel::El3),
            // Unreachable for a two-bit field; stated rather than `unreachable!`
            // because a panic on a board with no fault handler is a silent hang.
            _ => None,
        }
    }

    /// Whether the stub should perform the `EL2 → EL1` drop.
    ///
    /// True for `EL2` and nothing else. `EL3` is deliberately excluded: the
    /// Raspberry Pi firmware does not hand over there, and a Story with no
    /// exception vectors must not attempt a transition whose failure it cannot
    /// observe. Reporting the level and stopping is the honest response to a
    /// level this slice does not handle.
    pub const fn needs_drop_to_el1(self) -> bool {
        matches!(self, ExceptionLevel::El2)
    }

    /// Whether firmware could plausibly have entered at this level.
    ///
    /// `EL0` cannot: reaching it requires an `eret` from above, and nothing at
    /// `EL0` can configure a UART. A decode of `EL0` therefore means the read
    /// is wrong, not that the board is unusual — a distinction worth having
    /// before two hours are spent on the board.
    pub const fn is_plausible_firmware_entry(self) -> bool {
        !matches!(self, ExceptionLevel::El0)
    }

    /// The level's name, as it appears on the wire.
    ///
    /// A fixed string rather than a `Display` impl: `core::fmt` is more
    /// machinery than a board without a fault handler should carry on the path
    /// that exists to report where it woke up.
    pub const fn as_str(self) -> &'static str {
        match self {
            ExceptionLevel::El0 => "EL0",
            ExceptionLevel::El1 => "EL1",
            ExceptionLevel::El2 => "EL2",
            ExceptionLevel::El3 => "EL3",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Clause 3: the register is decoded, not assumed. `CurrentEL` carries the
    // level in bits [3:2]; every other bit is RES0 and must not participate.
    #[test]
    fn the_four_architectural_exception_levels_decode_from_bits_3_2() {
        assert_eq!(ExceptionLevel::decode(0b0000), Some(ExceptionLevel::El0));
        assert_eq!(ExceptionLevel::decode(0b0100), Some(ExceptionLevel::El1));
        assert_eq!(ExceptionLevel::decode(0b1000), Some(ExceptionLevel::El2));
        assert_eq!(ExceptionLevel::decode(0b1100), Some(ExceptionLevel::El3));
    }

    #[test]
    fn the_res0_bits_are_ignored_rather_than_decoded() {
        // A firmware that leaves dirt in the RES0 bits must not change which
        // level we believe we are at: the answer is bits [3:2] and nothing
        // else. This is the whole reason `decode` takes the raw register.
        // Every bit outside [3:2] set, and [3:2] itself set: EL3.
        assert_eq!(ExceptionLevel::decode(0xFFFF_FFFF_FFFF_FFFF), Some(ExceptionLevel::El3));
        assert_eq!(ExceptionLevel::decode(0xFFFF_FFFF_FFFF_FFFC), Some(ExceptionLevel::El3));
        // Every bit outside [3:2] set, and [3:2] clear: EL0 — the dirt above
        // and below must not promote the answer.
        assert_eq!(ExceptionLevel::decode(0xFFFF_FFFF_FFFF_FFF3), Some(ExceptionLevel::El0));
        assert_eq!(ExceptionLevel::decode(0b0110), Some(ExceptionLevel::El1));
        assert_eq!(ExceptionLevel::decode(0b1011), Some(ExceptionLevel::El2));
    }

    #[test]
    fn decode_is_total_over_every_representable_register_value() {
        // There is no undecodable `CurrentEL`: two bits, four levels. `decode`
        // returning `Option` would be dishonest if some input had no answer, so
        // this test exists to pin that it never returns `None` for a value the
        // register can hold. It returns `Option` only so a *reported* value
        // parsed from elsewhere can be rejected; see the next test.
        for raw in 0u64..256 {
            assert!(ExceptionLevel::decode(raw).is_some(), "no decode for {raw:#x}");
        }
    }

    // Clause 3: the drop is conditional on what was read, never assumed.
    #[test]
    fn only_el2_asks_for_a_drop_to_el1() {
        assert!(!ExceptionLevel::El0.needs_drop_to_el1());
        assert!(!ExceptionLevel::El1.needs_drop_to_el1());
        assert!(ExceptionLevel::El2.needs_drop_to_el1());
        // EL3 is not dropped from here. The Raspberry Pi firmware does not hand
        // over at EL3, and a Story with no fault reporting must not attempt a
        // two-level transition it cannot observe failing. It is reported and
        // left alone — the honest answer for a level this Story does not
        // handle.
        assert!(!ExceptionLevel::El3.needs_drop_to_el1());
    }

    #[test]
    fn el0_is_reported_as_the_impossible_entry_it_is() {
        // Firmware cannot hand over at EL0 — there is no way to get there
        // without an `eret` from above, and nothing below EL1 can configure a
        // UART. If the decode says EL0, the read is wrong, not the board.
        assert!(!ExceptionLevel::El0.is_plausible_firmware_entry());
        assert!(ExceptionLevel::El1.is_plausible_firmware_entry());
        assert!(ExceptionLevel::El2.is_plausible_firmware_entry());
        assert!(ExceptionLevel::El3.is_plausible_firmware_entry());
    }

    // Clause 3: "prints it before anything else" — so it has to render, and
    // rendering must not need `core::fmt` machinery on a board with no
    // allocator and no fault handler.
    #[test]
    fn each_level_renders_as_a_fixed_string() {
        assert_eq!(ExceptionLevel::El0.as_str(), "EL0");
        assert_eq!(ExceptionLevel::El1.as_str(), "EL1");
        assert_eq!(ExceptionLevel::El2.as_str(), "EL2");
        assert_eq!(ExceptionLevel::El3.as_str(), "EL3");
    }
}
