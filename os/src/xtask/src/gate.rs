//! Committed timing baselines and the regression comparison
//! (`STORY-P1-01-02`).
//!
//! Separate from [`crate::timing`] by responsibility: that module turns a
//! fixture's serial output into structured evidence, this one decides whether
//! that evidence is a regression. The split matters because the second
//! question has a different failure mode from the first — a parser that fails
//! closed is useless behind a comparison that quietly passes when it has
//! nothing to compare against.
//!
//! **Why this gate compares what it compares.** `REPORT-2026-07-27-02`
//! measured Tier 0 run-to-run p99 coefficients of variation of **39–61%** on
//! the smallest operations. A gate thresholding a tail on a single run would
//! therefore fail green code roughly as often as it caught anything, and would
//! be switched off by whoever it woke — the failure mode `LE-07` already
//! taught this project once. So:
//!
//! - only `min` and `p50` are gated (the statistics that were stable),
//! - over the **median of at least three runs** (never one), and
//! - the tails are still printed, labelled as reported-and-not-gated, so a
//!   reader never mistakes an ungated number for a passing one.
//!
//! **Provenance is enforced, not decorative.** A baseline row carries its
//! tier, architecture, profile and cycle source, and a comparison across any
//! disagreement is refused outright rather than absorbed into a tolerance:
//! a Tier 0 figure must never be able to masquerade as a hardware one, and a
//! release run compared against a dev-profile baseline is not measuring the
//! same code at all (`LE-13`).

use crate::timing::Envelope;
use std::collections::BTreeSet;
use std::fmt;

/// The baseline file's required header. Exact-match, like every other TSV in
/// this repository's assurance spine: a column silently added or reordered
/// would re-point every number in the file.
pub const BASELINE_HEADER: &str =
    "domain\tmetric\ttier\tarch\tprofile\tcycle_source\truns\tmin_cycles\tp50_cycles\trecorded_on";

/// Field count implied by [`BASELINE_HEADER`].
const BASELINE_FIELDS: usize = 10;

/// The Tier 0 tolerance, derived from this harness's own measured run-to-run
/// spread rather than chosen to make a particular day's numbers pass.
///
/// **The derivation** (five consecutive release-profile runs,
/// `REPORT-2026-07-27-04`). Even the *stable* statistics move more at Tier 0
/// than the first draft of this gate assumed: measured as the worst run's
/// excess over the five-run median, `p50` moved **+23%** (D04 context switch),
/// **+25%** (D05 dispatch round), **+28%** (D07 alloc/free) and **+67%** (D07
/// denial, though that is +12 *cycles* on an 18-cycle metric). A 20% relative
/// tolerance — this gate's first constant — would therefore have failed green
/// code on its first CI run.
///
/// A first constant of 40% was then **falsified by its own first gate run**:
/// D07 alloc/free's `min` came back at 92 against a 66 baseline — +39%, landing
/// exactly on the limit. So the spread is worse than any five-run sample
/// showed, which is the expected behavior of a shared, unpinned host under
/// TCG. The committed constant is **60%**, which clears that excursion with
/// real margin, plus a 24-cycle absolute floor for metrics small enough that
/// percentages stop meaning anything (a 12-cycle baseline is 3 cycles away
/// from a 25% "regression").
///
/// **What that costs, stated plainly**: at Tier 0 this gate can only catch
/// regressions of roughly **1.6x or worse**. It is a tripwire for the kind of
/// mistake that makes a path multiples slower — an accidental O(n) in a
/// selection loop, a lock added to an RT path — and it is *not* a defense
/// against a 10% creep. That is a property of TCG emulation rather than a
/// choice, and it is the most concrete argument yet for the hardware tier
/// (`LE-09`): a gate this loose is the best Tier 0 can honestly support. The
/// injected-regression demonstration lands ~8x over baseline and is caught by
/// an enormous margin.
pub const TIER0_TOLERANCE: Tolerance = Tolerance { relative_percent: 60, absolute_cycles: 24 };

/// The fewest runs the gate will conclude from. Three is the smallest count
/// that has a median at all without averaging two observations.
pub const MINIMUM_RUNS: usize = 3;

/// One committed baseline row: what a metric measured, and everything a
/// reader needs to know what produced that number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineRow {
    /// Performance-catalogue domain (`D04`, `D05`, `D07`, ...).
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
    /// Median-of-runs minimum, in cycles.
    pub min_cycles: u64,
    /// Median-of-runs p50, in cycles.
    pub p50_cycles: u64,
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
/// under `baseline + max(absolute_cycles, baseline * relative_percent / 100)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tolerance {
    /// Relative headroom, as a percentage of the baseline.
    pub relative_percent: u64,
    /// Absolute headroom floor, in cycles — what keeps a 16-cycle metric from
    /// being gated more tightly than the measurement's own granularity.
    pub absolute_cycles: u64,
}

impl Tolerance {
    /// The largest observed value that still passes.
    pub const fn limit(&self, baseline: u64) -> u64 {
        let relative = baseline * self.relative_percent / 100;
        let headroom =
            if relative > self.absolute_cycles { relative } else { self.absolute_cycles };
        baseline + headroom
    }
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
}

/// One statistic of one metric, compared against its baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    /// `domain/metric`.
    pub key: String,
    /// Which statistic — `min` or `p50`. The tails are deliberately absent.
    pub statistic: &'static str,
    /// The committed baseline value.
    pub baseline: u64,
    /// The median across the runs just measured.
    pub observed: u64,
    /// The largest value that would have passed.
    pub limit: u64,
    /// The conclusion.
    pub verdict: Verdict,
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
            GateError::EmptyField { line, column } => {
                write!(formatter, "baseline line {line}: `{column}` is empty")
            }
            GateError::DuplicateMetric { key } => {
                write!(formatter, "baseline carries `{key}` twice")
            }
            GateError::NoRows => {
                write!(formatter, "baseline file carries no rows — there is nothing to gate on")
            }
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
            recorded_on: fields[9].to_string(),
        };
        if row.runs == 0 {
            return Err(GateError::ZeroRuns { key: row.key() });
        }
        if row.min_cycles > row.p50_cycles {
            return Err(GateError::NonMonotonic { key: row.key() });
        }
        if !seen.insert(row.key()) {
            return Err(GateError::DuplicateMetric { key: row.key() });
        }
        rows.push(row);
    }

    if rows.is_empty() {
        return Err(GateError::NoRows);
    }
    Ok(Baseline { rows })
}

/// The lower-middle element of `values`.
///
/// Deliberately not a mean, and deliberately not an interpolated median on an
/// even count: the compared figure stays a cycle count some run actually
/// observed, so a failing gate can always be traced to a real capture.
pub fn median(values: &[u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    Some(sorted[(sorted.len() - 1) / 2])
}

/// Compares parsed runs against a baseline, one [`Comparison`] per gated
/// statistic per metric.
///
/// Every disagreement upstream of the arithmetic — too few runs, provenance
/// mismatch, a metric on one side and not the other — is an error rather than
/// a partial answer.
pub fn check_against_baseline(
    baseline: &Baseline,
    runs: &[Envelope],
    profile: &str,
    tolerance: Tolerance,
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
        let mut mins = Vec::with_capacity(runs.len());
        let mut p50s = Vec::with_capacity(runs.len());
        for run in runs {
            let record = run
                .metric(&key)
                .ok_or_else(|| GateError::MetricNotMeasured { key: key.clone() })?;
            mins.push(record.min);
            p50s.push(record.p50);
        }
        for (statistic, baseline_value, observed_values) in
            [("min", row.min_cycles, mins), ("p50", row.p50_cycles, p50s)]
        {
            let observed = median(&observed_values)
                .ok_or_else(|| GateError::MetricNotMeasured { key: key.clone() })?;
            let limit = tolerance.limit(baseline_value);
            let verdict = if observed > limit {
                Verdict::Regressed
            } else if baseline_value > tolerance.limit(observed) {
                Verdict::ImprovedBeyondTolerance
            } else {
                Verdict::Pass
            };
            comparisons.push(Comparison {
                key: key.clone(),
                statistic,
                baseline: baseline_value,
                observed,
                limit,
                verdict,
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
pub fn render_baseline(
    runs: &[Envelope],
    profile: &str,
    recorded_on: &str,
) -> Result<String, GateError> {
    if runs.len() < MINIMUM_RUNS {
        return Err(GateError::TooFewRuns { found: runs.len(), required: MINIMUM_RUNS });
    }
    let first = &runs[0];
    let mut text = String::from(BASELINE_HEADER);
    text.push('\n');
    for record in &first.metrics {
        let key = record.key();
        let mut mins = Vec::with_capacity(runs.len());
        let mut p50s = Vec::with_capacity(runs.len());
        for run in runs {
            let found = run
                .metric(&key)
                .ok_or_else(|| GateError::MetricNotMeasured { key: key.clone() })?;
            mins.push(found.min);
            p50s.push(found.p50);
        }
        let min = median(&mins).ok_or_else(|| GateError::MetricNotMeasured { key: key.clone() })?;
        let p50 = median(&p50s).ok_or_else(|| GateError::MetricNotMeasured { key: key.clone() })?;
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            record.domain,
            record.metric,
            first.tier,
            first.arch,
            profile,
            first.cycle_source,
            runs.len(),
            min,
            p50,
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

    const GOOD_ROW: &str =
        "D04\tcontext_switch\tT0\tx86_64\trelease\trdtsc\t3\t236\t246\t2026-07-27";

    fn envelope(min: u64, p50: u64) -> Envelope {
        let text = format!(
            "TINYOS-MEAS/1 BEGIN tier=T0 arch=x86_64 cycle_source=rdtsc overhead_cycles=36 cycles_per_us=2307\n\
             TINYOS-MEAS/1 METRIC domain=D04 metric=context_switch n=1000 dropped=0 warmup=100 min={min} p50={p50} p99={p99} p99_9={p99} max={p99} unit=cycles\n\
             TINYOS-MEAS/1 END metrics=1\n",
            p99 = p50 + 100
        );
        parse_stream(&text).expect("fixture stream is well formed")
    }

    fn runs(values: &[(u64, u64)]) -> Vec<Envelope> {
        values.iter().map(|(min, p50)| envelope(*min, *p50)).collect()
    }

    // Clause 5: the tolerance model, stated as arithmetic.
    #[test]
    fn the_tolerance_is_the_larger_of_its_relative_and_absolute_terms() {
        let tolerance = Tolerance { relative_percent: 20, absolute_cycles: 8 };
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
        let baseline = parse_baseline(&header_and(&[GOOD_ROW])).expect("well formed");
        assert_eq!(baseline.rows.len(), 1);
        let row = &baseline.rows[0];
        assert_eq!(row.key(), "D04/context_switch");
        assert_eq!(row.min_cycles, 236);
        assert_eq!(row.p50_cycles, 246);
        assert_eq!(row.profile, "release");
        assert_eq!(row.recorded_on, RECORDED);
    }

    #[test]
    fn a_wrong_or_missing_header_is_an_error() {
        assert!(matches!(parse_baseline(""), Err(GateError::BadHeader { .. })));
        assert!(matches!(parse_baseline(GOOD_ROW), Err(GateError::BadHeader { .. })));
        let wrong = format!("domain\tmetric\n{GOOD_ROW}\n");
        assert!(matches!(parse_baseline(&wrong), Err(GateError::BadHeader { .. })));
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
        let bad = "D04\tcontext_switch\tT0\tx86_64\trelease\trdtsc\t3\tfast\t246\t2026-07-27";
        assert!(matches!(
            parse_baseline(&header_and(&[bad])),
            Err(GateError::NotANumber { column: "min_cycles", .. })
        ));
    }

    #[test]
    fn a_baseline_whose_min_exceeds_its_p50_is_an_error() {
        let bad = "D04\tcontext_switch\tT0\tx86_64\trelease\trdtsc\t3\t900\t246\t2026-07-27";
        assert!(matches!(parse_baseline(&header_and(&[bad])), Err(GateError::NonMonotonic { .. })));
    }

    #[test]
    fn a_baseline_recorded_over_zero_runs_is_an_error() {
        let bad = "D04\tcontext_switch\tT0\tx86_64\trelease\trdtsc\t0\t236\t246\t2026-07-27";
        assert!(matches!(parse_baseline(&header_and(&[bad])), Err(GateError::ZeroRuns { .. })));
    }

    #[test]
    fn an_empty_field_is_an_error_rather_than_an_empty_provenance() {
        let bad = "D04\tcontext_switch\tT0\t\trelease\trdtsc\t3\t236\t246\t2026-07-27";
        assert!(matches!(
            parse_baseline(&header_and(&[bad])),
            Err(GateError::EmptyField { column: "arch", .. })
        ));
    }

    #[test]
    fn a_duplicated_metric_key_is_an_error() {
        assert!(matches!(
            parse_baseline(&header_and(&[GOOD_ROW, GOOD_ROW])),
            Err(GateError::DuplicateMetric { .. })
        ));
    }

    #[test]
    fn a_header_only_baseline_has_no_rows_and_is_an_error() {
        assert!(matches!(parse_baseline(&header_and(&[])), Err(GateError::NoRows)));
    }

    // Clause 4/5: the comparison itself.
    #[test]
    fn runs_inside_tolerance_pass() {
        let baseline = parse_baseline(&header_and(&[GOOD_ROW])).expect("well formed");
        // Baseline min 236 / p50 246; observed medians 240 / 250.
        let comparisons = check_against_baseline(
            &baseline,
            &runs(&[(238, 248), (240, 250), (244, 252)]),
            "release",
            TIER0_TOLERANCE,
        )
        .expect("comparable");
        assert_eq!(comparisons.len(), 2);
        assert!(comparisons.iter().all(|c| c.verdict == Verdict::Pass));
        assert_eq!(comparisons[0].statistic, "min");
        assert_eq!(comparisons[0].observed, 240);
        assert_eq!(comparisons[1].statistic, "p50");
        assert_eq!(comparisons[1].observed, 250);
    }

    #[test]
    fn a_run_beyond_the_tolerance_regresses_and_names_its_numbers() {
        let baseline = parse_baseline(&header_and(&[GOOD_ROW])).expect("well formed");
        // p50 246 + 60% = 393; 405 is past it.
        let comparisons = check_against_baseline(
            &baseline,
            &runs(&[(390, 400), (395, 405), (398, 410)]),
            "release",
            TIER0_TOLERANCE,
        )
        .expect("comparable");
        let p50 = comparisons.iter().find(|c| c.statistic == "p50").expect("p50 compared");
        assert_eq!(p50.verdict, Verdict::Regressed);
        assert_eq!(p50.baseline, 246);
        assert_eq!(p50.observed, 405);
        assert_eq!(p50.limit, 393);
    }

    #[test]
    fn an_improvement_beyond_tolerance_is_reported_but_is_not_a_failure() {
        let baseline = parse_baseline(&header_and(&[GOOD_ROW])).expect("well formed");
        let comparisons = check_against_baseline(
            &baseline,
            &runs(&[(20, 22), (21, 23), (22, 24)]),
            "release",
            TIER0_TOLERANCE,
        )
        .expect("comparable");
        assert!(comparisons.iter().all(|c| c.verdict == Verdict::ImprovedBeyondTolerance));
    }

    // Clause 2: provenance is enforced, not decorative.
    #[test]
    fn a_baseline_from_another_profile_is_refused_rather_than_absorbed() {
        let dev = "D04\tcontext_switch\tT0\tx86_64\tdev\trdtsc\t3\t236\t246\t2026-07-27";
        let baseline = parse_baseline(&header_and(&[dev])).expect("well formed");
        assert!(matches!(
            check_against_baseline(&baseline, &runs(&[(236, 246); 3]), "release", TIER0_TOLERANCE),
            Err(GateError::ProvenanceMismatch { field: "profile", .. })
        ));
    }

    #[test]
    fn a_tier_1_run_can_never_be_compared_against_a_tier_0_baseline() {
        let t1 = "D04\tcontext_switch\tT1\tx86_64\trelease\trdtsc\t3\t236\t246\t2026-07-27";
        let baseline = parse_baseline(&header_and(&[t1])).expect("well formed");
        assert!(matches!(
            check_against_baseline(&baseline, &runs(&[(236, 246); 3]), "release", TIER0_TOLERANCE),
            Err(GateError::ProvenanceMismatch { field: "tier", .. })
        ));
    }

    // Clause 6: set disagreements are errors, in both directions.
    #[test]
    fn a_baselined_metric_that_was_not_measured_is_an_error() {
        let extra = "D07\tpool_alloc\tT0\tx86_64\trelease\trdtsc\t3\t70\t78\t2026-07-27";
        let baseline = parse_baseline(&header_and(&[GOOD_ROW, extra])).expect("well formed");
        assert!(matches!(
            check_against_baseline(&baseline, &runs(&[(236, 246); 3]), "release", TIER0_TOLERANCE),
            Err(GateError::MetricNotMeasured { .. })
        ));
    }

    #[test]
    fn a_measured_metric_with_no_baseline_is_an_error_not_a_silent_skip() {
        // The baseline covers D04 only; the runs measure D04 *and* D07, so the
        // extra metric is measured but ungated — which must be refused rather
        // than quietly dropped, or a new metric looks gated when it is not.
        let two_metric = parse_stream(
            "TINYOS-MEAS/1 BEGIN tier=T0 arch=x86_64 cycle_source=rdtsc overhead_cycles=36 cycles_per_us=2307\n\
             TINYOS-MEAS/1 METRIC domain=D04 metric=context_switch n=1000 dropped=0 warmup=100 min=236 p50=246 p99=300 p99_9=340 max=400 unit=cycles\n\
             TINYOS-MEAS/1 METRIC domain=D07 metric=pool_alloc n=1000 dropped=0 warmup=100 min=70 p50=78 p99=90 p99_9=95 max=99 unit=cycles\n\
             TINYOS-MEAS/1 END metrics=2\n",
        )
        .expect("well formed");
        let baseline = parse_baseline(&header_and(&[GOOD_ROW])).expect("well formed");
        let three = vec![two_metric.clone(), two_metric.clone(), two_metric];
        assert!(matches!(
            check_against_baseline(&baseline, &three, "release", TIER0_TOLERANCE),
            Err(GateError::MetricNotBaselined { .. })
        ));
    }

    #[test]
    fn fewer_than_three_runs_cannot_conclude() {
        let baseline = parse_baseline(&header_and(&[GOOD_ROW])).expect("well formed");
        assert!(matches!(
            check_against_baseline(&baseline, &runs(&[(236, 246); 2]), "release", TIER0_TOLERANCE),
            Err(GateError::TooFewRuns { found: 2, required: MINIMUM_RUNS })
        ));
    }

    // Clause 2: what `--update-baseline` writes is what the parser accepts.
    #[test]
    fn a_rendered_baseline_round_trips_through_the_parser() {
        let text =
            render_baseline(&runs(&[(236, 246), (238, 248), (240, 250)]), "release", RECORDED)
                .expect("renderable");
        let baseline = parse_baseline(&text).expect("rendered baselines must parse");
        assert_eq!(baseline.rows.len(), 1);
        assert_eq!(baseline.rows[0].min_cycles, 238);
        assert_eq!(baseline.rows[0].p50_cycles, 248);
        assert_eq!(baseline.rows[0].runs, 3);
        assert_eq!(baseline.rows[0].profile, "release");
    }

    #[test]
    fn the_committed_tier0_baseline_parses_and_is_release_profile() {
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
        assert_eq!(baseline.rows.len(), 6);
        assert!(baseline.rows.iter().all(|row| row.profile == "release"));
        assert!(baseline.rows.iter().all(|row| row.tier == "T0"));
        let keys: BTreeSet<String> = baseline.rows.iter().map(BaselineRow::key).collect();
        assert!(keys.contains("D04/context_switch_yield_roundtrip_2switches"));
    }
}
