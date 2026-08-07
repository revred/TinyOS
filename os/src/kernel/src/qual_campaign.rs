//! The Q3 campaign's arithmetic: unaccounted physical time per window, and
//! the refusals that keep a zero honest (`ADR 0005`, `12A` §0).
//!
//! `REPORT-2026-08-07-01` holds Q1, Q2 and Q4 of the first platform
//! qualification record and says exactly what Q3 still needs: a campaign with
//! a stated duration and sample count, a distribution of **unaccounted
//! physical ticks per window**, and — the ADR's own trap clause — a silicon
//! positive control first, because *a zero from an instrument never shown to
//! produce a non-zero is an absence of measurement, not a measurement of
//! zero*. This module is the arithmetic half of both arms, host-tested here
//! so the board fixture (`fixture_measure_arm64`) only drives probes and
//! prints.
//!
//! # What "unaccounted" means, in this module's own terms
//!
//! Each window ([`hal_arm64::timer::probe_residency_window`]'s shape) carries
//! three advances over the same interval: the physical counter's
//! (`CNTPCT_EL0` — un-offsettable, the window's own anchor), the virtual
//! counter's, and the PMU cycle counter's. `PMCCNTR_EL0` counts at EL1 and
//! not in the secure world, so time spent in EL3 advances the physical
//! counter while the cycle counter stands still. The unaccounted ticks of a
//! window are therefore its physical ticks minus the ticks its PMU advance
//! accounts for at the campaign's own measured rate:
//!
//! ```text
//! unaccounted_i = cntpct_ticks_i − pmccntr_delta_i × 1000 / pmu_per_1000_ticks
//! ```
//!
//! The rate is the **median** of the campaign's own per-window ratios —
//! self-calibrated, never quoted from a datasheet (the no-bench-constant
//! rule), and robust exactly because an excursion window is the outlier the
//! median discards. The counter-split disagreement (`cntpct − cntvct`, the
//! `LE-103` channel) is carried beside it as its own maximum, because a moved
//! `CNTVOFF_EL2` is a different hiding place than a paused PMU.
//!
//! # The refusals are the instrument's honesty
//!
//! A campaign with no windows, a window that never closed (a stuck counter
//! tripped the spin bound), or a PMU that never advanced yields a named
//! [`QualRefusal`] — never a distribution of zeros, which would read as the
//! cleanest possible pass while measuring nothing.

use crate::measure::percentile;
use core::fmt;

/// One window's three counter advances, as the probe reported them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSample {
    /// The window's width in physical-counter ticks.
    pub cntpct_ticks: u64,
    /// The same window in virtual-counter ticks.
    pub cntvct_ticks: u64,
    /// `PMCCNTR_EL0`'s advance across the window.
    pub pmccntr_delta: u64,
}

impl WindowSample {
    /// The all-zero sample, for static buffers awaiting their probe.
    pub const ZERO: WindowSample =
        WindowSample { cntpct_ticks: 0, cntvct_ticks: 0, pmccntr_delta: 0 };
}

/// Why a campaign refused to summarize — each a distinct, wire-printable
/// reason, because a refused campaign is filed as a finding, not retried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualRefusal {
    /// No windows were collected at all.
    NoWindows,
    /// A window's physical advance is zero: the spin bound tripped on a stuck
    /// counter, and a distribution over broken windows is about nothing.
    WindowNeverClosed,
    /// The median PMU rate is zero: the cycle counter never advanced, so no
    /// physical tick can be accounted for and every window would read as one
    /// giant excursion — which is an instrument failure, not a measurement.
    PmuDead,
    /// The caller's scratch buffer is smaller than the sample set.
    ScratchTooSmall,
}

impl QualRefusal {
    /// The `reason=` token the fixture prints.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            QualRefusal::NoWindows => "no_windows",
            QualRefusal::WindowNeverClosed => "window_never_closed",
            QualRefusal::PmuDead => "pmu_dead",
            QualRefusal::ScratchTooSmall => "scratch_too_small",
        }
    }
}

/// What one campaign (or one control arm's idle set) measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignSummary {
    /// Windows summarized.
    pub windows: u32,
    /// The commanded window width, physical ticks.
    pub window_ticks: u64,
    /// The self-calibrated PMU rate: median cycles per 1000 physical ticks.
    pub pmu_per_1000_ticks: u64,
    /// Smallest per-window unaccounted physical ticks.
    pub unaccounted_min: u64,
    /// Median.
    pub unaccounted_p50: u64,
    /// 99th percentile (nearest-rank, [`percentile`]'s own convention).
    pub unaccounted_p99: u64,
    /// 99.9th percentile.
    pub unaccounted_p99_9: u64,
    /// Largest — the excursion the record quotes against a claimed bound.
    pub unaccounted_max: u64,
    /// Largest per-window `|cntpct − cntvct|` disagreement: the moved-offset
    /// channel, carried separately from the paused-PMU channel.
    pub offset_disagreement_max: u64,
}

/// One window's unaccounted physical ticks at a given PMU rate.
///
/// Saturating: a window whose PMU advance accounts for *more* than its
/// physical width (rate jitter in the flattering direction) is zero
/// unaccounted ticks, never a wrapped enormity.
#[must_use]
pub fn unaccounted_ticks(sample: &WindowSample, pmu_per_1000_ticks: u64) -> u64 {
    if pmu_per_1000_ticks == 0 {
        return sample.cntpct_ticks;
    }
    sample
        .cntpct_ticks
        .saturating_sub(sample.pmccntr_delta.saturating_mul(1000) / pmu_per_1000_ticks)
}

/// Summarizes a campaign's windows, or refuses by name.
///
/// `scratch` must be at least `samples.len()` — it holds the sorted ratios
/// and then the sorted unaccounted values, so the whole summary is
/// allocation-free at any campaign size the caller can statically afford.
///
/// # Errors
///
/// See [`QualRefusal`] — each names the property whose absence would
/// otherwise masquerade as a clean zero.
pub fn summarize(
    samples: &[WindowSample],
    scratch: &mut [u64],
    window_ticks: u64,
) -> Result<CampaignSummary, QualRefusal> {
    if samples.is_empty() {
        return Err(QualRefusal::NoWindows);
    }
    if scratch.len() < samples.len() {
        return Err(QualRefusal::ScratchTooSmall);
    }
    let scratch = &mut scratch[..samples.len()];

    // Pass one: the self-calibrated rate, as the median per-window ratio.
    for (slot, sample) in scratch.iter_mut().zip(samples) {
        if sample.cntpct_ticks == 0 {
            return Err(QualRefusal::WindowNeverClosed);
        }
        *slot = sample.pmccntr_delta.saturating_mul(1000) / sample.cntpct_ticks;
    }
    scratch.sort_unstable();
    let ratio = percentile(scratch, 1, 2).unwrap_or(0);
    if ratio == 0 {
        return Err(QualRefusal::PmuDead);
    }

    // Pass two: per-window unaccounted physical ticks against that rate,
    // and the moved-offset channel's own maximum alongside.
    let mut offset_disagreement_max: u64 = 0;
    for (slot, sample) in scratch.iter_mut().zip(samples) {
        *slot = unaccounted_ticks(sample, ratio);
        let disagreement = sample.cntpct_ticks.abs_diff(sample.cntvct_ticks);
        offset_disagreement_max = offset_disagreement_max.max(disagreement);
    }
    scratch.sort_unstable();

    // The refusals above make every `percentile` below infallible; refusing
    // again here would invent an unreachable arm.
    Ok(CampaignSummary {
        windows: samples.len() as u32,
        window_ticks,
        pmu_per_1000_ticks: ratio,
        unaccounted_min: scratch[0],
        unaccounted_p50: percentile(scratch, 1, 2).unwrap_or(0),
        unaccounted_p99: percentile(scratch, 99, 100).unwrap_or(0),
        unaccounted_p99_9: percentile(scratch, 999, 1_000).unwrap_or(0),
        unaccounted_max: scratch[scratch.len() - 1],
        offset_disagreement_max,
    })
}

/// The ADR's trap clause as a predicate: the control window's excursion must
/// stand strictly above **every** idle window's, and must be nonzero.
///
/// Strictly-above-the-idle-maximum rather than a margin constant, because a
/// margin would be a bench-tuned number and the bench evidence — an SMC
/// round-trip is tens-to-hundreds of ticks against single-digit idle jitter —
/// is exactly what the control boot exists to establish.
#[must_use]
pub fn control_seen(idle: &CampaignSummary, control_unaccounted: u64) -> bool {
    control_unaccounted > idle.unaccounted_max && control_unaccounted > 0
}

/// Writes the campaign's one wire line.
///
/// `TOS64-QUAL/1` framing so ti64dink's harvest carries it into the same
/// evidence file as the envelope, and `timing.rs`'s `TOS64-MEAS` sentinel
/// never matches it — the parser stays untouched, the same argument the
/// first three QUAL lines made on 2026-08-07.
///
/// # Errors
///
/// Propagates the sink's own error, as every envelope writer does.
pub fn write_campaign_line<W: fmt::Write>(sink: &mut W, summary: &CampaignSummary) -> fmt::Result {
    writeln!(
        sink,
        "TOS64-QUAL/1 campaign windows={} window_ticks={} pmu_per_1000_ticks={} \
         unaccounted_min={} unaccounted_p50={} unaccounted_p99={} unaccounted_p99_9={} \
         unaccounted_max={} offset_disagreement_max={}",
        summary.windows,
        summary.window_ticks,
        summary.pmu_per_1000_ticks,
        summary.unaccounted_min,
        summary.unaccounted_p50,
        summary.unaccounted_p99,
        summary.unaccounted_p99_9,
        summary.unaccounted_max,
        summary.offset_disagreement_max,
    )
}

/// Writes the SMC positive-control arm's one wire line.
///
/// Carries both halves the verdict folds together — `event_fired` (the SMC
/// was actually issued mid-window) and `seen` (its excursion stood above
/// every idle window) — plus the PSCI version word the call returned, which
/// is itself Q2 corroboration on the wire.
///
/// # Errors
///
/// Propagates the sink's own error.
pub fn write_control_line<W: fmt::Write>(
    sink: &mut W,
    psci_version: u64,
    idle: &CampaignSummary,
    control_unaccounted: u64,
    event_fired: bool,
    seen: bool,
) -> fmt::Result {
    writeln!(
        sink,
        "TOS64-QUAL/1 smc_control psci_version={psci_version:#010x} idle_windows={} \
         idle_unaccounted_max={} control_unaccounted={control_unaccounted} \
         event_fired={event_fired} seen={seen}",
        idle.windows, idle.unaccounted_max,
    )
}

/// Writes a refusal line for either arm — the campaign's failure filed on the
/// wire rather than a silent absence.
///
/// # Errors
///
/// Propagates the sink's own error.
pub fn write_refusal_line<W: fmt::Write>(
    sink: &mut W,
    arm: &str,
    refusal: QualRefusal,
) -> fmt::Result {
    writeln!(sink, "TOS64-QUAL/1 {arm} REFUSED reason={}", refusal.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sample whose PMU advance accounts for exactly its physical width at
    /// `ratio` cycles per 1000 ticks.
    fn idle(cntpct: u64, ratio: u64) -> WindowSample {
        WindowSample {
            cntpct_ticks: cntpct,
            cntvct_ticks: cntpct,
            pmccntr_delta: cntpct * ratio / 1000,
        }
    }

    /// A sample with `stolen` physical ticks the PMU did not witness.
    fn excursion(cntpct: u64, ratio: u64, stolen: u64) -> WindowSample {
        WindowSample {
            cntpct_ticks: cntpct,
            cntvct_ticks: cntpct,
            pmccntr_delta: (cntpct - stolen) * ratio / 1000,
        }
    }

    #[test]
    fn an_idle_campaign_summarizes_to_zero_unaccounted() {
        // The Pi 5's own numbers: 540,000-tick windows, PMCCNTR at 2400 MHz
        // against 54 MHz — 44,444 cycles per 1000 ticks.
        let samples = [idle(540_000, 44_444); 32];
        let mut scratch = [0u64; 32];
        let summary = summarize(&samples, &mut scratch, 540_000).expect("idle windows summarize");
        assert_eq!(summary.windows, 32);
        assert_eq!(summary.window_ticks, 540_000);
        assert_eq!(summary.pmu_per_1000_ticks, 44_444);
        assert_eq!(summary.unaccounted_min, 0);
        assert_eq!(summary.unaccounted_p50, 0);
        assert_eq!(summary.unaccounted_max, 0);
        assert_eq!(summary.offset_disagreement_max, 0);
    }

    #[test]
    fn an_excursion_window_surfaces_in_the_max_and_not_the_median() {
        let mut samples = [idle(540_000, 44_444); 100];
        // One window lost ~100 physical ticks to the secure world.
        samples[41] = excursion(540_000, 44_444, 100);
        let mut scratch = [0u64; 100];
        let summary = summarize(&samples, &mut scratch, 540_000).expect("summarizes");
        assert_eq!(summary.unaccounted_p50, 0, "one outlier must not move the median");
        // Two integer divisions bracket the reconstruction — one flooring the
        // synthetic PMU delta, one flooring its recovery — so the surfaced
        // excursion sits within two ticks of what was stolen. Against an SMC
        // round-trip of tens-to-hundreds of ticks, that quantum is noise.
        assert!(
            (99..=102).contains(&summary.unaccounted_max),
            "the stolen ticks surface in the max: {}",
            summary.unaccounted_max
        );
    }

    #[test]
    fn the_rate_is_the_median_so_excursions_do_not_recalibrate_the_ruler() {
        // Half idle at the true rate; one excursion window. A mean would drag
        // the rate down and smear the excursion across every window.
        let mut samples = [idle(1_000_000, 50_000); 11];
        samples[5] = excursion(1_000_000, 50_000, 400_000);
        let mut scratch = [0u64; 11];
        let summary = summarize(&samples, &mut scratch, 1_000_000).expect("summarizes");
        assert_eq!(summary.pmu_per_1000_ticks, 50_000);
        assert_eq!(summary.unaccounted_min, 0);
        assert_eq!(summary.unaccounted_max, 400_000);
    }

    #[test]
    fn the_offset_channel_is_carried_separately() {
        let mut samples = [idle(540_000, 44_444); 8];
        // One window where the virtual counter advanced 40 ticks less: a
        // moved CNTVOFF_EL2, the LE-103 channel — visible even though the
        // PMU accounted for every physical tick.
        samples[3].cntvct_ticks = 540_000 - 40;
        let mut scratch = [0u64; 8];
        let summary = summarize(&samples, &mut scratch, 540_000).expect("summarizes");
        assert_eq!(summary.offset_disagreement_max, 40);
        assert_eq!(summary.unaccounted_max, 0, "the PMU channel stays clean");
    }

    #[test]
    fn no_windows_is_a_named_refusal() {
        let mut scratch = [0u64; 0];
        assert_eq!(summarize(&[], &mut scratch, 540_000), Err(QualRefusal::NoWindows));
    }

    #[test]
    fn a_window_that_never_closed_is_a_named_refusal() {
        let mut samples = [idle(540_000, 44_444); 4];
        samples[2].cntpct_ticks = 0; // the spin bound tripped
        let mut scratch = [0u64; 4];
        assert_eq!(summarize(&samples, &mut scratch, 540_000), Err(QualRefusal::WindowNeverClosed));
    }

    #[test]
    fn a_dead_pmu_is_a_named_refusal_not_a_distribution_of_excursions() {
        let samples =
            [WindowSample { cntpct_ticks: 540_000, cntvct_ticks: 540_000, pmccntr_delta: 0 }; 4];
        let mut scratch = [0u64; 4];
        assert_eq!(summarize(&samples, &mut scratch, 540_000), Err(QualRefusal::PmuDead));
    }

    #[test]
    fn a_short_scratch_is_a_named_refusal_not_a_partial_summary() {
        let samples = [idle(540_000, 44_444); 4];
        let mut scratch = [0u64; 3];
        assert_eq!(summarize(&samples, &mut scratch, 540_000), Err(QualRefusal::ScratchTooSmall));
    }

    #[test]
    fn unaccounted_saturates_rather_than_wrapping() {
        // A window whose PMU accounts for MORE than its width — rate jitter
        // in the flattering direction — is zero, never a wrapped enormity.
        let generous = WindowSample {
            cntpct_ticks: 540_000,
            cntvct_ticks: 540_000,
            pmccntr_delta: 24_010_000,
        };
        assert_eq!(unaccounted_ticks(&generous, 44_444), 0);
    }

    #[test]
    fn the_control_is_seen_only_strictly_above_every_idle_window() {
        let samples = [idle(540_000, 44_444); 16];
        let mut scratch = [0u64; 16];
        let idle_summary = summarize(&samples, &mut scratch, 540_000).expect("summarizes");
        assert!(!control_seen(&idle_summary, 0), "a zero control saw nothing");
        assert!(
            !control_seen(&idle_summary, idle_summary.unaccounted_max),
            "equal to the idle max is not above it"
        );
        assert!(control_seen(&idle_summary, idle_summary.unaccounted_max + 1));
    }

    #[test]
    fn the_campaign_line_is_exact_bytes() {
        let summary = CampaignSummary {
            windows: 6_000,
            window_ticks: 540_000,
            pmu_per_1000_ticks: 44_444,
            unaccounted_min: 0,
            unaccounted_p50: 0,
            unaccounted_p99: 1,
            unaccounted_p99_9: 3,
            unaccounted_max: 9,
            offset_disagreement_max: 2,
        };
        let mut sink = String::new();
        write_campaign_line(&mut sink, &summary).expect("writes");
        assert_eq!(
            sink,
            "TOS64-QUAL/1 campaign windows=6000 window_ticks=540000 pmu_per_1000_ticks=44444 \
             unaccounted_min=0 unaccounted_p50=0 unaccounted_p99=1 unaccounted_p99_9=3 \
             unaccounted_max=9 offset_disagreement_max=2\n"
        );
    }

    #[test]
    fn the_control_line_is_exact_bytes_and_carries_both_halves() {
        let idle_summary = CampaignSummary {
            windows: 16,
            window_ticks: 540_000,
            pmu_per_1000_ticks: 44_444,
            unaccounted_min: 0,
            unaccounted_p50: 0,
            unaccounted_p99: 1,
            unaccounted_p99_9: 1,
            unaccounted_max: 2,
            offset_disagreement_max: 1,
        };
        let mut sink = String::new();
        write_control_line(&mut sink, 0x0001_0001, &idle_summary, 137, true, true).expect("writes");
        assert_eq!(
            sink,
            "TOS64-QUAL/1 smc_control psci_version=0x00010001 idle_windows=16 \
             idle_unaccounted_max=2 control_unaccounted=137 event_fired=true seen=true\n"
        );
    }

    #[test]
    fn the_refusal_line_names_its_reason() {
        let mut sink = String::new();
        write_refusal_line(&mut sink, "campaign", QualRefusal::PmuDead).expect("writes");
        assert_eq!(sink, "TOS64-QUAL/1 campaign REFUSED reason=pmu_dead\n");
    }
}
