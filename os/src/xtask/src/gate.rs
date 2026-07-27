//! Committed timing baselines and the regression comparison
//! (`STORY-P1-01-02`, re-based onto same-run ratios by `STORY-P1-01-04`).
//!
//! Separate from [`crate::timing`] by responsibility: that module turns a
//! fixture's serial output into structured evidence, this one decides whether
//! that evidence is a regression. The split matters because the second
//! question has a different failure mode from the first — a parser that fails
//! closed is useless behind a comparison that quietly passes when it has
//! nothing to compare against.
//!
//! # Why this gate compares ratios rather than cycle counts
//!
//! `STORY-P1-01-02` gated absolute cycle counts against committed baselines,
//! and five sessions of red CI showed what that verdict is worth. Between two
//! runs of **identical binaries** — the second commit changed one markdown
//! file — every gated metric moved together by 1.8–2.2x, and the metric with
//! the least headroom crossed its limit and reported `REGRESSED` about code
//! that had not changed (`LE-16` meeting `LE-18`; Handover 10 has the table).
//!
//! The finding that matters is that **the noise is global**. Nothing about
//! `D05/dispatch_select` is unstable; the shared runner's speed varies about
//! twofold and drags every measurement with it. So a quantity measured
//! *between two metrics in the same run* is far more stable than either
//! metric's absolute value, and a genuine regression in one operation moves
//! that quantity while a slow runner does not.
//!
//! This project had already solved the same problem once and this gate never
//! got the memo: `kernel::fixture_idt_apic_timer` gates on
//! `MAX_INTERVAL_RATIO`, *"a self-consistency bound rather than a fixed
//! microsecond figure, since QEMU's own APIC-timer-to-wall-clock relationship
//! under software emulation is not itself a stable absolute number this
//! fixture should depend on."* Verbatim applicable here.
//!
//! So the gated quantity is [`REFERENCE_METRIC`]-relative: for each statistic,
//! `metric / reference` in parts per million, formed **per run** and then
//! medianed. Per run and then medianed, not the other way round — the per-run
//! ratio is the quantity the runner's speed cancels out of, and medianing the
//! two absolutes first would re-admit exactly the noise this design removes.
//!
//! # What that costs, stated plainly
//!
//! **A uniform slowdown of everything, reference included, passes.** That is
//! the price of scale-invariance and it is deliberate. The reference is a
//! fixed integer computation touching no kernel subsystem, so no scheduler,
//! pool, address-space or fault change can move it; the realistic shape of the
//! hazard is a toolchain or codegen-flag change, and [`REFERENCE_TOLERANCE`]
//! is the wide structural band that watches for it. `LE-16` is not closed by
//! this Story — the gate's sensitivity is restated in units that mean
//! something, not eliminated.
//!
//! # What did not change
//!
//! - only `min` and `p50` are gated (the statistics `REPORT-2026-07-27-02`
//!   measured as stable; Tier 0 p99 run-to-run variation is 39–61%),
//! - over the **median of at least three runs** (never one),
//! - the tails are still printed, labelled as reported-and-not-gated, so a
//!   reader never mistakes an ungated number for a passing one,
//! - and **provenance is enforced, not decorative**: a baseline row carries
//!   its tier, architecture, profile and cycle source, and a comparison across
//!   any disagreement is refused outright rather than absorbed into a
//!   tolerance. A Tier 0 figure must never be able to masquerade as a hardware
//!   one, and a release run compared against a dev-profile baseline is not
//!   measuring the same code at all (`LE-13`).

use crate::timing::Envelope;
use std::collections::BTreeSet;
use std::fmt;

/// The baseline file's required header. Exact-match, like every other TSV in
/// this repository's assurance spine: a column silently added or reordered
/// would re-point every number in the file.
///
/// The two ratio columns are what `STORY-P1-01-04` added. A ten-column
/// baseline recorded before them is rejected by this exact match rather than
/// read with the ratio columns defaulted — a baseline that carries no ratios
/// cannot gate on them, and defaulting would gate on whichever number landed
/// in the new position.
pub const BASELINE_HEADER: &str = "domain\tmetric\ttier\tarch\tprofile\tcycle_source\truns\tmin_cycles\tp50_cycles\tmin_ratio_ppm\tp50_ratio_ppm\trecorded_on";

/// Field count implied by [`BASELINE_HEADER`].
const BASELINE_FIELDS: usize = 12;

/// The scale ratios are carried at. Integer parts-per-million rather than a
/// float: this workspace has no floating point in a gate path and would gain
/// nothing but a rounding argument by acquiring one.
pub const PPM: u64 = 1_000_000;

/// The metric every other metric is normalised against.
///
/// Measured by `kernel::fixture_measure::phase_reference_loop` — a fixed
/// integer computation that touches no scheduler, no pool, no context switch,
/// no fault path and no allocation, so nothing this project's kernel code can
/// change is able to move it. It runs through exactly the same `Stopwatch`,
/// `Calibration`, `Samples` and `summarize` path as every gated metric, so
/// any systematic error in the measurement machinery cancels out of the ratio
/// instead of being imported into it.
pub const REFERENCE_METRIC: &str = "REF/fixed_integer_loop";

/// The tolerance applied to **ratios**, in ppm.
///
/// **The derivation, including the part of it that was falsified.**
/// `TEST-P1-01-04-A` clause 4 committed, before any measurement was taken,
/// that the ratio spread must be at least **3x tighter** than the absolute
/// spread over runs deliberately spanning a 1.5x range in the reference. Six
/// simulated gate invocations at `--runs=3`, spanning a **2.02x** reference
/// swing between a quiet host and one loaded with 14 spinners, did **not**
/// meet that bound: the improvement is 1.41x–2.28x, not 3x
/// (`REPORT-2026-07-28-06` has the table).
///
/// What the same measurement did show is the number that matters. Against
/// **absolute** swings of 1.72x–4.18x on unchanged code, the **ratio** swings
/// are 1.22x–1.83x.
///
/// The constant is sized against the excursion the gate will actually meet:
/// a baseline recorded on a quiet host, compared against an observation taken
/// on a loaded one. Measured, that is **+62%**
/// (`D05/dispatch_select_highest_priority_ready`), then +55% (D07 denial),
/// +26%, +9%, and one metric that improves. 100% clears the worst of them by
/// 38 points.
///
/// **This is a looser number than `STORY-P1-01-02`'s 60%, and it is a better
/// gate.** That 60% applied to absolutes which were measured swinging up to
/// +318% on unchanged code, so it was not a 1.6x detector — it was a coin
/// toss that had been red on `main` for five sessions. This one contains
/// every excursion the calibration produced, so what it reports is about the
/// code. Stated plainly: **at Tier 0 this gate catches ratio regressions of
/// roughly 2x or worse.** `LE-16` is restated in units that mean something,
/// not closed, and no Tier 0 work can improve it — that still needs `LE-09`'s
/// board.
///
/// Handover 05's rule was followed rather than worked around: the constant
/// clears *measured* noise with margin, and was not chosen by trying values
/// until the gate went green.
///
/// The floor is denominated in **ppm** and exists for the same reason the
/// cycle floor did — to stop a metric small enough for quantisation to
/// dominate from being gated more tightly than the measurement's own
/// granularity. It is deliberately modest: at 20,000 ppm it is ~7 cycles of
/// headroom against a ~650-cycle reference, well under the 60% relative term
/// for every metric that survived gating, so it protects small ratios without
/// swamping them.
pub const TIER0_RATIO_TOLERANCE: Tolerance =
    Tolerance { relative_percent: 100, absolute_floor: 20_000 };

/// Metrics this fixture measures, reports, and deliberately does **not** gate
/// at Tier 0 — each with the measured reason it cannot carry a verdict.
///
/// This list is committed in code rather than as a column in the baseline file
/// on purpose. A metric moving from gated to ungated is exactly the edit
/// somebody makes at 2am to turn CI green, and it should require changing a
/// source file with a stated reason next to it and a test that pins the set —
/// not a one-character change to a TSV that nobody diffs. `LE-07`'s lesson,
/// applied to the shape of the gate rather than to whether it runs.
///
/// A metric here is still parsed, still baselined, still printed, and still
/// subject to every fail-closed check: it must be measured, it must be
/// baselined, and its ratio is shown on every run. What it does not do is
/// produce a verdict, because the measurement underneath it cannot support
/// one.
pub const UNGATED_AT_TIER0: &[(&str, &str)] = &[(
    "D07/pool_u64x64_alloc_free_round_trip",
    "medians to 0 cycles: the operation costs less than the calibrated rdtsc \
     overhead subtracted from it, so its value is quantisation rather than evidence",
)];

/// Whether `key` is measured and reported but deliberately ungated.
pub fn is_ungated(key: &str) -> bool {
    UNGATED_AT_TIER0.iter().any(|(metric, _)| *metric == key)
}

/// The tolerance applied to [`REFERENCE_METRIC`]'s **absolute** cycles.
///
/// Deliberately far wider than any regression tolerance, because it is not a
/// regression detector. It is a tripwire for *the reference having stopped
/// being the reference* — the loop edited, the optimiser deleting it, a
/// toolchain change moving it — and every other verdict on the run now depends
/// on that not having happened.
///
/// 300% (a 4x band) against the **2.2x** worst run-to-run swing this project
/// has recorded between identical binaries. Anything tighter would reintroduce
/// `LE-18` on the one metric the whole gate is now anchored to, which would be
/// a more elaborate way of having the same bug.
pub const REFERENCE_TOLERANCE: Tolerance = Tolerance { relative_percent: 300, absolute_floor: 24 };

/// The two tolerances, travelling together so a caller cannot supply one and
/// forget the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatePolicy {
    /// Applied to every non-reference metric's ratio, in ppm.
    pub ratio: Tolerance,
    /// Applied to [`REFERENCE_METRIC`]'s absolute cycles.
    pub reference: Tolerance,
}

/// The committed Tier 0 policy.
pub const TIER0_POLICY: GatePolicy =
    GatePolicy { ratio: TIER0_RATIO_TOLERANCE, reference: REFERENCE_TOLERANCE };

// The two bounds `TEST-P1-01-04-A` fixes on these constants, checked by the
// compiler rather than by a test — a constant cannot be edited past them even
// in a build where the tests are not run.
//
// Clause 5: the reference's structural band clears the 2.2x worst run-to-run
// swing this project has recorded between identical binaries, with margin.
const _: () = assert!(REFERENCE_TOLERANCE.limit(1_000) >= 3_000);
// Clause 4: the ratio tolerance clears the worst excursion the calibration
// measured from a quiet-host baseline to a loaded-host observation (+62%, on
// `D05/dispatch_select_highest_priority_ready`) with real margin. A constant
// trimmed to just above the measured worst case is a constant chosen to make
// today's numbers pass, which is the thing Handover 05 warned about.
const _: () = assert!(TIER0_RATIO_TOLERANCE.relative_percent >= 90);

/// The fewest runs the gate will conclude from. Three is the smallest count
/// that has a median at all without averaging two observations.
pub const MINIMUM_RUNS: usize = 3;

/// One committed baseline row: what a metric measured, and everything a
/// reader needs to know what produced that number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineRow {
    /// Performance-catalogue domain (`D04`, `D05`, `D07`, ...), or `REF` for
    /// [`REFERENCE_METRIC`], which is deliberately not a catalogue domain: it
    /// is measurement scaffolding, not evidence about any guardrail.
    pub domain: String,
    /// Metric name, unique within the file.
    pub metric: String,
    /// Test-matrix tier (`T0` for QEMU) — the field that keeps an emulated
    /// number from ever being quoted as hardware evidence.
    pub tier: String,
    /// Architecture the number came from.
    pub arch: String,
    /// Cargo profile (`release`) — see `LE-13`.
    pub profile: String,
    /// Cycle-source implementor that produced it.
    pub cycle_source: String,
    /// How many runs the medians below were taken over.
    pub runs: u64,
    /// Median-of-runs minimum, in cycles. **Reported, not gated** (except on
    /// the reference row) — kept so a reader can see what the machine was
    /// doing, and so a future hardware tier has the absolute history.
    pub min_cycles: u64,
    /// Median-of-runs p50, in cycles. Reported, not gated.
    pub p50_cycles: u64,
    /// Median across runs of the per-run `min / reference_min`, in ppm. **This
    /// is what the gate compares.**
    pub min_ratio_ppm: u64,
    /// Median across runs of the per-run `p50 / reference_p50`, in ppm.
    pub p50_ratio_ppm: u64,
    /// The date this row was recorded.
    pub recorded_on: String,
}

impl BaselineRow {
    /// `domain/metric`, the key shared with [`crate::timing::MetricRecord`].
    pub fn key(&self) -> String {
        format!("{}/{}", self.domain, self.metric)
    }
}

/// A parsed baseline file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Baseline {
    /// Rows, in file order.
    pub rows: Vec<BaselineRow>,
}

impl Baseline {
    /// Looks a row up by `domain/metric`.
    pub fn row(&self, key: &str) -> Option<&BaselineRow> {
        self.rows.iter().find(|row| row.key() == key)
    }
}

/// The regression tolerance model: an observation passes while it stays at or
/// under `baseline + max(absolute_floor, baseline * relative_percent / 100)`.
///
/// Unit-agnostic on purpose — the same arithmetic gates ppm ratios and, for
/// [`REFERENCE_METRIC`] alone, absolute cycles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tolerance {
    /// Relative headroom, as a percentage of the baseline.
    pub relative_percent: u64,
    /// Absolute headroom floor, in whatever unit the baseline is denominated
    /// in — what keeps a metric small enough for quantisation to dominate from
    /// being gated more tightly than the measurement's own granularity.
    pub absolute_floor: u64,
}

impl Tolerance {
    /// The largest observed value that still passes.
    pub const fn limit(&self, baseline: u64) -> u64 {
        let relative = baseline * self.relative_percent / 100;
        let headroom = if relative > self.absolute_floor { relative } else { self.absolute_floor };
        baseline + headroom
    }
}

/// Which quantity a [`Comparison`] actually gated on.
///
/// Present so the gate's output can never leave a reader guessing whether the
/// number in front of them was the one that decided the verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantity {
    /// A same-run ratio against [`REFERENCE_METRIC`], in ppm. Every metric
    /// except the reference itself.
    RatioPpm,
    /// Absolute cycles. [`REFERENCE_METRIC`] only — see
    /// [`REFERENCE_TOLERANCE`] for why, and for why the band is so wide.
    Cycles,
}

/// What the gate concluded about one statistic of one metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Within tolerance in both directions.
    Pass,
    /// Slower than the baseline by more than the tolerance — the gate fails.
    Regressed,
    /// Faster than the baseline by more than the tolerance. **Not** a
    /// failure, but reported: a metric that suddenly got much faster usually
    /// means the workload stopped happening, not that the code improved.
    ImprovedBeyondTolerance,
    /// Measured, baselined and printed, but carrying no verdict — the metric
    /// is in [`UNGATED_AT_TIER0`] because the measurement underneath it cannot
    /// support one. Never a failure, and never counted as a pass either.
    ReportedNotGated,
}

/// One statistic of one metric, compared against its baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    /// `domain/metric`.
    pub key: String,
    /// Which statistic — `min` or `p50`. The tails are deliberately absent.
    pub statistic: &'static str,
    /// Which quantity the three numbers below are denominated in.
    pub quantity: Quantity,
    /// The committed baseline value, in `quantity`'s unit.
    pub baseline: u64,
    /// The median across the runs just measured, in `quantity`'s unit.
    pub observed: u64,
    /// The largest value that would have passed, in `quantity`'s unit.
    pub limit: u64,
    /// The conclusion.
    pub verdict: Verdict,
    /// The observed median in **cycles**, always — carried alongside a ratio
    /// verdict so the gate can report the absolute it did not gate on, which
    /// is what anyone diagnosing a failure actually needs.
    pub observed_cycles: u64,
    /// The baseline in **cycles**, always.
    pub baseline_cycles: u64,
}

/// Every way the gate refuses to conclude "pass".
///
/// Each variant is a *refusal*, not a warning: there is no path through this
/// module that turns an unanswerable question into a passing gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateError {
    /// The baseline file's header was absent or not exactly the expected one.
    BadHeader {
        /// The header line as found.
        found: String,
    },
    /// A row did not carry exactly the header's field count.
    FieldCount {
        /// 1-based line number.
        line: usize,
        /// How many fields were found.
        found: usize,
    },
    /// A numeric column was not a number.
    NotANumber {
        /// 1-based line number.
        line: usize,
        /// The offending column.
        column: &'static str,
        /// The offending value.
        value: String,
    },
    /// A row claimed a minimum larger than its own median.
    NonMonotonic {
        /// The metric key.
        key: String,
    },
    /// A row claimed to have been recorded over zero runs.
    ZeroRuns {
        /// The metric key.
        key: String,
    },
    /// A row carried a zero ratio — a baseline no observation can be compared
    /// against, since the tolerance is relative to it.
    ZeroRatio {
        /// The metric key.
        key: String,
    },
    /// A row carried an empty field, so part of its provenance is missing.
    EmptyField {
        /// 1-based line number.
        line: usize,
        /// The offending column.
        column: &'static str,
    },
    /// Two rows shared one `domain/metric` key.
    DuplicateMetric {
        /// The repeated key.
        key: String,
    },
    /// The file parsed but carried no rows — an empty gate is not a gate.
    NoRows,
    /// The baseline carried no [`REFERENCE_METRIC`] row, so nothing in it can
    /// be normalised.
    ReferenceNotBaselined,
    /// The [`REFERENCE_METRIC`] row's own ratio columns were not unity. It is
    /// its own denominator; anything else means the file was hand-edited.
    ReferenceRatioNotUnity {
        /// Which statistic disagreed.
        statistic: &'static str,
        /// The value found.
        found: u64,
    },
    /// A measured run carried no [`REFERENCE_METRIC`] — every ratio in the run
    /// divides by it.
    ReferenceNotMeasured {
        /// 1-based run index.
        run: usize,
    },
    /// A measured run's [`REFERENCE_METRIC`] was zero. Not a very fast
    /// reference; a broken one.
    ReferenceZero {
        /// 1-based run index.
        run: usize,
    },
    /// The measured runs' provenance disagrees with the baseline's.
    ProvenanceMismatch {
        /// Which provenance field disagreed.
        field: &'static str,
        /// What the baseline says.
        baseline: String,
        /// What the runs say.
        observed: String,
    },
    /// A baselined metric was not measured at all — the shape a silently
    /// deleted measurement takes.
    MetricNotMeasured {
        /// The missing key.
        key: String,
    },
    /// A measured metric has no baseline, so nothing can be concluded about
    /// it. An error rather than a skip: an ungated metric that looks gated is
    /// worse than no gate.
    MetricNotBaselined {
        /// The unbaselined key.
        key: String,
    },
    /// Fewer runs than [`MINIMUM_RUNS`].
    TooFewRuns {
        /// Runs supplied.
        found: usize,
        /// Runs required.
        required: usize,
    },
}

impl fmt::Display for GateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GateError::BadHeader { found } => write!(
                formatter,
                "baseline header is `{found}`, expected exactly `{BASELINE_HEADER}`"
            ),
            GateError::FieldCount { line, found } => write!(
                formatter,
                "baseline line {line} has {found} fields, expected {BASELINE_FIELDS}"
            ),
            GateError::NotANumber { line, column, value } => {
                write!(formatter, "baseline line {line}: `{column}={value}` is not a number")
            }
            GateError::NonMonotonic { key } => {
                write!(formatter, "baseline `{key}` claims a min larger than its own p50")
            }
            GateError::ZeroRuns { key } => {
                write!(formatter, "baseline `{key}` claims to be recorded over 0 runs")
            }
            GateError::ZeroRatio { key } => write!(
                formatter,
                "baseline `{key}` carries a zero ratio; nothing can be compared against it"
            ),
            GateError::EmptyField { line, column } => {
                write!(formatter, "baseline line {line}: `{column}` is empty")
            }
            GateError::DuplicateMetric { key } => {
                write!(formatter, "baseline carries `{key}` twice")
            }
            GateError::NoRows => {
                write!(formatter, "baseline file carries no rows — there is nothing to gate on")
            }
            GateError::ReferenceNotBaselined => write!(
                formatter,
                "baseline carries no `{REFERENCE_METRIC}` row, so no metric in it can be normalised; \
                 this gate compares same-run ratios and the reference is the denominator"
            ),
            GateError::ReferenceRatioNotUnity { statistic, found } => write!(
                formatter,
                "baseline `{REFERENCE_METRIC}` reports {statistic}_ratio_ppm={found}; it is its own \
                 denominator, so the only correct value is {PPM}"
            ),
            GateError::ReferenceNotMeasured { run } => write!(
                formatter,
                "run {run} measured no `{REFERENCE_METRIC}`; every ratio in the run divides by it, \
                 so there is nothing to gate on rather than something to gate loosely"
            ),
            GateError::ReferenceZero { run } => write!(
                formatter,
                "run {run} measured `{REFERENCE_METRIC}` as 0 cycles; that is a broken reference, \
                 not a fast one"
            ),
            GateError::ProvenanceMismatch { field, baseline, observed } => write!(
                formatter,
                "baseline was recorded with {field}={baseline} but these runs report {field}={observed}; \
                 these are not comparable measurements"
            ),
            GateError::MetricNotMeasured { key } => write!(
                formatter,
                "baseline expects `{key}` but no run measured it — a metric that vanished is a regression in evidence"
            ),
            GateError::MetricNotBaselined { key } => write!(
                formatter,
                "`{key}` was measured but has no baseline; commit one (`--update-baseline`) rather than leaving it ungated"
            ),
            GateError::TooFewRuns { found, required } => write!(
                formatter,
                "the gate needs at least {required} runs to take a median, got {found}"
            ),
        }
    }
}

/// `value / reference` in ppm, or `None` when the reference measured nothing.
///
/// `None` rather than a saturating value on purpose: a zero denominator is an
/// unanswerable question, and every caller turns it into a refusal.
pub fn ratio_ppm(value: u64, reference: u64) -> Option<u64> {
    if reference == 0 {
        return None;
    }
    Some(value * PPM / reference)
}

/// Parses a committed baseline file, failing closed on every malformed shape.
pub fn parse_baseline(text: &str) -> Result<Baseline, GateError> {
    let mut lines = text.lines();
    let header = lines.next().unwrap_or_default().trim_end_matches('\r');
    if header != BASELINE_HEADER {
        return Err(GateError::BadHeader { found: header.to_string() });
    }

    let mut rows: Vec<BaselineRow> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (zero_based, raw_line) in lines.enumerate() {
        let line_number = zero_based + 2;
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != BASELINE_FIELDS {
            return Err(GateError::FieldCount { line: line_number, found: fields.len() });
        }
        const COLUMNS: [&str; BASELINE_FIELDS] = [
            "domain",
            "metric",
            "tier",
            "arch",
            "profile",
            "cycle_source",
            "runs",
            "min_cycles",
            "p50_cycles",
            "min_ratio_ppm",
            "p50_ratio_ppm",
            "recorded_on",
        ];
        for (index, value) in fields.iter().enumerate() {
            if value.is_empty() {
                return Err(GateError::EmptyField { line: line_number, column: COLUMNS[index] });
            }
        }
        let number = |index: usize, column: &'static str| -> Result<u64, GateError> {
            fields[index].parse::<u64>().map_err(|_| GateError::NotANumber {
                line: line_number,
                column,
                value: fields[index].to_string(),
            })
        };
        let row = BaselineRow {
            domain: fields[0].to_string(),
            metric: fields[1].to_string(),
            tier: fields[2].to_string(),
            arch: fields[3].to_string(),
            profile: fields[4].to_string(),
            cycle_source: fields[5].to_string(),
            runs: number(6, "runs")?,
            min_cycles: number(7, "min_cycles")?,
            p50_cycles: number(8, "p50_cycles")?,
            min_ratio_ppm: number(9, "min_ratio_ppm")?,
            p50_ratio_ppm: number(10, "p50_ratio_ppm")?,
            recorded_on: fields[11].to_string(),
        };
        if row.runs == 0 {
            return Err(GateError::ZeroRuns { key: row.key() });
        }
        if row.min_cycles > row.p50_cycles {
            return Err(GateError::NonMonotonic { key: row.key() });
        }
        if row.min_ratio_ppm == 0 || row.p50_ratio_ppm == 0 {
            return Err(GateError::ZeroRatio { key: row.key() });
        }
        if row.key() == REFERENCE_METRIC {
            for (statistic, found) in [("min", row.min_ratio_ppm), ("p50", row.p50_ratio_ppm)] {
                if found != PPM {
                    return Err(GateError::ReferenceRatioNotUnity { statistic, found });
                }
            }
        }
        if !seen.insert(row.key()) {
            return Err(GateError::DuplicateMetric { key: row.key() });
        }
        rows.push(row);
    }

    if rows.is_empty() {
        return Err(GateError::NoRows);
    }
    if !seen.contains(REFERENCE_METRIC) {
        return Err(GateError::ReferenceNotBaselined);
    }
    Ok(Baseline { rows })
}

/// The lower-middle element of `values`.
///
/// Deliberately not a mean, and deliberately not an interpolated median on an
/// even count: the compared figure stays a value some run actually observed,
/// so a failing gate can always be traced to a real capture.
pub fn median(values: &[u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    Some(sorted[(sorted.len() - 1) / 2])
}

/// The reference metric's `(min, p50)` in each run, in run order.
///
/// Refuses rather than substitutes: a run without a reference, or with a zero
/// one, makes every ratio in that run undefined.
fn reference_per_run(runs: &[Envelope]) -> Result<Vec<(u64, u64)>, GateError> {
    let mut values = Vec::with_capacity(runs.len());
    for (index, run) in runs.iter().enumerate() {
        let record = run
            .metric(REFERENCE_METRIC)
            .ok_or(GateError::ReferenceNotMeasured { run: index + 1 })?;
        if record.min == 0 || record.p50 == 0 {
            return Err(GateError::ReferenceZero { run: index + 1 });
        }
        values.push((record.min, record.p50));
    }
    Ok(values)
}

/// Compares parsed runs against a baseline, one [`Comparison`] per gated
/// statistic per metric.
///
/// Every disagreement upstream of the arithmetic — too few runs, provenance
/// mismatch, a metric on one side and not the other, a missing reference — is
/// an error rather than a partial answer.
pub fn check_against_baseline(
    baseline: &Baseline,
    runs: &[Envelope],
    profile: &str,
    policy: GatePolicy,
) -> Result<Vec<Comparison>, GateError> {
    if runs.len() < MINIMUM_RUNS {
        return Err(GateError::TooFewRuns { found: runs.len(), required: MINIMUM_RUNS });
    }

    // Provenance first: comparing incomparable things is a category error, and
    // finding out after the arithmetic would mean printing numbers that never
    // meant anything.
    for row in &baseline.rows {
        for run in runs {
            check_provenance("tier", &row.tier, &run.tier)?;
            check_provenance("arch", &row.arch, &run.arch)?;
            check_provenance("cycle_source", &row.cycle_source, &run.cycle_source)?;
        }
        check_provenance("profile", &row.profile, profile)?;
    }

    // The reference before the metric sets: a run that did not measure it has
    // no gated evidence at all, and reporting that as "one baselined metric is
    // missing" would understate it into something a reader might shrug at.
    let reference = reference_per_run(runs)?;

    let measured: BTreeSet<String> =
        runs.iter().flat_map(|run| run.metrics.iter().map(|record| record.key())).collect();
    for row in &baseline.rows {
        if !measured.contains(&row.key()) {
            return Err(GateError::MetricNotMeasured { key: row.key() });
        }
    }
    for key in &measured {
        if baseline.row(key).is_none() {
            return Err(GateError::MetricNotBaselined { key: key.clone() });
        }
    }

    let mut comparisons = Vec::with_capacity(baseline.rows.len() * 2);
    for row in &baseline.rows {
        let key = row.key();
        let is_reference = key == REFERENCE_METRIC;
        let mut mins = Vec::with_capacity(runs.len());
        let mut p50s = Vec::with_capacity(runs.len());
        let mut min_ratios = Vec::with_capacity(runs.len());
        let mut p50_ratios = Vec::with_capacity(runs.len());
        for (index, run) in runs.iter().enumerate() {
            let record = run
                .metric(&key)
                .ok_or_else(|| GateError::MetricNotMeasured { key: key.clone() })?;
            mins.push(record.min);
            p50s.push(record.p50);
            // Per run, then medianed — never a ratio of two medians. The
            // per-run ratio is the quantity the runner's speed cancels out of.
            let (reference_min, reference_p50) = reference[index];
            min_ratios.push(
                ratio_ppm(record.min, reference_min)
                    .ok_or(GateError::ReferenceZero { run: index + 1 })?,
            );
            p50_ratios.push(
                ratio_ppm(record.p50, reference_p50)
                    .ok_or(GateError::ReferenceZero { run: index + 1 })?,
            );
        }
        for (statistic, baseline_cycles, cycles, baseline_ratio, ratios) in [
            ("min", row.min_cycles, mins, row.min_ratio_ppm, min_ratios),
            ("p50", row.p50_cycles, p50s, row.p50_ratio_ppm, p50_ratios),
        ] {
            let observed_cycles =
                median(&cycles).ok_or_else(|| GateError::MetricNotMeasured { key: key.clone() })?;
            let observed_ratio =
                median(&ratios).ok_or_else(|| GateError::MetricNotMeasured { key: key.clone() })?;
            // The reference is gated on its absolute value against a
            // deliberately wide structural band; gating it against its own
            // ratio would compare 1,000,000 with 1,000,000 forever.
            let (quantity, tolerance, baseline_value, observed) = if is_reference {
                (Quantity::Cycles, policy.reference, baseline_cycles, observed_cycles)
            } else {
                (Quantity::RatioPpm, policy.ratio, baseline_ratio, observed_ratio)
            };
            let limit = tolerance.limit(baseline_value);
            let verdict = if is_ungated(&key) {
                // Everything upstream still applied — it had to be measured
                // and it had to be baselined. Only the conclusion is withheld.
                Verdict::ReportedNotGated
            } else if observed > limit {
                Verdict::Regressed
            } else if baseline_value > tolerance.limit(observed) {
                Verdict::ImprovedBeyondTolerance
            } else {
                Verdict::Pass
            };
            comparisons.push(Comparison {
                key: key.clone(),
                statistic,
                quantity,
                baseline: baseline_value,
                observed,
                limit,
                verdict,
                observed_cycles,
                baseline_cycles,
            });
        }
    }
    Ok(comparisons)
}

fn check_provenance(field: &'static str, baseline: &str, observed: &str) -> Result<(), GateError> {
    if baseline != observed {
        return Err(GateError::ProvenanceMismatch {
            field,
            baseline: baseline.to_string(),
            observed: observed.to_string(),
        });
    }
    Ok(())
}

/// Renders a baseline file from measured runs.
///
/// What `--update-baseline` writes is exactly what [`parse_baseline`] accepts —
/// a round trip the tests pin, because a generator that can emit a file its own
/// parser rejects turns a routine baseline refresh into a broken gate.
///
/// The recorded ratio is the **median of the per-run ratios**, matching what
/// [`check_against_baseline`] compares. Recording it any other way would mean a
/// baseline that does not pass against the very runs it was rendered from,
/// which the tests also pin.
pub fn render_baseline(
    runs: &[Envelope],
    profile: &str,
    recorded_on: &str,
) -> Result<String, GateError> {
    if runs.len() < MINIMUM_RUNS {
        return Err(GateError::TooFewRuns { found: runs.len(), required: MINIMUM_RUNS });
    }
    let reference = reference_per_run(runs)?;
    let first = &runs[0];
    let mut text = String::from(BASELINE_HEADER);
    text.push('\n');
    for record in &first.metrics {
        let key = record.key();
        let mut mins = Vec::with_capacity(runs.len());
        let mut p50s = Vec::with_capacity(runs.len());
        let mut min_ratios = Vec::with_capacity(runs.len());
        let mut p50_ratios = Vec::with_capacity(runs.len());
        for (index, run) in runs.iter().enumerate() {
            let found = run
                .metric(&key)
                .ok_or_else(|| GateError::MetricNotMeasured { key: key.clone() })?;
            mins.push(found.min);
            p50s.push(found.p50);
            let (reference_min, reference_p50) = reference[index];
            min_ratios.push(
                ratio_ppm(found.min, reference_min)
                    .ok_or(GateError::ReferenceZero { run: index + 1 })?,
            );
            p50_ratios.push(
                ratio_ppm(found.p50, reference_p50)
                    .ok_or(GateError::ReferenceZero { run: index + 1 })?,
            );
        }
        let min = median(&mins).ok_or_else(|| GateError::MetricNotMeasured { key: key.clone() })?;
        let p50 = median(&p50s).ok_or_else(|| GateError::MetricNotMeasured { key: key.clone() })?;
        let min_ratio =
            median(&min_ratios).ok_or_else(|| GateError::MetricNotMeasured { key: key.clone() })?;
        let p50_ratio =
            median(&p50_ratios).ok_or_else(|| GateError::MetricNotMeasured { key: key.clone() })?;
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            record.domain,
            record.metric,
            first.tier,
            first.arch,
            profile,
            first.cycle_source,
            runs.len(),
            min,
            p50,
            min_ratio,
            p50_ratio,
            recorded_on
        ));
    }
    Ok(text)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::timing::parse_stream;

    const RECORDED: &str = "2026-07-27";

    fn header_and(rows: &[&str]) -> String {
        let mut text = String::from(BASELINE_HEADER);
        text.push('\n');
        for row in rows {
            text.push_str(row);
            text.push('\n');
        }
        text
    }

    /// The reference row every well-formed baseline in these tests carries.
    /// Its own ratio columns are unity by construction (clause 2).
    const REFERENCE_ROW: &str =
        "REF\tfixed_integer_loop\tT0\tx86_64\trelease\trdtsc\t3\t100\t120\t1000000\t1000000\t2026-07-27";

    /// 236/100 = 2.36 and 246/120 = 2.05, against `REFERENCE_ROW`.
    const GOOD_ROW: &str =
        "D04\tcontext_switch\tT0\tx86_64\trelease\trdtsc\t3\t236\t246\t2360000\t2050000\t2026-07-27";

    fn envelope_with_reference(
        min: u64,
        p50: u64,
        reference_min: u64,
        reference_p50: u64,
    ) -> Envelope {
        let text = format!(
            "TINYOS-MEAS/1 BEGIN tier=T0 arch=x86_64 cycle_source=rdtsc overhead_cycles=36 cycles_per_us=2307\n\
             TINYOS-MEAS/1 METRIC domain=REF metric=fixed_integer_loop n=1000 dropped=0 warmup=100 min={reference_min} p50={reference_p50} p99={reference_tail} p99_9={reference_tail} max={reference_tail} unit=cycles\n\
             TINYOS-MEAS/1 METRIC domain=D04 metric=context_switch n=1000 dropped=0 warmup=100 min={min} p50={p50} p99={tail} p99_9={tail} max={tail} unit=cycles\n\
             TINYOS-MEAS/1 END metrics=2\n",
            tail = p50 + 100,
            reference_tail = reference_p50 + 100,
        );
        parse_stream(&text).expect("fixture stream is well formed")
    }

    /// A run whose reference sits at the same 100/120 the baseline rows above
    /// record, so a metric's ratio is a direct function of its own numbers.
    fn envelope(min: u64, p50: u64) -> Envelope {
        envelope_with_reference(min, p50, 100, 120)
    }

    fn runs(values: &[(u64, u64)]) -> Vec<Envelope> {
        values.iter().map(|(min, p50)| envelope(*min, *p50)).collect()
    }

    /// Runs stated as `(min, p50, reference_min, reference_p50)`.
    fn runs_with_reference(values: &[(u64, u64, u64, u64)]) -> Vec<Envelope> {
        values
            .iter()
            .map(|(min, p50, reference_min, reference_p50)| {
                envelope_with_reference(*min, *p50, *reference_min, *reference_p50)
            })
            .collect()
    }

    // ---------------------------------------------------------------------
    // TEST-P1-01-04-A — the ratio model.
    // ---------------------------------------------------------------------

    // Clause 2: the gated quantity is a same-run ratio carried in ppm.
    #[test]
    fn a_ratio_is_carried_in_parts_per_million() {
        assert_eq!(ratio_ppm(236, 100), Some(2_360_000));
        assert_eq!(ratio_ppm(100, 100), Some(PPM));
        assert_eq!(ratio_ppm(1, 1000), Some(1_000));
        // Dividing by a reference that measured nothing is not a small ratio,
        // it is an unanswerable question.
        assert_eq!(ratio_ppm(236, 0), None);
    }

    // Clause 3: the design's central claim, stated as arithmetic. A uniformly
    // slower runner is exactly a common factor, and must change no verdict.
    #[test]
    fn scaling_every_metric_by_a_common_factor_changes_no_ratio_verdict() {
        let baseline =
            parse_baseline(&header_and(&[REFERENCE_ROW, GOOD_ROW])).expect("well formed");
        let unscaled = [(236, 246, 100, 120), (240, 250, 102, 122), (244, 252, 98, 118)];
        let at_one = check_against_baseline(
            &baseline,
            &runs_with_reference(&unscaled),
            "release",
            TIER0_POLICY,
        )
        .expect("comparable");

        for factor in [2u64, 7, 23] {
            let scaled: Vec<(u64, u64, u64, u64)> = unscaled
                .iter()
                .map(|(a, b, c, d)| (a * factor, b * factor, c * factor, d * factor))
                .collect();
            let at_factor = check_against_baseline(
                &baseline,
                &runs_with_reference(&scaled),
                "release",
                TIER0_POLICY,
            )
            .expect("comparable");
            let ratios_at_one: Vec<_> =
                at_one.iter().filter(|c| c.quantity == Quantity::RatioPpm).collect();
            let ratios_at_factor: Vec<_> =
                at_factor.iter().filter(|c| c.quantity == Quantity::RatioPpm).collect();
            assert_eq!(ratios_at_one.len(), ratios_at_factor.len());
            for (one, scaled) in ratios_at_one.iter().zip(ratios_at_factor.iter()) {
                assert_eq!(one.key, scaled.key);
                assert_eq!(one.statistic, scaled.statistic);
                assert_eq!(
                    one.observed, scaled.observed,
                    "a {factor}x slower runner moved the gated ratio for {}/{}",
                    one.key, one.statistic
                );
                assert_eq!(one.limit, scaled.limit);
                assert_eq!(one.verdict, scaled.verdict);
            }
        }
    }

    // Clause 2: per-run ratios, then a median — not a ratio of two medians.
    // The two disagree here, which is the point: the per-run ratio is the
    // quantity the noise cancels from.
    #[test]
    fn the_ratio_is_medianed_per_run_rather_than_computed_from_medians() {
        let row = "D04\tcontext_switch\tT0\tx86_64\trelease\trdtsc\t3\t100\t100\t1000000\t1000000\t2026-07-27";
        let reference =
            "REF\tfixed_integer_loop\tT0\tx86_64\trelease\trdtsc\t3\t100\t100\t1000000\t1000000\t2026-07-27";
        let baseline = parse_baseline(&header_and(&[reference, row])).expect("well formed");
        // Per-run p50 ratios are 1.0, 2.0 and 0.5 — median 1.0. The median of
        // the metric (200) over the median of the reference (100) would be
        // 2.0, a number no run observed.
        let comparisons = check_against_baseline(
            &baseline,
            &runs_with_reference(&[
                (100, 100, 100, 100),
                (200, 200, 100, 100),
                (300, 300, 600, 600),
            ]),
            "release",
            TIER0_POLICY,
        )
        .expect("comparable");
        let p50 = comparisons
            .iter()
            .find(|c| c.key == "D04/context_switch" && c.statistic == "p50")
            .expect("compared");
        assert_eq!(p50.quantity, Quantity::RatioPpm);
        assert_eq!(p50.observed, PPM);
    }

    // Clause 2/5: the reference is gated on absolutes, never on its own ratio.
    #[test]
    fn the_reference_is_gated_on_absolutes_and_never_on_its_own_ratio() {
        let baseline =
            parse_baseline(&header_and(&[REFERENCE_ROW, GOOD_ROW])).expect("well formed");
        let comparisons =
            check_against_baseline(&baseline, &runs(&[(236, 246); 3]), "release", TIER0_POLICY)
                .expect("comparable");
        let reference: Vec<_> = comparisons.iter().filter(|c| c.key == REFERENCE_METRIC).collect();
        assert_eq!(reference.len(), 2, "the reference is compared on min and p50");
        assert!(
            reference.iter().all(|c| c.quantity == Quantity::Cycles),
            "gating the reference against its own ratio would be a tautology"
        );
        assert!(
            comparisons
                .iter()
                .filter(|c| c.key != REFERENCE_METRIC)
                .all(|c| c.quantity == Quantity::RatioPpm),
            "every other metric is gated on its ratio"
        );
    }

    // Clause 5: the reference's band is a structural tripwire, wide enough
    // that the 2.2x swing this project has recorded cannot trip it.
    #[test]
    fn the_reference_band_clears_the_recorded_run_to_run_swing() {
        let baseline =
            parse_baseline(&header_and(&[REFERENCE_ROW, GOOD_ROW])).expect("well formed");
        // The reference at 2.2x its baseline — the worst swing Handover 10
        // recorded between identical binaries — must still pass.
        let swung = runs_with_reference(&[(519, 541, 220, 264); 3]);
        let comparisons =
            check_against_baseline(&baseline, &swung, "release", TIER0_POLICY).expect("comparable");
        assert!(
            comparisons.iter().all(|c| c.verdict != Verdict::Regressed),
            "a 2.2x runner swing on unchanged code must not fail this gate"
        );
    }

    // Clause 5: every ratio in a run divides by the reference, so its absence
    // or its being zero is a harness error rather than a pass.
    #[test]
    fn a_run_without_the_reference_metric_is_a_harness_error() {
        let baseline =
            parse_baseline(&header_and(&[REFERENCE_ROW, GOOD_ROW])).expect("well formed");
        let no_reference = parse_stream(
            "TINYOS-MEAS/1 BEGIN tier=T0 arch=x86_64 cycle_source=rdtsc overhead_cycles=36 cycles_per_us=2307\n\
             TINYOS-MEAS/1 METRIC domain=D04 metric=context_switch n=1000 dropped=0 warmup=100 min=236 p50=246 p99=300 p99_9=340 max=400 unit=cycles\n\
             TINYOS-MEAS/1 END metrics=1\n",
        )
        .expect("well formed");
        let three = vec![no_reference.clone(), no_reference.clone(), no_reference];
        assert!(matches!(
            check_against_baseline(&baseline, &three, "release", TIER0_POLICY),
            Err(GateError::ReferenceNotMeasured { run: 1 })
        ));
    }

    #[test]
    fn a_run_whose_reference_measured_zero_is_a_harness_error() {
        let baseline =
            parse_baseline(&header_and(&[REFERENCE_ROW, GOOD_ROW])).expect("well formed");
        let zeroed =
            runs_with_reference(&[(236, 246, 100, 120), (236, 246, 0, 0), (236, 246, 100, 120)]);
        assert!(matches!(
            check_against_baseline(&baseline, &zeroed, "release", TIER0_POLICY),
            Err(GateError::ReferenceZero { run: 2 })
        ));
    }

    // Clause 6: the new columns join the fail-closed set.
    #[test]
    fn a_baseline_without_a_reference_row_is_an_error() {
        assert!(matches!(
            parse_baseline(&header_and(&[GOOD_ROW])),
            Err(GateError::ReferenceNotBaselined)
        ));
    }

    #[test]
    fn a_reference_row_whose_ratio_is_not_unity_is_an_error() {
        let bad =
            "REF\tfixed_integer_loop\tT0\tx86_64\trelease\trdtsc\t3\t100\t120\t900000\t1000000\t2026-07-27";
        assert!(matches!(
            parse_baseline(&header_and(&[bad, GOOD_ROW])),
            Err(GateError::ReferenceRatioNotUnity { .. })
        ));
    }

    #[test]
    fn a_non_numeric_ratio_column_is_an_error() {
        let bad =
            "D04\tcontext_switch\tT0\tx86_64\trelease\trdtsc\t3\t236\t246\tfast\t2050000\t2026-07-27";
        assert!(matches!(
            parse_baseline(&header_and(&[REFERENCE_ROW, bad])),
            Err(GateError::NotANumber { column: "min_ratio_ppm", .. })
        ));
    }

    #[test]
    fn a_zero_ratio_column_is_an_error_rather_than_an_unreachable_baseline() {
        let bad =
            "D04\tcontext_switch\tT0\tx86_64\trelease\trdtsc\t3\t236\t246\t0\t2050000\t2026-07-27";
        assert!(matches!(
            parse_baseline(&header_and(&[REFERENCE_ROW, bad])),
            Err(GateError::ZeroRatio { .. })
        ));
    }

    // Clause 7: the sensitivity claimed is a tested number, pinned at the
    // committed tolerance's own boundary rather than asserted in prose.
    #[test]
    fn a_regression_at_the_tolerance_boundary_is_caught_and_one_cycle_under_it_is_not() {
        let baseline =
            parse_baseline(&header_and(&[REFERENCE_ROW, GOOD_ROW])).expect("well formed");
        let baseline_ratio = 2_050_000u64; // GOOD_ROW's p50 ratio.
        let limit = TIER0_POLICY.ratio.limit(baseline_ratio);
        // Hold the reference at its baseline so the metric's p50 maps one-to-one
        // onto a ratio: p50 = ratio * 120 / 1_000_000.
        let at_limit = limit * 120 / PPM;
        let comparisons = check_against_baseline(
            &baseline,
            &runs_with_reference(&[(236, at_limit, 100, 120); 3]),
            "release",
            TIER0_POLICY,
        )
        .expect("comparable");
        let p50 = comparisons
            .iter()
            .find(|c| c.key == "D04/context_switch" && c.statistic == "p50")
            .expect("compared");
        assert_ne!(p50.verdict, Verdict::Regressed, "a metric exactly at the limit passes");

        // One measurement quantum past it. The harness records even cycle
        // counts, so 2 is the smallest step that can actually be observed —
        // a "+1 cycle" case would be testing a value no run can produce.
        let past_limit = at_limit + 2;
        let comparisons = check_against_baseline(
            &baseline,
            &runs_with_reference(&[(236, past_limit, 100, 120); 3]),
            "release",
            TIER0_POLICY,
        )
        .expect("comparable");
        let p50 = comparisons
            .iter()
            .find(|c| c.key == "D04/context_switch" && c.statistic == "p50")
            .expect("compared");
        assert_eq!(p50.verdict, Verdict::Regressed);
    }

    // Clause 6: the absolutes travel with every comparison so the gate can
    // report them, labelled, alongside the quantity it actually gated.
    #[test]
    fn every_comparison_carries_the_absolute_it_did_not_gate_on() {
        let baseline =
            parse_baseline(&header_and(&[REFERENCE_ROW, GOOD_ROW])).expect("well formed");
        let comparisons = check_against_baseline(
            &baseline,
            &runs(&[(236, 246), (240, 250), (244, 252)]),
            "release",
            TIER0_POLICY,
        )
        .expect("comparable");
        let p50 = comparisons
            .iter()
            .find(|c| c.key == "D04/context_switch" && c.statistic == "p50")
            .expect("compared");
        assert_eq!(p50.quantity, Quantity::RatioPpm);
        assert_eq!(p50.observed_cycles, 250);
        assert_eq!(p50.baseline_cycles, 246);
    }

    // Clause 4: the committed constants are the ones the Report derives, and
    // a silent edit to either is a test failure rather than a quiet loosening.
    #[test]
    fn the_committed_tolerances_are_the_derived_ones() {
        assert_eq!(TIER0_POLICY.ratio, TIER0_RATIO_TOLERANCE);
        assert_eq!(TIER0_POLICY.reference, REFERENCE_TOLERANCE);
    }

    // Clause 9 / the narrowing this Story's calibration forced: the ungated
    // set is exactly what the Report justifies, and every entry states why in
    // the source. Pinning it here is what stops a metric being quietly moved
    // out of the gate to turn CI green.
    #[test]
    fn the_ungated_set_is_exactly_the_metrics_the_calibration_disqualified() {
        assert_eq!(UNGATED_AT_TIER0.len(), 1);
        assert!(is_ungated("D07/pool_u64x64_alloc_free_round_trip"));
        assert!(!is_ungated("D05/dispatch_select_highest_priority_ready"));
        assert!(!is_ungated(REFERENCE_METRIC));
        assert!(
            UNGATED_AT_TIER0.iter().all(|(_, reason)| reason.len() > 40),
            "an ungated metric must carry a stated reason, not an empty string"
        );
    }

    // An ungated metric is still measured, still baselined, and still printed —
    // only the conclusion is withheld. A regression in one cannot fail the
    // gate, and that is the deliberate cost recorded in the Report.
    #[test]
    fn an_ungated_metric_is_compared_but_never_regresses() {
        let ungated = "D07\tpool_u64x64_alloc_free_round_trip\tT0\tx86_64\trelease\trdtsc\t3\t10\t12\t100000\t100000\t2026-07-27";
        let baseline = parse_baseline(&header_and(&[REFERENCE_ROW, ungated])).expect("well formed");
        let stream = |min: u64, p50: u64| {
            parse_stream(&format!(
                "TINYOS-MEAS/1 BEGIN tier=T0 arch=x86_64 cycle_source=rdtsc overhead_cycles=36 cycles_per_us=2307\n\
                 TINYOS-MEAS/1 METRIC domain=REF metric=fixed_integer_loop n=1000 dropped=0 warmup=100 min=100 p50=120 p99=200 p99_9=220 max=240 unit=cycles\n\
                 TINYOS-MEAS/1 METRIC domain=D07 metric=pool_u64x64_alloc_free_round_trip n=1000 dropped=0 warmup=100 min={min} p50={p50} p99=900 p99_9=900 max=900 unit=cycles\n\
                 TINYOS-MEAS/1 END metrics=2\n"
            ))
            .expect("well formed")
        };
        // A 50x slowdown, which any gated metric would fail on outright.
        let runs = vec![stream(500, 600), stream(500, 600), stream(500, 600)];
        let comparisons =
            check_against_baseline(&baseline, &runs, "release", TIER0_POLICY).expect("comparable");
        let ungated: Vec<_> = comparisons
            .iter()
            .filter(|c| c.key == "D07/pool_u64x64_alloc_free_round_trip")
            .collect();
        assert_eq!(ungated.len(), 2, "it is still compared on min and p50");
        assert!(ungated.iter().all(|c| c.verdict == Verdict::ReportedNotGated));
        // ... and its ratio is still computed, so the number is on screen.
        assert_eq!(ungated[1].observed, 600 * PPM / 120);
    }

    // ---------------------------------------------------------------------
    // STORY-P1-01-02's fail-closed set, unchanged in intent and re-stated
    // against the twelve-column baseline this Story introduces.
    // ---------------------------------------------------------------------

    // Clause 5: the tolerance model, stated as arithmetic.
    #[test]
    fn the_tolerance_is_the_larger_of_its_relative_and_absolute_terms() {
        let tolerance = Tolerance { relative_percent: 20, absolute_floor: 8 };
        // Relative term dominates on a large baseline: 500 + 100.
        assert_eq!(tolerance.limit(500), 600);
        // Absolute floor dominates on a small one: 20% of 10 is 2, floor is 8.
        assert_eq!(tolerance.limit(10), 18);
        // A zero baseline still admits the absolute floor rather than pinning
        // the metric at exactly zero forever.
        assert_eq!(tolerance.limit(0), 8);
    }

    // Clause 4: median across runs, lower-middle on ties.
    #[test]
    fn the_compared_value_is_the_median_across_runs() {
        assert_eq!(median(&[10, 30, 20]), Some(20));
        assert_eq!(median(&[10]), Some(10));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn an_even_run_count_takes_the_lower_middle_not_an_average() {
        // Averaging would give 25 — a number no run ever observed.
        assert_eq!(median(&[10, 20, 30, 40]), Some(20));
    }

    // Clause 3: baseline parsing fails closed.
    #[test]
    fn a_well_formed_baseline_parses() {
        let baseline =
            parse_baseline(&header_and(&[REFERENCE_ROW, GOOD_ROW])).expect("well formed");
        assert_eq!(baseline.rows.len(), 2);
        let row = baseline.row("D04/context_switch").expect("present");
        assert_eq!(row.min_cycles, 236);
        assert_eq!(row.p50_cycles, 246);
        assert_eq!(row.min_ratio_ppm, 2_360_000);
        assert_eq!(row.p50_ratio_ppm, 2_050_000);
        assert_eq!(row.profile, "release");
        assert_eq!(row.recorded_on, RECORDED);
    }

    #[test]
    fn a_wrong_or_missing_header_is_an_error() {
        assert!(matches!(parse_baseline(""), Err(GateError::BadHeader { .. })));
        assert!(matches!(parse_baseline(GOOD_ROW), Err(GateError::BadHeader { .. })));
        let wrong = format!("domain\tmetric\n{GOOD_ROW}\n");
        assert!(matches!(parse_baseline(&wrong), Err(GateError::BadHeader { .. })));
        // The ten-column header STORY-P1-01-02 committed is now one of the
        // wrong ones: a baseline recorded before the ratio columns existed
        // carries no ratios, and reading it as though it did would gate on
        // whichever number happened to land in the new position.
        let ten_column =
            "domain\tmetric\ttier\tarch\tprofile\tcycle_source\truns\tmin_cycles\tp50_cycles\trecorded_on";
        assert!(matches!(
            parse_baseline(&format!("{ten_column}\n")),
            Err(GateError::BadHeader { .. })
        ));
    }

    #[test]
    fn a_row_with_the_wrong_field_count_is_an_error() {
        let short = "D04\tcontext_switch\tT0\tx86_64\trelease\trdtsc\t3\t236\t246";
        assert!(matches!(
            parse_baseline(&header_and(&[short])),
            Err(GateError::FieldCount { line: 2, found: 9 })
        ));
    }

    #[test]
    fn a_non_numeric_column_is_an_error() {
        let bad =
            "D04\tcontext_switch\tT0\tx86_64\trelease\trdtsc\t3\tfast\t246\t2360000\t2050000\t2026-07-27";
        assert!(matches!(
            parse_baseline(&header_and(&[REFERENCE_ROW, bad])),
            Err(GateError::NotANumber { column: "min_cycles", .. })
        ));
    }

    #[test]
    fn a_baseline_whose_min_exceeds_its_p50_is_an_error() {
        let bad =
            "D04\tcontext_switch\tT0\tx86_64\trelease\trdtsc\t3\t900\t246\t2360000\t2050000\t2026-07-27";
        assert!(matches!(
            parse_baseline(&header_and(&[REFERENCE_ROW, bad])),
            Err(GateError::NonMonotonic { .. })
        ));
    }

    #[test]
    fn a_baseline_recorded_over_zero_runs_is_an_error() {
        let bad =
            "D04\tcontext_switch\tT0\tx86_64\trelease\trdtsc\t0\t236\t246\t2360000\t2050000\t2026-07-27";
        assert!(matches!(
            parse_baseline(&header_and(&[REFERENCE_ROW, bad])),
            Err(GateError::ZeroRuns { .. })
        ));
    }

    #[test]
    fn an_empty_field_is_an_error_rather_than_an_empty_provenance() {
        let bad =
            "D04\tcontext_switch\tT0\t\trelease\trdtsc\t3\t236\t246\t2360000\t2050000\t2026-07-27";
        assert!(matches!(
            parse_baseline(&header_and(&[REFERENCE_ROW, bad])),
            Err(GateError::EmptyField { column: "arch", .. })
        ));
    }

    #[test]
    fn a_duplicated_metric_key_is_an_error() {
        assert!(matches!(
            parse_baseline(&header_and(&[REFERENCE_ROW, GOOD_ROW, GOOD_ROW])),
            Err(GateError::DuplicateMetric { .. })
        ));
    }

    #[test]
    fn a_header_only_baseline_has_no_rows_and_is_an_error() {
        assert!(matches!(parse_baseline(&header_and(&[])), Err(GateError::NoRows)));
    }

    // Clause 4/5: the comparison itself, now over ratios.
    #[test]
    fn runs_inside_tolerance_pass() {
        let baseline =
            parse_baseline(&header_and(&[REFERENCE_ROW, GOOD_ROW])).expect("well formed");
        // Baseline ratios 2.36 / 2.05; observed medians 240/100 and 250/120.
        let comparisons = check_against_baseline(
            &baseline,
            &runs(&[(238, 248), (240, 250), (244, 252)]),
            "release",
            TIER0_POLICY,
        )
        .expect("comparable");
        assert_eq!(comparisons.len(), 4, "two statistics for the metric and two for the reference");
        assert!(comparisons.iter().all(|c| c.verdict == Verdict::Pass));
        let metric: Vec<_> = comparisons.iter().filter(|c| c.key == "D04/context_switch").collect();
        assert_eq!(metric[0].statistic, "min");
        assert_eq!(metric[0].observed, 2_400_000);
        assert_eq!(metric[1].statistic, "p50");
        assert_eq!(metric[1].observed, 2_083_333);
    }

    #[test]
    fn a_run_beyond_the_tolerance_regresses_and_names_its_numbers() {
        let baseline =
            parse_baseline(&header_and(&[REFERENCE_ROW, GOOD_ROW])).expect("well formed");
        // The reference is held at its baseline, so this is a genuine slowdown
        // of the measured path rather than a slow runner: p50 246 -> 900 is a
        // 3.7x move in the ratio, past any tolerance this gate could commit to.
        let comparisons = check_against_baseline(
            &baseline,
            &runs(&[(880, 890), (890, 900), (895, 910)]),
            "release",
            TIER0_POLICY,
        )
        .expect("comparable");
        let p50 = comparisons
            .iter()
            .find(|c| c.key == "D04/context_switch" && c.statistic == "p50")
            .expect("p50 compared");
        assert_eq!(p50.verdict, Verdict::Regressed);
        assert_eq!(p50.quantity, Quantity::RatioPpm);
        assert_eq!(p50.baseline, 2_050_000);
        assert_eq!(p50.observed, 900 * PPM / 120);
        assert_eq!(p50.limit, TIER0_POLICY.ratio.limit(2_050_000));
        // The absolute travels with it, so the gate's output can report the
        // cycle counts a reader needs to diagnose the failure.
        assert_eq!(p50.observed_cycles, 900);
        assert_eq!(p50.baseline_cycles, 246);
    }

    #[test]
    fn an_improvement_beyond_tolerance_is_reported_but_is_not_a_failure() {
        let baseline =
            parse_baseline(&header_and(&[REFERENCE_ROW, GOOD_ROW])).expect("well formed");
        let comparisons = check_against_baseline(
            &baseline,
            &runs(&[(20, 22), (21, 23), (22, 24)]),
            "release",
            TIER0_POLICY,
        )
        .expect("comparable");
        let metric: Vec<_> = comparisons.iter().filter(|c| c.key == "D04/context_switch").collect();
        assert!(metric.iter().all(|c| c.verdict == Verdict::ImprovedBeyondTolerance));
        // ... and the reference, which did not move, is not swept along with it.
        let reference: Vec<_> = comparisons.iter().filter(|c| c.key == REFERENCE_METRIC).collect();
        assert!(reference.iter().all(|c| c.verdict == Verdict::Pass));
    }

    // Clause 2: provenance is enforced, not decorative.
    #[test]
    fn a_baseline_from_another_profile_is_refused_rather_than_absorbed() {
        let dev =
            "D04\tcontext_switch\tT0\tx86_64\tdev\trdtsc\t3\t236\t246\t2360000\t2050000\t2026-07-27";
        let reference =
            "REF\tfixed_integer_loop\tT0\tx86_64\tdev\trdtsc\t3\t100\t120\t1000000\t1000000\t2026-07-27";
        let baseline = parse_baseline(&header_and(&[reference, dev])).expect("well formed");
        assert!(matches!(
            check_against_baseline(&baseline, &runs(&[(236, 246); 3]), "release", TIER0_POLICY),
            Err(GateError::ProvenanceMismatch { field: "profile", .. })
        ));
    }

    #[test]
    fn a_tier_1_run_can_never_be_compared_against_a_tier_0_baseline() {
        let t1 =
            "D04\tcontext_switch\tT1\tx86_64\trelease\trdtsc\t3\t236\t246\t2360000\t2050000\t2026-07-27";
        let reference =
            "REF\tfixed_integer_loop\tT1\tx86_64\trelease\trdtsc\t3\t100\t120\t1000000\t1000000\t2026-07-27";
        let baseline = parse_baseline(&header_and(&[reference, t1])).expect("well formed");
        assert!(matches!(
            check_against_baseline(&baseline, &runs(&[(236, 246); 3]), "release", TIER0_POLICY),
            Err(GateError::ProvenanceMismatch { field: "tier", .. })
        ));
    }

    // Clause 6: set disagreements are errors, in both directions.
    #[test]
    fn a_baselined_metric_that_was_not_measured_is_an_error() {
        let extra =
            "D07\tpool_alloc\tT0\tx86_64\trelease\trdtsc\t3\t70\t78\t700000\t650000\t2026-07-27";
        let baseline =
            parse_baseline(&header_and(&[REFERENCE_ROW, GOOD_ROW, extra])).expect("well formed");
        assert!(matches!(
            check_against_baseline(&baseline, &runs(&[(236, 246); 3]), "release", TIER0_POLICY),
            Err(GateError::MetricNotMeasured { .. })
        ));
    }

    #[test]
    fn a_measured_metric_with_no_baseline_is_an_error_not_a_silent_skip() {
        // The baseline covers the reference and D04; the runs measure D07 as
        // well, so the extra metric is measured but ungated — which must be
        // refused rather than quietly dropped, or a new metric looks gated
        // when it is not.
        let three_metric = parse_stream(
            "TINYOS-MEAS/1 BEGIN tier=T0 arch=x86_64 cycle_source=rdtsc overhead_cycles=36 cycles_per_us=2307\n\
             TINYOS-MEAS/1 METRIC domain=REF metric=fixed_integer_loop n=1000 dropped=0 warmup=100 min=100 p50=120 p99=200 p99_9=220 max=240 unit=cycles\n\
             TINYOS-MEAS/1 METRIC domain=D04 metric=context_switch n=1000 dropped=0 warmup=100 min=236 p50=246 p99=300 p99_9=340 max=400 unit=cycles\n\
             TINYOS-MEAS/1 METRIC domain=D07 metric=pool_alloc n=1000 dropped=0 warmup=100 min=70 p50=78 p99=90 p99_9=95 max=99 unit=cycles\n\
             TINYOS-MEAS/1 END metrics=3\n",
        )
        .expect("well formed");
        let baseline =
            parse_baseline(&header_and(&[REFERENCE_ROW, GOOD_ROW])).expect("well formed");
        let three = vec![three_metric.clone(), three_metric.clone(), three_metric];
        assert!(matches!(
            check_against_baseline(&baseline, &three, "release", TIER0_POLICY),
            Err(GateError::MetricNotBaselined { .. })
        ));
    }

    #[test]
    fn fewer_than_three_runs_cannot_conclude() {
        let baseline =
            parse_baseline(&header_and(&[REFERENCE_ROW, GOOD_ROW])).expect("well formed");
        assert!(matches!(
            check_against_baseline(&baseline, &runs(&[(236, 246); 2]), "release", TIER0_POLICY),
            Err(GateError::TooFewRuns { found: 2, required: MINIMUM_RUNS })
        ));
    }

    // Clause 2/8: what `--update-baseline` writes is what the parser accepts,
    // ratio columns included.
    #[test]
    fn a_rendered_baseline_round_trips_through_the_parser_including_its_ratios() {
        let text = render_baseline(
            &runs_with_reference(&[
                (236, 246, 100, 120),
                (238, 248, 100, 120),
                (240, 250, 100, 120),
            ]),
            "release",
            RECORDED,
        )
        .expect("renderable");
        let baseline = parse_baseline(&text).expect("rendered baselines must parse");
        assert_eq!(baseline.rows.len(), 2);
        let metric = baseline.row("D04/context_switch").expect("present");
        assert_eq!(metric.min_cycles, 238);
        assert_eq!(metric.p50_cycles, 248);
        assert_eq!(metric.min_ratio_ppm, 2_380_000);
        assert_eq!(metric.p50_ratio_ppm, 248 * PPM / 120);
        assert_eq!(metric.runs, 3);
        assert_eq!(metric.profile, "release");
        // The reference's own row is unity, which is what the parser demands.
        let reference = baseline.row(REFERENCE_METRIC).expect("present");
        assert_eq!(reference.min_ratio_ppm, PPM);
        assert_eq!(reference.p50_ratio_ppm, PPM);
    }

    // Clause 8: a rendered baseline, compared against the runs it was
    // rendered from, passes. A generator whose own output fails its own gate
    // would make every baseline refresh a coin toss.
    #[test]
    fn a_rendered_baseline_passes_against_the_runs_it_came_from() {
        let measured =
            runs_with_reference(&[(236, 246, 100, 120), (238, 248, 104, 126), (240, 250, 96, 116)]);
        let text = render_baseline(&measured, "release", RECORDED).expect("renderable");
        let baseline = parse_baseline(&text).expect("parses");
        let comparisons = check_against_baseline(&baseline, &measured, "release", TIER0_POLICY)
            .expect("comparable");
        assert!(comparisons.iter().all(|c| c.verdict == Verdict::Pass));
    }

    #[test]
    fn the_committed_tier0_baseline_parses_and_carries_a_reference_row() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .and_then(std::path::Path::parent)
            .expect("xtask manifest lives at os/src/xtask")
            .join("goals")
            .join("performance")
            .join("baselines")
            .join("tier0-x86_64.tsv");
        let text = std::fs::read_to_string(&path).expect("the committed baseline must exist");
        let baseline = parse_baseline(&text).expect("the committed baseline must parse");
        assert_eq!(baseline.rows.len(), 7, "six measured metrics plus the reference");
        assert!(baseline.rows.iter().all(|row| row.profile == "release"));
        assert!(baseline.rows.iter().all(|row| row.tier == "T0"));
        let keys: BTreeSet<String> = baseline.rows.iter().map(BaselineRow::key).collect();
        assert!(keys.contains("D04/context_switch_yield_roundtrip_2switches"));
        assert!(keys.contains(REFERENCE_METRIC));
    }
}
