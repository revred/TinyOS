//! Cycle-calibrated measurement harness (`STORY-P1-01-01`) — the ruler every
//! other `EPIC-P1` Feature's exit criteria are stated in.
//!
//! Generalizes what `STORY-P0-03-01`'s `fixture_pool_bench` prototyped as a
//! one-off (calibrated `RDTSC` overhead, fixed-capacity sample buffers,
//! integer nearest-rank percentiles, percentile lines over COM1) into the
//! kernel's standing measurement primitive, with three deliberate changes
//! that the one-off got wrong for reuse:
//!
//! 1. **Arch-neutral by construction.** Nothing here names `RDTSC`; the cycle
//!    source is [`hal::time::CycleSource`], so the ARM64/Pi 5 slice tracked as
//!    loose end `LE-09` reuses this module unchanged.
//! 2. **Drop accounting instead of silent truncation.** A sample offered to a
//!    full buffer increments [`Samples::dropped`] and is reported downstream;
//!    the one-off simply indexed a caller-sized array and trusted the caller.
//! 3. **A versioned, parseable envelope** ([`Report`]) instead of prose lines,
//!    so `xtask` can fail closed on a malformed stream (`BND-15`/`-16`/`-17`)
//!    rather than silently reading zero samples out of garbage.
//!
//! Real-time discipline (`agent/CODING_STANDARDS.md#real-time-discipline-kernel-and-driver-code`):
//! no allocation anywhere (every buffer is caller-owned, fixed-capacity), no
//! unbounded loop (every count is a caller-supplied bound), no lock, and no
//! panic on any measurement path — `summarize` returns `None` for an empty
//! buffer rather than indexing it, and overhead correction saturates rather
//! than wrapping.
//!
//! **Perturbation bound.** The harness's own cost is one [`CycleSource`] read
//! before and one after the timed region, plus one bounds-checked store into
//! the sample buffer. [`Calibration`] measures the paired-read cost directly
//! (minimum of N back-to-back reads) and [`Calibration::correct`] subtracts
//! it, so a reported sample is the timed region's cost plus the store, less
//! the calibrated read pair. That residual store cost is not subtracted and
//! is the documented, un-subtracted part of the bound.

use core::fmt::{self, Write};
use hal::time::CycleSource;

/// The report envelope's version sentinel. Bumping the digit is a breaking
/// change to the format `xtask`'s parser accepts — the parser rejects every
/// version it does not know rather than best-effort parsing an unknown one.
pub const ENVELOPE: &str = "TINYOS-MEAS/1";

/// The unit every percentile in this envelope is denominated in. Cycles, not
/// microseconds: a cycle count is what a [`CycleSource`] can honestly
/// produce, and conversion needs a [`hal::time::Timebase`] that may not
/// exist (see that trait's own doc comment).
pub const UNIT: &str = "cycles";

/// The measured cost of the harness's own paired cycle-source reads.
///
/// Established as the *minimum* observed back-to-back read delta rather than
/// the mean: the minimum is the closest available estimate of the
/// irreducible read cost with no interference, and subtracting a
/// noise-inflated mean would flatter every subsequent measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Calibration {
    overhead_cycles: u64,
}

impl Calibration {
    /// Calibrates against `source` over `samples` back-to-back read pairs.
    ///
    /// `samples` is the caller's explicit bound (no unbounded loop on an RT
    /// path). A `samples` of 0 yields an overhead of 0 — nothing was
    /// measured, so nothing is subtracted, which is the conservative
    /// direction (it over-reports cost rather than under-reporting it).
    pub fn measure<S: CycleSource>(source: &S, samples: usize) -> Self {
        let mut overhead_cycles = u64::MAX;
        for _ in 0..samples {
            let before = source.read_cycles();
            let after = source.read_cycles();
            let delta = after.saturating_sub(before);
            if delta < overhead_cycles {
                overhead_cycles = delta;
            }
        }
        if overhead_cycles == u64::MAX {
            overhead_cycles = 0;
        }
        Calibration { overhead_cycles }
    }

    /// Builds a calibration from an already-known overhead — used by host
    /// tests and by any caller reproducing a previously-reported run.
    pub const fn from_overhead_cycles(overhead_cycles: u64) -> Self {
        Calibration { overhead_cycles }
    }

    /// The calibrated overhead, in cycles.
    pub const fn overhead_cycles(&self) -> u64 {
        self.overhead_cycles
    }

    /// Corrects a raw cycle delta by subtracting the calibrated overhead,
    /// **saturating at zero**: a timed region cheaper than the calibrated
    /// read pair records 0, never a wrapped `u64` that would land in the
    /// far tail of every percentile that followed it.
    pub const fn correct(&self, raw_delta: u64) -> u64 {
        raw_delta.saturating_sub(self.overhead_cycles)
    }
}

/// A fixed-capacity, non-allocating sample buffer with explicit drop
/// accounting.
///
/// `N` is a compile-time capacity, so a `Samples` can live in `.bss` (as the
/// Tier 0 fixtures place it) with no heap and no stack pressure. Offering
/// more than `N` samples never grows, never overwrites, and never silently
/// discards: the surplus lands in [`Samples::dropped`], which every
/// [`Report`] line carries, so over-supply is visible in the artifact rather
/// than invisible in the data (`SEC-20`, exhaustion: the buffer cannot be
/// made to grow by any input).
#[derive(Debug)]
pub struct Samples<const N: usize> {
    data: [u64; N],
    len: usize,
    dropped: usize,
}

impl<const N: usize> Default for Samples<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Samples<N> {
    /// An empty buffer.
    pub const fn new() -> Self {
        Samples { data: [0; N], len: 0, dropped: 0 }
    }

    /// The compile-time capacity.
    pub const fn capacity(&self) -> usize {
        N
    }

    /// How many samples are recorded.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether no sample is recorded.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// How many offered samples were refused because the buffer was full.
    pub const fn dropped(&self) -> usize {
        self.dropped
    }

    /// Records one already-overhead-corrected sample, returning whether it
    /// was stored. A `false` return has already been accounted in
    /// [`Samples::dropped`] — the caller may ignore it, and the artifact
    /// still reports the drop.
    pub fn record(&mut self, cycles: u64) -> bool {
        if self.len == N {
            self.dropped += 1;
            return false;
        }
        self.data[self.len] = cycles;
        self.len += 1;
        true
    }

    /// Discards every recorded sample *and* the drop count, so one buffer can
    /// be reused across measurement phases (the pattern the Tier 0 fixtures
    /// use to keep their static footprint to a single buffer).
    pub fn clear(&mut self) {
        self.len = 0;
        self.dropped = 0;
    }

    /// Sorts the recorded prefix in place and extracts its summary, or `None`
    /// when nothing was recorded.
    ///
    /// Returning `None` rather than a zero-filled `Summary` is deliberate: a
    /// phase that recorded nothing is a measurement *failure*, and a
    /// zero-filled summary would report it as an implausibly fast pass.
    pub fn summarize(&mut self) -> Option<Summary> {
        if self.len == 0 {
            return None;
        }
        let recorded = &mut self.data[..self.len];
        recorded.sort_unstable();
        Some(Summary {
            n: self.len,
            dropped: self.dropped,
            min: recorded[0],
            p50: percentile(recorded, 50, 100)?,
            p99: percentile(recorded, 99, 100)?,
            p99_9: percentile(recorded, 999, 1_000)?,
            max: recorded[self.len - 1],
        })
    }
}

/// Nearest-rank percentile over an already-sorted slice, expressed as the
/// integer fraction `num`/`den` (`50, 100` for p50; `999, 1000` for p99.9)
/// rather than a float — this crate is `#![no_std]` with no `libm`, and
/// nearest-rank needs no floating point at all. `None` for an empty slice.
pub fn percentile(sorted: &[u64], num: usize, den: usize) -> Option<u64> {
    if sorted.is_empty() || den == 0 {
        return None;
    }
    let rank = (sorted.len() - 1).saturating_mul(num) / den;
    sorted.get(rank).copied()
}

/// One phase's percentile summary, carrying exactly the columns
/// `goals/performance/catalogue.tsv` states its budgets in (`p50`, `p99`,
/// `p99.9`, `max`), plus the `n`/`dropped` provenance a reader needs to know
/// how much evidence stands behind them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    /// Samples the summary is computed from.
    pub n: usize,
    /// Samples offered but refused for lack of capacity.
    pub dropped: usize,
    /// Smallest sample.
    pub min: u64,
    /// Nearest-rank 50th percentile.
    pub p50: u64,
    /// Nearest-rank 99th percentile.
    pub p99: u64,
    /// Nearest-rank 99.9th percentile.
    pub p99_9: u64,
    /// Largest sample.
    pub max: u64,
}

/// Times one region between construction and [`Stopwatch::stop`].
///
/// Deliberately a two-call API rather than a closure-taking `time(|| ...)`:
/// the RT paths this Epic measures (`context::switch`, `dispatch::run_once`)
/// are `unsafe` calls around raw context pointers that must not be moved into
/// a closure, and a closure boundary would add its own un-calibrated frame to
/// exactly the measurement it is meant to be invisible to.
#[derive(Debug)]
pub struct Stopwatch<'a, S: CycleSource> {
    source: &'a S,
    started: u64,
}

impl<'a, S: CycleSource> Stopwatch<'a, S> {
    /// Reads `source` and starts timing.
    pub fn start(source: &'a S) -> Self {
        Stopwatch { source, started: source.read_cycles() }
    }

    /// Reads `source` again and returns the overhead-corrected delta.
    pub fn stop(self, calibration: &Calibration) -> u64 {
        let ended = self.source.read_cycles();
        calibration.correct(ended.saturating_sub(self.started))
    }
}

/// The run-scoped facts every metric in one report shares, emitted once on
/// the `BEGIN` line so a consumer can tell two runs (or two tiers, or two
/// architectures) apart without guessing from the numbers.
#[derive(Debug, Clone, Copy)]
pub struct Environment<'a> {
    /// Test-matrix tier this run comes from (`T0` for QEMU, `T1`/`T2` for
    /// hardware) — the field that keeps a Tier 0 number from being quoted as
    /// hardware evidence.
    pub tier: &'a str,
    /// Target architecture (`x86_64`, later `aarch64`).
    pub arch: &'a str,
    /// Which [`CycleSource`] implementor produced the samples.
    pub cycle_source: &'a str,
    /// The calibrated per-sample read overhead already subtracted from every
    /// percentile below.
    pub overhead_cycles: u64,
    /// The established cycles-per-microsecond factor, or `None` when no
    /// trustworthy factor exists — emitted as `unknown`, never as a guess.
    pub cycles_per_us: Option<u32>,
}

/// One metric's identity plus its summary, as emitted on one `METRIC` line.
#[derive(Debug, Clone, Copy)]
pub struct Metric<'a> {
    /// The `goals/performance/catalogue.tsv` domain this metric is evidence
    /// for (`D04`, `D05`, `D07`, ...), so a reader never has to infer which
    /// budget column applies.
    pub domain: &'a str,
    /// Metric name, unique within one report.
    pub name: &'a str,
    /// Unmeasured warmup iterations run before sampling started.
    pub warmup: usize,
    /// The percentiles.
    pub summary: Summary,
}

/// Emits the versioned `TINYOS-MEAS/1` envelope to any [`Write`] sink — COM1
/// inside a fixture, a `String` in a host test.
///
/// The envelope is three line kinds, in order: one `BEGIN` carrying the
/// [`Environment`], one `METRIC` per [`Metric`], and one `END` whose
/// `metrics=` count is the number of `METRIC` lines actually written. That
/// count is what lets `xtask` distinguish a complete report from output
/// truncated by a guest that crashed or a UART that stalled — the failure
/// mode a bare list of lines cannot detect at all.
#[derive(Debug)]
pub struct Report<'w, W: Write> {
    sink: &'w mut W,
    emitted: usize,
}

impl<'w, W: Write> Report<'w, W> {
    /// Writes the `BEGIN` line.
    pub fn begin(sink: &'w mut W, environment: &Environment<'_>) -> Result<Self, fmt::Error> {
        let report = Report { sink, emitted: 0 };
        write!(
            report.sink,
            "{ENVELOPE} BEGIN tier={} arch={} cycle_source={} overhead_cycles={} cycles_per_us=",
            environment.tier,
            environment.arch,
            environment.cycle_source,
            environment.overhead_cycles
        )?;
        match environment.cycles_per_us {
            Some(factor) => writeln!(report.sink, "{factor}")?,
            None => writeln!(report.sink, "unknown")?,
        }
        Ok(report)
    }

    /// Writes one `METRIC` line.
    pub fn metric(&mut self, metric: &Metric<'_>) -> Result<(), fmt::Error> {
        let summary = &metric.summary;
        writeln!(
            self.sink,
            "{ENVELOPE} METRIC domain={} metric={} n={} dropped={} warmup={} min={} p50={} p99={} p99_9={} max={} unit={UNIT}",
            metric.domain,
            metric.name,
            summary.n,
            summary.dropped,
            metric.warmup,
            summary.min,
            summary.p50,
            summary.p99,
            summary.p99_9,
            summary.max
        )?;
        self.emitted += 1;
        Ok(())
    }

    /// Writes the `END` line and returns how many `METRIC` lines were
    /// emitted.
    pub fn end(self) -> Result<usize, fmt::Error> {
        writeln!(self.sink, "{ENVELOPE} END metrics={}", self.emitted)?;
        Ok(self.emitted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    /// Advances by a scripted sequence of deltas, cycling once exhausted, so
    /// a test can dictate exactly what a timed region "costs".
    struct ScriptedSource {
        now: Cell<u64>,
        deltas: &'static [u64],
        next: Cell<usize>,
    }

    impl ScriptedSource {
        fn new(deltas: &'static [u64]) -> Self {
            ScriptedSource { now: Cell::new(0), deltas, next: Cell::new(0) }
        }
    }

    impl CycleSource for ScriptedSource {
        fn read_cycles(&self) -> u64 {
            let value = self.now.get();
            let index = self.next.get();
            self.now.set(value + self.deltas[index % self.deltas.len()]);
            self.next.set(index + 1);
            value
        }
    }

    fn summary_of<const N: usize>(values: &[u64]) -> Summary {
        let mut samples: Samples<N> = Samples::new();
        for &value in values {
            samples.record(value);
        }
        samples.summarize().expect("values is non-empty")
    }

    // Clause 4: the catalogue's own budget columns, by integer nearest rank.
    #[test]
    fn percentiles_over_one_to_one_thousand_match_nearest_rank() {
        let values: Vec<u64> = (1..=1_000).collect();
        let summary = summary_of::<1_000>(&values);
        assert_eq!(summary.n, 1_000);
        assert_eq!(summary.min, 1);
        assert_eq!(summary.p50, 500);
        assert_eq!(summary.p99, 990);
        assert_eq!(summary.p99_9, 999);
        assert_eq!(summary.max, 1_000);
        assert_eq!(summary.dropped, 0);
    }

    #[test]
    fn a_single_sample_is_every_percentile() {
        let summary = summary_of::<4>(&[77]);
        assert_eq!(
            summary,
            Summary { n: 1, dropped: 0, min: 77, p50: 77, p99: 77, p99_9: 77, max: 77 }
        );
    }

    #[test]
    fn summarizing_is_order_independent() {
        let ascending: Vec<u64> = (1..=1_000).collect();
        let mut shuffled = ascending.clone();
        // Deterministic shuffle (no RNG dependency in a `no_std` crate's
        // test): reverse, then interleave halves.
        shuffled.reverse();
        let (front, back) = shuffled.split_at(500);
        let interleaved: Vec<u64> =
            front.iter().zip(back.iter()).flat_map(|(a, b)| [*a, *b]).collect();
        assert_eq!(summary_of::<1_000>(&ascending), summary_of::<1_000>(&interleaved));
    }

    #[test]
    fn percentiles_are_monotonically_ordered_for_every_input_shape() {
        for shape in [
            vec![5u64],
            vec![9, 1],
            vec![1, 1, 1, 1],
            (0..37).map(|i| (i * 7919) % 101).collect(),
            (0..999).map(|i| if i == 998 { 1_000_000 } else { 10 }).collect(),
        ] {
            let summary = summary_of::<1_024>(&shape);
            assert!(
                summary.min <= summary.p50
                    && summary.p50 <= summary.p99
                    && summary.p99 <= summary.p99_9
                    && summary.p99_9 <= summary.max,
                "monotonicity violated for {shape:?}: {summary:?}"
            );
        }
    }

    #[test]
    fn percentile_of_an_empty_slice_is_none() {
        assert_eq!(percentile(&[], 50, 100), None);
        assert_eq!(percentile(&[1, 2, 3], 50, 0), None);
    }

    // Clause 2: bounded capacity, explicit drop accounting, no overwrite.
    #[test]
    fn surplus_samples_are_counted_as_dropped_never_overwritten() {
        let mut samples: Samples<4> = Samples::new();
        for value in [10u64, 20, 30, 40] {
            assert!(samples.record(value), "capacity not yet reached");
        }
        for value in [1u64, 2, 3] {
            assert!(!samples.record(value), "buffer is full — must refuse, not overwrite");
        }
        assert_eq!(samples.len(), 4);
        assert_eq!(samples.capacity(), 4);
        assert_eq!(samples.dropped(), 3);
        let summary = samples.summarize().expect("four samples recorded");
        assert_eq!(summary.n, 4);
        assert_eq!(summary.dropped, 3);
        assert_eq!(summary.min, 10, "the refused samples never displaced a recorded one");
        assert_eq!(summary.max, 40);
    }

    #[test]
    fn an_empty_buffer_has_no_summary() {
        let mut samples: Samples<8> = Samples::new();
        assert!(samples.is_empty());
        assert_eq!(samples.summarize(), None);
    }

    #[test]
    fn clearing_resets_both_samples_and_drop_accounting_for_phase_reuse() {
        let mut samples: Samples<2> = Samples::new();
        for value in [1u64, 2, 3, 4] {
            samples.record(value);
        }
        assert_eq!(samples.dropped(), 2);
        samples.clear();
        assert!(samples.is_empty());
        assert_eq!(samples.dropped(), 0);
        assert!(samples.record(99));
        assert_eq!(samples.summarize().map(|s| (s.n, s.dropped, s.p50)), Some((1, 0, 99)));
    }

    // Clause 3: calibration is the minimum, and correction saturates.
    #[test]
    fn calibration_takes_the_minimum_observed_read_pair() {
        // Read deltas cycle 40, 12, 90: the minimum pair cost is 12.
        let source = ScriptedSource::new(&[40, 12, 90]);
        let calibration = Calibration::measure(&source, 60);
        assert_eq!(calibration.overhead_cycles(), 12);
    }

    #[test]
    fn calibration_over_zero_samples_subtracts_nothing() {
        let source = ScriptedSource::new(&[40]);
        assert_eq!(Calibration::measure(&source, 0).overhead_cycles(), 0);
    }

    #[test]
    fn overhead_correction_saturates_at_zero_instead_of_wrapping() {
        let calibration = Calibration::from_overhead_cycles(30);
        assert_eq!(calibration.correct(100), 70);
        assert_eq!(calibration.correct(30), 0);
        assert_eq!(calibration.correct(1), 0, "must not wrap into the far tail");
        assert_eq!(calibration.correct(0), 0);
    }

    #[test]
    fn a_stopwatch_reports_the_corrected_span_of_the_region_it_timed() {
        // Every read advances the source by 25; a start/stop pair therefore
        // spans 25 raw cycles, of which 10 is calibrated overhead.
        let source = ScriptedSource::new(&[25]);
        let calibration = Calibration::from_overhead_cycles(10);
        let watch = Stopwatch::start(&source);
        assert_eq!(watch.stop(&calibration), 15);
    }

    // Clause 1: the API is arch-neutral — this whole test module drives it
    // with a host test double and never mentions an architecture. The
    // shared `CycleSource` conformance suite is `hal::time::conformance`,
    // exercised here against that same double to prove the harness's own
    // source contract is the one the suite checks.
    #[test]
    fn the_harness_source_contract_is_the_shared_conformance_contract() {
        let source = ScriptedSource::new(&[3]);
        assert_eq!(hal::time::conformance::check(&source, 10), Ok(27));
    }

    // Clause 5: the versioned envelope, byte for byte.
    #[test]
    fn the_envelope_is_a_versioned_begin_metric_end_triple() {
        let mut sink = String::new();
        let environment = Environment {
            tier: "T0",
            arch: "x86_64",
            cycle_source: "rdtsc",
            overhead_cycles: 26,
            cycles_per_us: Some(1_000),
        };
        let mut report = Report::begin(&mut sink, &environment).expect("String sink cannot fail");
        report
            .metric(&Metric {
                domain: "D07",
                name: "pool_alloc_free_round_trip",
                warmup: 500,
                summary: Summary {
                    n: 10_000,
                    dropped: 0,
                    min: 40,
                    p50: 44,
                    p99: 60,
                    p99_9: 120,
                    max: 900,
                },
            })
            .expect("String sink cannot fail");
        assert_eq!(report.end().expect("String sink cannot fail"), 1);

        assert_eq!(
            sink,
            "TINYOS-MEAS/1 BEGIN tier=T0 arch=x86_64 cycle_source=rdtsc overhead_cycles=26 \
             cycles_per_us=1000\n\
             TINYOS-MEAS/1 METRIC domain=D07 metric=pool_alloc_free_round_trip n=10000 dropped=0 \
             warmup=500 min=40 p50=44 p99=60 p99_9=120 max=900 unit=cycles\n\
             TINYOS-MEAS/1 END metrics=1\n"
        );
    }

    #[test]
    fn an_unknown_timebase_is_emitted_as_unknown_never_as_a_guess() {
        let mut sink = String::new();
        let environment = Environment {
            tier: "T0",
            arch: "x86_64",
            cycle_source: "rdtsc",
            overhead_cycles: 26,
            cycles_per_us: None,
        };
        let report = Report::begin(&mut sink, &environment).expect("String sink cannot fail");
        assert_eq!(report.end().expect("String sink cannot fail"), 0);
        assert!(sink.contains("cycles_per_us=unknown"), "got {sink}");
        assert!(sink.contains("END metrics=0"), "an empty report still closes its envelope");
    }

    #[test]
    fn the_end_line_counts_exactly_the_metrics_emitted() {
        let mut sink = String::new();
        let environment = Environment {
            tier: "T0",
            arch: "x86_64",
            cycle_source: "test",
            overhead_cycles: 0,
            cycles_per_us: None,
        };
        let mut report = Report::begin(&mut sink, &environment).expect("String sink cannot fail");
        for index in 0..3u32 {
            let name = match index {
                0 => "a",
                1 => "b",
                _ => "c",
            };
            report
                .metric(&Metric {
                    domain: "D04",
                    name,
                    warmup: 0,
                    summary: Summary { n: 1, dropped: 0, min: 1, p50: 1, p99: 1, p99_9: 1, max: 1 },
                })
                .expect("String sink cannot fail");
        }
        assert_eq!(report.end().expect("String sink cannot fail"), 3);
        assert_eq!(sink.matches("METRIC").count(), 3);
        assert!(sink.ends_with("END metrics=3\n"));
    }
}
