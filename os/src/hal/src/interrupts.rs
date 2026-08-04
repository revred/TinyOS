//! Interrupt masking as a **scope**, not a one-way switch
//! (`STORY-P1-07-10`, `LE-71`).
//!
//! A measurement region must run interrupt-free or its samples silently
//! include interrupt handlers. The obvious way to get that — mask on the way
//! in and let the caller worry about the rest — is what `LE-71` records:
//! `fixture_measure` masked IRQs and nothing ever unmasked them, so the tick
//! could only fire in the narrow window before the fixture started. On the
//! Pi 5 that window admitted exactly one tick, one tick is one timestamp,
//! one timestamp is zero intervals, and the ratio check that
//! `STORY-P1-07-04` criterion 1 depends on can never form from zero
//! intervals. The masking was correct; its *scope* was unbounded.
//!
//! So the region is expressed as a scope here, in arch-neutral code, and the
//! architecture supplies only the two register pokes. That split is
//! deliberate: the ordering rule (mask, run, restore-what-was-there) is the
//! part that was wrong and the part a host test can hold, while the `MSR`
//! instructions are the part no host can execute. `LE-66`'s finding applies —
//! a seam with no tests is not thin, it is untested — so the testable half is
//! made as large as it honestly can be.

/// The `DAIF` `I` bit: set means IRQs are **masked** at `PSTATE`.
///
/// Arch-neutral by placement but AArch64 by definition; it lives here so the
/// decode that turns a register read into a decision is host-testable, which
/// the `MSR` that produced the read never can be.
pub const DAIF_I: u64 = 1 << 7;

/// Whether IRQs were enabled at the moment a masked region was entered.
///
/// Carried out of [`InterruptGate::mask`] and back into
/// [`InterruptGate::restore`] so a region entered with interrupts already
/// masked is left masked. Restoring by unconditional unmask would enable
/// interrupts a caller never had — on a boot whose tick was *refused*, that
/// would open the door with no timer behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptState {
    enabled_on_entry: bool,
}

impl InterruptState {
    /// The state meaning "IRQs were enabled when the region was entered".
    #[must_use]
    pub const fn enabled() -> Self {
        InterruptState { enabled_on_entry: true }
    }

    /// The state meaning "IRQs were already masked when the region was
    /// entered" — restoring it must **not** unmask.
    #[must_use]
    pub const fn masked() -> Self {
        InterruptState { enabled_on_entry: false }
    }

    /// Decodes a raw `DAIF` read. Only the `I` bit decides; `D`, `A` and `F`
    /// are other exception classes and say nothing about IRQ acceptance.
    #[must_use]
    pub const fn from_daif(daif: u64) -> Self {
        InterruptState { enabled_on_entry: daif & DAIF_I == 0 }
    }

    /// Whether the region must unmask on the way out.
    #[must_use]
    pub const fn was_enabled(self) -> bool {
        self.enabled_on_entry
    }
}

/// The two register pokes an architecture must supply for [`with_interrupts_masked`].
///
/// Implementations are expected to be thin to the point of triviality — one
/// `MRS`/`MSR` pair each. Every rule about *when* they are called lives in
/// [`with_interrupts_masked`], where it can be tested.
pub trait InterruptGate {
    /// Masks IRQs and reports whether they were enabled beforehand.
    fn mask(&self) -> InterruptState;

    /// Restores the state [`InterruptGate::mask`] reported.
    fn restore(&self, state: InterruptState);
}

/// Runs `body` with IRQs masked, then restores whatever was there before.
///
/// The restore is unconditional with respect to `body`'s control flow: an
/// early `return` inside the closure returns from the closure, not from this
/// function, so the region still closes. That is the specific shape `LE-71`
/// needed — `fixture_measure` has a `return false` partway through its emit
/// path, and a restore written as a trailing statement would have been
/// skipped exactly when a run failed.
///
/// This crate is built without unwinding (`panic = "abort"`), so a panicking
/// `body` aborts rather than escaping past the restore; no drop guard is
/// required and none is claimed.
pub fn with_interrupts_masked<G, F, T>(gate: &G, body: F) -> T
where
    G: InterruptGate,
    F: FnOnce() -> T,
{
    let entry_state = gate.mask();
    let result = body();
    gate.restore(entry_state);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;

    /// What a gate was asked to do, in order — the ordering is the whole
    /// claim, so it is recorded rather than counted.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Event {
        Masked,
        BodyRan,
        Restored(InterruptState),
    }

    /// A recording gate that reports a chosen entry state.
    struct RecordingGate {
        entry: InterruptState,
        log: RefCell<Vec<Event>>,
    }

    impl RecordingGate {
        fn reporting(entry: InterruptState) -> Self {
            RecordingGate { entry, log: RefCell::new(Vec::new()) }
        }

        fn log(&self) -> Vec<Event> {
            self.log.borrow().clone()
        }
    }

    impl InterruptGate for RecordingGate {
        fn mask(&self) -> InterruptState {
            self.log.borrow_mut().push(Event::Masked);
            self.entry
        }

        fn restore(&self, state: InterruptState) {
            self.log.borrow_mut().push(Event::Restored(state));
        }
    }

    // ---- the DAIF decode: platform semantics, per LE-66 -------------------

    #[test]
    fn a_clear_i_bit_means_interrupts_were_enabled() {
        assert!(InterruptState::from_daif(0).was_enabled());
    }

    #[test]
    fn a_set_i_bit_means_interrupts_were_already_masked() {
        assert!(!InterruptState::from_daif(DAIF_I).was_enabled());
    }

    /// `D`, `A` and `F` are debug, SError and FIQ. A region that masked one
    /// of those says nothing about IRQ acceptance, and reading them as `I`
    /// would restore the wrong thing.
    #[test]
    fn the_other_daif_bits_do_not_decide_irq_state() {
        let d_a_f_only = (1 << 9) | (1 << 8) | (1 << 6);
        assert!(InterruptState::from_daif(d_a_f_only).was_enabled(), "only the I bit may decide");
        assert!(!InterruptState::from_daif(d_a_f_only | DAIF_I).was_enabled());
    }

    // ---- the scope: the LE-71 regression ---------------------------------

    /// The defect in one test: masking must not be one-way. Before this
    /// Story `fixture_measure` masked and nothing restored, so the tick died
    /// at the fixture and `STORY-P1-07-04` criterion 1 was unreachable.
    #[test]
    fn masking_is_a_scope_and_the_region_is_always_closed() {
        let gate = RecordingGate::reporting(InterruptState::enabled());
        with_interrupts_masked(&gate, || {
            gate.log.borrow_mut().push(Event::BodyRan);
        });
        assert_eq!(
            gate.log(),
            vec![Event::Masked, Event::BodyRan, Event::Restored(InterruptState::enabled())]
        );
    }

    /// Restore is handed exactly what mask reported — not a guess, and not an
    /// unconditional unmask. A boot whose tick was refused enters with IRQs
    /// already masked and must leave that way.
    #[test]
    fn a_region_entered_already_masked_is_left_masked() {
        let gate = RecordingGate::reporting(InterruptState::masked());
        with_interrupts_masked(&gate, || {});
        assert_eq!(
            gate.log(),
            vec![Event::Masked, Event::Restored(InterruptState::masked())],
            "restoring an already-masked region must not unmask it"
        );
    }

    /// `fixture_measure`'s emit path returns early on failure. A restore
    /// written as a trailing statement would be skipped precisely when a run
    /// failed — the worst time to leave interrupts off.
    #[test]
    fn an_early_return_inside_the_body_still_closes_the_region() {
        let gate = RecordingGate::reporting(InterruptState::enabled());
        let verdict = with_interrupts_masked(&gate, || {
            if gate.entry.was_enabled() {
                return false;
            }
            true
        });
        assert!(!verdict, "the body's value is returned unchanged");
        assert_eq!(gate.log(), vec![Event::Masked, Event::Restored(InterruptState::enabled())]);
    }

    #[test]
    fn the_body_s_value_is_returned_to_the_caller() {
        let gate = RecordingGate::reporting(InterruptState::enabled());
        assert_eq!(with_interrupts_masked(&gate, || 0x5EEDu32), 0x5EED);
    }
}
