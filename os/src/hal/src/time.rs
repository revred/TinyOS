//! Arch-neutral time sources for measurement (`STORY-P1-01-01`).
//!
//! `EPIC-P1`'s whole point is *measured* determinism, and Handover 37's
//! hardware directive makes the Raspberry Pi 5 (ARM64) the first physical
//! timing target — so the one thing the measurement harness must not do is
//! bake `RDTSC` into its interface the way `STORY-P0-03-01`'s one-off
//! `fixture_pool_bench` did. These two traits are that seam: the kernel-side
//! harness (`kernel::measure`) is generic over [`CycleSource`], the x86_64
//! backend supplies `hal_x86_64::tsc::Tsc`, and the ARM64 slice tracked as
//! loose end `LE-09` supplies a `CNTVCT_EL0`-backed implementor later
//! without the harness or any fixture changing.
//!
//! Both traits are deliberately single-method (Interface Segregation, per
//! `agent/CODING_STANDARDS.md#i--interface-segregation`): timing code that
//! only needs to count cycles must not be coupled to a timebase it never
//! reads, because a cycle source with no known frequency is a real and
//! expected case — QEMU/TCG's emulated TSC being exactly that case.
//!
//! [`conformance`] is the shared conformance suite the Liskov rule requires
//! of any trait with two or more implementors: it runs identically against
//! every [`CycleSource`], on the host against a test double and on the metal
//! from inside a Tier 0 fixture, so a backend that violates the trait's
//! documented contract fails a test the others pass.

/// A monotonically non-decreasing free-running counter of CPU cycles (or of
/// whatever the architecture's cheapest always-available cycle-granular
/// counter is: `RDTSC` on x86_64, `CNTVCT_EL0` on AArch64).
///
/// # Contract every implementor must honor
///
/// - **Non-decreasing.** Two reads ordered in program order never observe a
///   decrease. Wrapping is not a permitted excuse: every architecture this
///   project targets provides at least a 56-bit counter, which at any
///   plausible clock does not wrap inside a measurement run.
/// - **Forward progress.** The counter advances on its own; it is never
///   constant across a bounded run of reads.
/// - **No side effects.** Reading is free of observable effect on any state
///   the caller owns — in particular it never allocates, never blocks, and
///   never takes a lock, so it is legal on an RT path.
/// - **Cheap and calibratable.** The read's own cost is small and stable
///   enough that a minimum-of-N calibration (`kernel::measure::Calibration`)
///   is meaningful.
///
/// A read says nothing about elapsed *time*; converting cycles to
/// microseconds requires a separate, explicitly-calibrated [`Timebase`].
pub trait CycleSource {
    /// Reads the counter.
    fn read_cycles(&self) -> u64;
}

/// A known conversion factor from a [`CycleSource`]'s cycles to wall-clock
/// microseconds.
///
/// Separate from [`CycleSource`] precisely because it is frequently
/// *unavailable*: under QEMU/TCG the emulated TSC has no documented,
/// stable frequency relationship to host wall-clock time, and
/// `REPORT-2026-07-27-01` recorded the resulting gap as a real blocker —
/// microsecond-denominated performance guardrails could not be scored at all
/// against cycle-denominated evidence. An implementor that cannot honestly
/// establish a factor returns `None` rather than guessing, and every artifact
/// downstream then reports cycles only and says so.
pub trait Timebase {
    /// Cycles per microsecond, or `None` when no trustworthy factor could be
    /// established.
    ///
    /// Implementors must return `None` rather than a plausible-looking
    /// default: a fabricated timebase silently converts every derived
    /// microsecond figure into an unfalsifiable claim.
    fn cycles_per_us(&self) -> Option<u32>;
}

/// The shared conformance suite for [`CycleSource`], required by
/// `agent/CODING_STANDARDS.md#l--liskov-substitution` for any trait with two
/// or more implementors.
///
/// Written as ordinary `no_std` runtime checks rather than `#[cfg(test)]`
/// helpers so the *same* suite runs in three places: host unit tests against
/// a test double, the Tier 0 QEMU fixture against `hal_x86_64::tsc::Tsc` on
/// real target-compiled code, and (when `LE-09` lands) an ARM64 fixture
/// against that backend, on hardware where no host test harness exists.
pub mod conformance {
    use super::CycleSource;

    /// Why a [`CycleSource`] failed [`check`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ConformanceFailure {
        /// A read observed a smaller value than an earlier read — a direct
        /// violation of the non-decreasing contract.
        WentBackwards {
            /// The earlier, larger reading.
            previous: u64,
            /// The later, smaller reading.
            observed: u64,
        },
        /// Every read in the run returned the same value: the source is
        /// stuck, not merely fast.
        NoForwardProgress {
            /// How many reads were taken.
            samples: usize,
        },
        /// [`check`] was asked for fewer than two samples, which cannot
        /// establish either property. A caller error, reported rather than
        /// silently passing.
        TooFewSamples {
            /// The requested sample count.
            samples: usize,
        },
    }

    /// Runs the suite over `source`, taking `samples` bounded consecutive
    /// reads (bounded because this runs on RT-discipline code paths: no
    /// unbounded loop, no allocation, no blocking).
    ///
    /// Returns `Ok(observed_span)` — the total advance across the run — so a
    /// caller can report the source's granularity alongside the pass.
    pub fn check<S: CycleSource>(source: &S, samples: usize) -> Result<u64, ConformanceFailure> {
        if samples < 2 {
            return Err(ConformanceFailure::TooFewSamples { samples });
        }
        let first = source.read_cycles();
        let mut previous = first;
        let mut advanced = false;
        for _ in 1..samples {
            let observed = source.read_cycles();
            if observed < previous {
                return Err(ConformanceFailure::WentBackwards { previous, observed });
            }
            if observed > previous {
                advanced = true;
            }
            previous = observed;
        }
        if !advanced {
            return Err(ConformanceFailure::NoForwardProgress { samples });
        }
        Ok(previous - first)
    }
}

#[cfg(test)]
mod tests {
    use super::conformance::{check, ConformanceFailure};
    use super::{CycleSource, Timebase};
    use core::cell::Cell;

    /// A scripted host test double: each read advances by `step`, so a
    /// `step` of 0 models a stuck counter and a negative-going source is
    /// modelled by [`Backwards`] below.
    struct StepSource {
        now: Cell<u64>,
        step: u64,
    }

    impl StepSource {
        fn new(start: u64, step: u64) -> Self {
            StepSource { now: Cell::new(start), step }
        }
    }

    impl CycleSource for StepSource {
        fn read_cycles(&self) -> u64 {
            let value = self.now.get();
            self.now.set(value + self.step);
            value
        }
    }

    /// A deliberately non-conforming source: its second read goes backwards.
    struct Backwards {
        reads: Cell<usize>,
    }

    impl CycleSource for Backwards {
        fn read_cycles(&self) -> u64 {
            let n = self.reads.get();
            self.reads.set(n + 1);
            if n == 0 {
                1_000
            } else {
                500
            }
        }
    }

    struct KnownTimebase(Option<u32>);

    impl Timebase for KnownTimebase {
        fn cycles_per_us(&self) -> Option<u32> {
            self.0
        }
    }

    #[test]
    fn a_conforming_source_passes_and_reports_its_span() {
        let source = StepSource::new(0, 7);
        assert_eq!(check(&source, 11), Ok(70));
    }

    #[test]
    fn a_stuck_source_fails_conformance() {
        let source = StepSource::new(42, 0);
        assert_eq!(check(&source, 64), Err(ConformanceFailure::NoForwardProgress { samples: 64 }));
    }

    #[test]
    fn a_backwards_source_fails_conformance() {
        let source = Backwards { reads: Cell::new(0) };
        assert_eq!(
            check(&source, 8),
            Err(ConformanceFailure::WentBackwards { previous: 1_000, observed: 500 })
        );
    }

    #[test]
    fn fewer_than_two_samples_cannot_establish_conformance() {
        let source = StepSource::new(0, 1);
        assert_eq!(check(&source, 1), Err(ConformanceFailure::TooFewSamples { samples: 1 }));
        assert_eq!(check(&source, 0), Err(ConformanceFailure::TooFewSamples { samples: 0 }));
    }

    #[test]
    fn a_timebase_may_honestly_report_that_it_does_not_know() {
        assert_eq!(KnownTimebase(None).cycles_per_us(), None);
        assert_eq!(KnownTimebase(Some(2_500)).cycles_per_us(), Some(2_500));
    }
}
