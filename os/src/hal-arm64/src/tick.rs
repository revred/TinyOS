//! The tick's arithmetic and report lines (`STORY-P1-07-04`), plus the
//! interrupt-side state — the pure half of what the GIC and the virtual
//! timer make possible.
//!
//! `TEST-P1-07-04-A` clause 1: the tick is verified **by ratio between
//! consecutive intervals**, never by absolute value. An absolute check on a
//! first bring-up conflates a wrong divisor, a wrong clock source and a
//! wrong frequency register into one indistinguishable failure; a ratio
//! near 1 proves the period is *stable*, and the declared interval is
//! reported alongside without being the pass condition. The uniform-factor
//! error a ratio deliberately lets through is caught by clause 4's measured
//! rates ([`pmu_line`] carries them).
//!
//! Everything here except the `record_tick`/`snapshot` statics at the bottom
//! is pure and host-tested (`SEC-19`). The handler-side state is a fixed
//! ring of atomics — bounded, allocation-free (`SEC-20`), single-writer (the
//! IRQ handler) with a single racing reader (the park loop), both on one
//! core.

use crate::mmu::{push, push_decimal};

/// The declared tick interval, in generic-timer ticks: 10 ms at the 54 MHz
/// this board's `CNTFRQ_EL0` is expected to read. Declared, reported, and
/// deliberately **not** the pass condition (clause 1).
pub const TICK_INTERVAL_TICKS: u32 = 540_000;

/// Intervals kept for the ratio check. A power of two, so the writer's index
/// arithmetic is a mask.
pub const INTERVAL_RING_SIZE: usize = 8;

/// Capacity of every report line this module renders.
pub const LINE_CAPACITY: usize = 96;

/// The ratio bounds across consecutive intervals, in per-mille: for each
/// adjacent pair, `1000 × current ÷ previous`, folded to the smallest and
/// largest observed. `None` until two non-zero intervals exist — a ratio
/// over silence would be a confident claim about nothing.
#[must_use]
pub fn ratio_bounds_per_mille(intervals: &[u64]) -> Option<(u64, u64)> {
    let mut bounds: Option<(u64, u64)> = None;
    let mut previous: Option<u64> = None;
    for &interval in intervals {
        if interval == 0 {
            // An unwritten ring slot, not a measurement — and a hole ends
            // the "consecutive" property, so the pair chain restarts.
            previous = None;
            continue;
        }
        if let Some(before) = previous {
            let ratio = interval.saturating_mul(1000) / before;
            bounds = Some(match bounds {
                None => (ratio, ratio),
                Some((rmin, rmax)) => (rmin.min(ratio), rmax.max(ratio)),
            });
        }
        previous = Some(interval);
    }
    bounds
}

/// Renders the live tick line:
/// `TOS64-TICK/1 count=<n> tval=<declared> rmin=<pm> rmax=<pm>\n`, with
/// `rmin`/`rmax` reading `none` until the ring holds two intervals.
#[must_use]
pub fn tick_line(
    count: u32,
    declared_interval: u32,
    bounds: Option<(u64, u64)>,
) -> ([u8; LINE_CAPACITY], usize) {
    let mut line = [0u8; LINE_CAPACITY];
    let mut len = 0;
    push(&mut line, &mut len, b"TOS64-TICK/1 count=");
    push_decimal(&mut line, &mut len, u64::from(count));
    push(&mut line, &mut len, b" tval=");
    push_decimal(&mut line, &mut len, u64::from(declared_interval));
    match bounds {
        Some((rmin, rmax)) => {
            push(&mut line, &mut len, b" rmin=");
            push_decimal(&mut line, &mut len, rmin);
            push(&mut line, &mut len, b" rmax=");
            push_decimal(&mut line, &mut len, rmax);
        }
        None => push(&mut line, &mut len, b" rmin=none rmax=none"),
    }
    push(&mut line, &mut len, b"\n");
    (line, len)
}

/// Renders the refused tick line:
/// `TOS64-TICK/1 refused=<register> readback=<hex8>\n` — the GIC register
/// whose readback disagreed, so a dead tick is a diagnosis, not a hang.
#[must_use]
pub fn tick_refused_line(refused: crate::gic::GicRefused) -> ([u8; LINE_CAPACITY], usize) {
    let mut line = [0u8; LINE_CAPACITY];
    let mut len = 0;
    push(&mut line, &mut len, b"TOS64-TICK/1 refused=");
    push(&mut line, &mut len, refused.as_str().as_bytes());
    push(&mut line, &mut len, b" readback=");
    push(&mut line, &mut len, &crate::pl011::hex_u64(u64::from(refused.readback()))[8..]);
    push(&mut line, &mut len, b"\n");
    (line, len)
}

/// Renders the conformance line (`TEST-P1-07-04-A` clauses 2 and 5):
/// `TOS64-CONF/1 cntvct=pass span=<n> cntfrq=<hz> cpus=<factor|none>\n`, or
/// `cntvct=<failure>` with the failure named. `cpus=none` is a **pass for
/// the code and a finding about the firmware** — honest absence surviving
/// silicon is the clause, so the raw register rides alongside whatever the
/// judgement said.
#[must_use]
pub fn conformance_line(
    outcome: Result<u64, hal::time::conformance::ConformanceFailure>,
    cntfrq_raw: u64,
    cycles_per_us: Option<u32>,
) -> ([u8; LINE_CAPACITY], usize) {
    use hal::time::conformance::ConformanceFailure;
    let mut line = [0u8; LINE_CAPACITY];
    let mut len = 0;
    push(&mut line, &mut len, b"TOS64-CONF/1 cntvct=");
    match outcome {
        Ok(span) => {
            push(&mut line, &mut len, b"pass span=");
            push_decimal(&mut line, &mut len, span);
        }
        Err(ConformanceFailure::NoForwardProgress { samples }) => {
            push(&mut line, &mut len, b"stuck samples=");
            push_decimal(&mut line, &mut len, samples as u64);
        }
        Err(ConformanceFailure::WentBackwards { previous, observed }) => {
            push(&mut line, &mut len, b"backwards previous=");
            push_decimal(&mut line, &mut len, previous);
            push(&mut line, &mut len, b" observed=");
            push_decimal(&mut line, &mut len, observed);
        }
        Err(ConformanceFailure::TooFewSamples { samples }) => {
            push(&mut line, &mut len, b"toofew samples=");
            push_decimal(&mut line, &mut len, samples as u64);
        }
    }
    push(&mut line, &mut len, b" cntfrq=");
    push_decimal(&mut line, &mut len, cntfrq_raw);
    match cycles_per_us {
        Some(factor) => {
            push(&mut line, &mut len, b" cpus=");
            push_decimal(&mut line, &mut len, u64::from(factor));
        }
        None => push(&mut line, &mut len, b" cpus=none"),
    }
    push(&mut line, &mut len, b"\n");
    (line, len)
}

/// Renders the counter-decision line (`TEST-P1-07-04-A` clauses 3 and 4):
/// `TOS64-PMU/1 delta=<cycles> rate=<mhz|none> source=<decision>\n`.
/// `delta` is what `PMCCNTR_EL0` advanced across the probe window; `rate` is
/// that advance divided by the window's wall time (the *measured* rate,
/// clause 4 — never the manual's); `source` is [`cycle_source_decision`]'s
/// verdict.
#[must_use]
pub fn pmu_line(pmccntr_delta: u64, rate_mhz: Option<u64>) -> ([u8; LINE_CAPACITY], usize) {
    let mut line = [0u8; LINE_CAPACITY];
    let mut len = 0;
    push(&mut line, &mut len, b"TOS64-PMU/1 delta=");
    push_decimal(&mut line, &mut len, pmccntr_delta);
    match rate_mhz {
        Some(rate) => {
            push(&mut line, &mut len, b" rate=");
            push_decimal(&mut line, &mut len, rate);
            push(&mut line, &mut len, b"mhz");
        }
        None => push(&mut line, &mut len, b" rate=none"),
    }
    push(&mut line, &mut len, b" source=");
    push(&mut line, &mut len, cycle_source_decision(pmccntr_delta).as_bytes());
    push(&mut line, &mut len, b"\n");
    (line, len)
}

/// The recorded `LE-15` decision meeting registers that may refuse it: a
/// `PMCCNTR_EL0` that advanced becomes the microbenchmark `CycleSource`; one
/// that trapped into silence or read a constant does **not fail the Story**
/// — it takes the `CNTVCT_EL0`-with-batching fallback and narrows `LE-15`
/// instead (clause 3). This function is the whole decision, pure, so the
/// fallback path is exercised on the host rather than assumed.
#[must_use]
pub const fn cycle_source_decision(pmccntr_delta: u64) -> &'static str {
    if pmccntr_delta == 0 {
        return "cntvct-fallback";
    }
    "pmccntr"
}

/// The measured rate of a counter across a probe window, in MHz:
/// `delta × timebase_hz ÷ window_ticks ÷ 1e6`, or `None` when the window
/// never opened. Clause 4's arithmetic — the rate is *measured against the
/// other counter*, catching the uniform-factor error clause 1 lets through.
#[must_use]
pub const fn measured_rate_mhz(delta: u64, window_ticks: u64, timebase_hz: u64) -> Option<u64> {
    if window_ticks == 0 || timebase_hz == 0 {
        return None;
    }
    // delta / (window_ticks / timebase_hz) in Hz, then to MHz — reordered so
    // the integer arithmetic keeps its precision: cycles per tick first
    // would truncate to zero for any CPU slower than the timebase × 1e6.
    Some(delta.saturating_mul(timebase_hz / 1_000_000) / window_ticks)
}

// --- aarch64 glue: the interrupt-side state ----------------------------------
//
// A fixed ring of atomics, single writer (the IRQ handler), one racing
// reader (the park loop), one core. Relaxed ordering is sufficient: the
// reader tolerates a torn *set* of intervals (it recomputes every second),
// and each individual load/store is atomic.

#[cfg(target_arch = "aarch64")]
mod state {
    use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    /// Ticks handled since boot.
    pub static COUNT: AtomicU32 = AtomicU32::new(0);
    /// The counter reading at the previous tick, zero until the first.
    pub static LAST_AT: AtomicU64 = AtomicU64::new(0);
    /// The most recent intervals, a ring indexed by `COUNT`.
    pub static INTERVALS: [AtomicU64; super::INTERVAL_RING_SIZE] = [
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
    ];
    /// INTIDs that arrived without being the timer — counted, never fatal.
    pub static UNEXPECTED: AtomicU32 = AtomicU32::new(0);

    /// Records one tick at counter reading `now`.
    pub fn record(now: u64) {
        let previous = LAST_AT.swap(now, Ordering::Relaxed);
        if previous != 0 {
            let count = COUNT.load(Ordering::Relaxed) as usize;
            INTERVALS[count % super::INTERVAL_RING_SIZE]
                .store(now.wrapping_sub(previous), Ordering::Relaxed);
        }
        COUNT.fetch_add(1, Ordering::Relaxed);
    }

    /// The tick count and the interval ring, copied out.
    pub fn snapshot() -> (u32, [u64; super::INTERVAL_RING_SIZE]) {
        let mut intervals = [0u64; super::INTERVAL_RING_SIZE];
        for (slot, value) in intervals.iter_mut().enumerate() {
            *value = INTERVALS[slot].load(Ordering::Relaxed);
        }
        (COUNT.load(Ordering::Relaxed), intervals)
    }
}

/// Records one tick — called from the IRQ handler and nowhere else.
#[cfg(target_arch = "aarch64")]
pub fn record_tick(now: u64) {
    state::record(now);
}

/// Counts an interrupt that was not the timer. Never fatal: it is retired
/// at the GIC and remembered here for the report.
#[cfg(target_arch = "aarch64")]
pub fn record_unexpected() {
    state::UNEXPECTED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
}

/// The live tick line from the current state — what the park loop paints
/// every second at its pinned canvas row.
#[cfg(target_arch = "aarch64")]
#[must_use]
pub fn status_line() -> ([u8; LINE_CAPACITY], usize) {
    let (count, intervals) = state::snapshot();
    tick_line(count, TICK_INTERVAL_TICKS, ratio_bounds_per_mille(&intervals))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hal::time::conformance::ConformanceFailure;

    // ---- clause 1: ratio, not absolute -------------------------------------

    #[test]
    fn a_stable_period_bounds_its_ratios_tightly_around_one_thousand() {
        let intervals = [540_000u64, 540_270, 539_730, 540_000];
        let (rmin, rmax) = ratio_bounds_per_mille(&intervals).expect("four intervals");
        assert!((999..=1000).contains(&rmin), "rmin {rmin}");
        assert!((1000..=1001).contains(&rmax), "rmax {rmax}");
    }

    #[test]
    fn a_uniformly_wrong_period_still_passes_the_ratio_and_must() {
        // Clause 1's own caveat: every interval wrong by the same factor is
        // a divisor error, it is real, and the ratio deliberately passes it
        // — clause 4's measured rate is where it is caught.
        let intervals = [1_080_000u64, 1_080_000, 1_080_000];
        assert_eq!(ratio_bounds_per_mille(&intervals), Some((1000, 1000)));
    }

    #[test]
    fn an_erratic_period_is_convicted_by_its_bounds() {
        let intervals = [540_000u64, 270_000, 1_080_000];
        let (rmin, rmax) = ratio_bounds_per_mille(&intervals).expect("three intervals");
        assert_eq!(rmin, 500);
        assert_eq!(rmax, 4000);
    }

    #[test]
    fn fewer_than_two_intervals_is_no_ratio_not_a_perfect_one() {
        assert_eq!(ratio_bounds_per_mille(&[]), None);
        assert_eq!(ratio_bounds_per_mille(&[540_000]), None);
        // Zeroes are unwritten ring slots, not measurements.
        assert_eq!(ratio_bounds_per_mille(&[0, 0, 0]), None);
        assert_eq!(ratio_bounds_per_mille(&[540_000, 0]), None);
    }

    // ---- the lines: exact bytes --------------------------------------------

    #[test]
    fn the_tick_line_is_exact_bytes() {
        let (line, len) = tick_line(1234, TICK_INTERVAL_TICKS, Some((999, 1001)));
        assert_eq!(
            &line[..len],
            b"TOS64-TICK/1 count=1234 tval=540000 rmin=999 rmax=1001\n" as &[u8]
        );
        let (line, len) = tick_line(0, TICK_INTERVAL_TICKS, None);
        assert_eq!(
            &line[..len],
            b"TOS64-TICK/1 count=0 tval=540000 rmin=none rmax=none\n" as &[u8]
        );
    }

    #[test]
    fn the_refused_tick_line_names_the_register_and_its_readback() {
        let (line, len) = tick_refused_line(crate::gic::GicRefused::MaskNotHeld(0x0000_00A5));
        assert_eq!(&line[..len], b"TOS64-TICK/1 refused=gicc-pmr readback=000000A5\n" as &[u8]);
    }

    #[test]
    fn the_conformance_line_is_exact_bytes_in_both_arms() {
        let (line, len) = conformance_line(Ok(118), 54_000_000, Some(54));
        assert_eq!(
            &line[..len],
            b"TOS64-CONF/1 cntvct=pass span=118 cntfrq=54000000 cpus=54\n" as &[u8]
        );
        // Honest absence: the raw register is still on the line.
        let (line, len) = conformance_line(Ok(90), 0, None);
        assert_eq!(&line[..len], b"TOS64-CONF/1 cntvct=pass span=90 cntfrq=0 cpus=none\n" as &[u8]);
        let (line, len) =
            conformance_line(Err(ConformanceFailure::NoForwardProgress { samples: 64 }), 0, None);
        assert_eq!(
            &line[..len],
            b"TOS64-CONF/1 cntvct=stuck samples=64 cntfrq=0 cpus=none\n" as &[u8]
        );
        let (line, len) = conformance_line(
            Err(ConformanceFailure::WentBackwards { previous: 100, observed: 90 }),
            54_000_000,
            Some(54),
        );
        assert_eq!(
            &line[..len],
            b"TOS64-CONF/1 cntvct=backwards previous=100 observed=90 cntfrq=54000000 cpus=54\n"
                as &[u8]
        );
    }

    #[test]
    fn the_pmu_line_carries_the_decision_in_both_arms() {
        let (line, len) = pmu_line(24_000_000, Some(2400));
        assert_eq!(
            &line[..len],
            b"TOS64-PMU/1 delta=24000000 rate=2400mhz source=pmccntr\n" as &[u8]
        );
        // Clause 3: a dead PMU is a finding, not a failure — the fallback is
        // named on the line.
        let (line, len) = pmu_line(0, None);
        assert_eq!(
            &line[..len],
            b"TOS64-PMU/1 delta=0 rate=none source=cntvct-fallback\n" as &[u8]
        );
    }

    // ---- clauses 3 and 4: decision and measured rate -----------------------

    #[test]
    fn the_decision_prefers_pmccntr_and_takes_the_fallback_deliberately() {
        assert_eq!(cycle_source_decision(24_000_000), "pmccntr");
        // The fallback path exercised on the host, per clause 3's "tested
        // rather than assumed".
        assert_eq!(cycle_source_decision(0), "cntvct-fallback");
    }

    #[test]
    fn the_measured_rate_is_the_cross_counter_arithmetic() {
        // 24e6 PMU cycles across 540_000 ticks of a 54 MHz timebase = 10 ms
        // of wall time = 2.4 GHz.
        assert_eq!(measured_rate_mhz(24_000_000, 540_000, 54_000_000), Some(2400));
        // A window that never opened measures nothing.
        assert_eq!(measured_rate_mhz(24_000_000, 0, 54_000_000), None);
        assert_eq!(measured_rate_mhz(24_000_000, 540_000, 0), None);
    }
}
