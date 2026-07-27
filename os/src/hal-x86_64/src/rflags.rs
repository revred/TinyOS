//! `RFLAGS` bit arithmetic (`STORY-P1-04-01`).
//!
//! The pure half of the interrupt-free critical section
//! [`crate::interrupts::without_interrupts`] implements, split out here for
//! the same reason [`crate::idt`] is split out of [`crate::interrupts`]:
//! deciding *whether to re-enable interrupts on the way out* is ordinary
//! arithmetic over a `u64` and is provably correct on any toolchain, while
//! `cli`/`pushfq` only mean anything on a real x86_64 CPU. This module is
//! therefore **not** gated to `not(target_os = "windows")` — its tests must
//! run on a Windows development machine, not only on the Linux CI runner,
//! exactly as [`crate::tsc`]'s PIT arithmetic already is.

/// Bit 9 of `RFLAGS`: the interrupt-enable flag (Intel SDM Vol 1 §3.4.3.3).
pub const INTERRUPT_FLAG: u64 = 1 << 9;

/// Whether `rflags` has `IF` set, i.e. whether maskable interrupts were
/// enabled at the moment that value was captured.
pub const fn interrupts_enabled(rflags: u64) -> bool {
    rflags & INTERRUPT_FLAG != 0
}

/// Whether a critical section entered with `saved_rflags` must re-enable
/// interrupts as it exits.
///
/// The whole rule is "restore what was there, never assume it was on". A
/// nested section — one entered while an outer section already had `IF`
/// clear — must **not** `sti` on the way out: doing so would re-enable
/// interrupts in the middle of the outer section, which is precisely the
/// window that section exists to close, and it would do it silently. That
/// is the failure this function exists to make impossible, and why it is a
/// named function with its own test rather than an inline `& (1 << 9)` at
/// the one call site.
pub const fn should_reenable(saved_rflags: u64) -> bool {
    interrupts_enabled(saved_rflags)
}

#[cfg(test)]
mod tests {
    use super::*;

    // `TEST-P1-04-01-A` clause 3: a section entered with interrupts enabled
    // re-enables on exit; one entered with them already disabled does not.
    #[test]
    fn a_section_entered_with_interrupts_enabled_reenables_on_exit() {
        // 0x202: reserved bit 1 + IF — the value `kernel::context::Context::new`
        // itself seeds a fresh task's frame with.
        assert!(should_reenable(0x202));
        assert!(interrupts_enabled(0x202));
    }

    #[test]
    fn a_nested_section_does_not_reenable_interrupts_its_caller_disabled() {
        // 0x002: the same flags with IF cleared, i.e. what `pushfq` reads
        // inside an outer critical section.
        assert!(!should_reenable(0x002));
        assert!(!interrupts_enabled(0x002));
    }

    // Only bit 9 decides this. A value with every *other* flag set must
    // still read as "interrupts were off" — an implementation that tested a
    // neighbouring bit (TF at 8, DF at 10 are the easy slips) would pass a
    // test using realistic flag values and fail here.
    #[test]
    fn no_flag_other_than_if_can_be_mistaken_for_it() {
        assert!(!interrupts_enabled(!INTERRUPT_FLAG));
        assert!(interrupts_enabled(u64::MAX));
        assert!(!interrupts_enabled(0));
        for bit in 0..64u32 {
            let value = 1u64 << bit;
            assert_eq!(
                interrupts_enabled(value),
                bit == 9,
                "bit {bit} must not be read as the interrupt flag"
            );
        }
    }
}
