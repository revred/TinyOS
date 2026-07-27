//! Host-side parser for the kernel measurement harness's `TINYOS-MEAS/1`
//! envelope (`STORY-P1-01-01`), plus the run-to-run variance arithmetic the
//! Story's third acceptance criterion requires.
//!
//! The one rule this module exists to enforce: **a malformed measurement
//! stream is a harness error, never a pass and never a silently smaller
//! sample set** (`FEAT-P1-01`'s containment contract, boundary tests
//! `BND-15`/`BND-16`/`BND-17`). Timing evidence that parsed "mostly fine" is
//! how a regression gate quietly stops gating — so every deviation from the
//! envelope this module knows, including output that simply stops
//! mid-stream, is an error carrying enough context to debug it.
//!
//! Non-envelope lines are ignored: a fixture may legitimately print its own
//! progress or self-check chatter on the same UART. A line carrying the
//! `TINYOS-MEAS` sentinel, however, is always parsed strictly — the sentinel
//! is the claim "this is measurement evidence", and evidence is never
//! best-effort.

use std::collections::BTreeSet;
use std::fmt;

/// The only envelope version this parser accepts. An unknown version is an
/// error rather than a best-effort parse: the format's whole purpose is that
/// a consumer knows exactly which keys carry which meaning.
pub const SUPPORTED_ENVELOPE: &str = "TINYOS-MEAS/1";

/// The sentinel every envelope line starts with, regardless of version — how
/// this parser tells "a measurement line I must validate" from "unrelated
/// fixture chatter I must ignore".
const SENTINEL: &str = "TINYOS-MEAS";

/// One parsed `METRIC` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricRecord {
    /// Performance-catalogue domain this metric is evidence for.
    pub domain: String,
    /// Metric name, unique within a run.
    pub metric: String,
    /// Samples behind the percentiles.
    pub n: u64,
    /// Samples the guest-side buffer refused for lack of capacity.
    pub dropped: u64,
    /// Unmeasured warmup iterations.
    pub warmup: u64,
    /// Smallest sample.
    pub min: u64,
    /// 50th percentile.
    pub p50: u64,
    /// 99th percentile.
    pub p99: u64,
    /// 99.9th percentile.
    pub p99_9: u64,
    /// Largest sample.
    pub max: u64,
    /// Unit the values above are denominated in (`cycles`).
    pub unit: String,
}

impl MetricRecord {
    /// `domain/metric`, the key that must be unique within a run and
    /// identical across runs being compared.
    pub fn key(&self) -> String {
        format!("{}/{}", self.domain, self.metric)
    }
}

/// One parsed run: the `BEGIN` line's environment plus every `METRIC` line,
/// in emission order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// Test-matrix tier the run came from.
    pub tier: String,
    /// Target architecture.
    pub arch: String,
    /// Cycle-source implementor name.
    pub cycle_source: String,
    /// Calibrated per-sample overhead already subtracted guest-side.
    pub overhead_cycles: u64,
    /// Cycles per microsecond, or `None` when the guest reported `unknown`.
    pub cycles_per_us: Option<u32>,
    /// The metrics, in emission order.
    pub metrics: Vec<MetricRecord>,
}

impl Envelope {
    /// Looks a metric up by `domain/metric` key.
    pub fn metric(&self, key: &str) -> Option<&MetricRecord> {
        self.metrics.iter().find(|record| record.key() == key)
    }
}

/// Every way a measurement stream can fail to be evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimingError {
    /// A sentinel-bearing line named a version this parser does not know.
    UnsupportedVersion {
        /// The version token as found.
        found: String,
    },
    /// A sentinel-bearing line was neither `BEGIN`, `METRIC`, nor `END`.
    UnknownRecordKind {
        /// The token found where a record kind was expected.
        found: String,
    },
    /// No `BEGIN` line was present, or a `METRIC`/`END` preceded it.
    MissingBegin,
    /// A second `BEGIN` appeared inside an open envelope.
    RepeatedBegin,
    /// The stream ended without an `END` line — the shape a crashed guest or
    /// a stalled UART actually produces.
    MissingEnd,
    /// A second `END` appeared.
    RepeatedEnd,
    /// The `END` line's count disagreed with the `METRIC` lines seen.
    MetricCountMismatch {
        /// What `END metrics=` claimed.
        declared: u64,
        /// What was actually parsed.
        observed: u64,
    },
    /// A required key was absent from a record.
    MissingKey {
        /// Record kind the key was expected on.
        record: &'static str,
        /// The absent key.
        key: &'static str,
    },
    /// A record carried a key this version does not define.
    UnknownKey {
        /// Record kind the key appeared on.
        record: &'static str,
        /// The undefined key.
        key: String,
    },
    /// A record carried the same key twice.
    DuplicateKey {
        /// Record kind the key appeared on.
        record: &'static str,
        /// The repeated key.
        key: String,
    },
    /// A field expected to be a number was not one.
    NotANumber {
        /// The offending key.
        key: String,
        /// The offending value.
        value: String,
    },
    /// A token in a record was not a `key=value` pair at all.
    MalformedField {
        /// Record kind the token appeared on.
        record: &'static str,
        /// The offending token.
        token: String,
    },
    /// Two `METRIC` lines shared one `domain/metric` key.
    DuplicateMetric {
        /// The repeated key.
        key: String,
    },
    /// A metric's percentiles were not `min <= p50 <= p99 <= p99.9 <= max`.
    NonMonotonicPercentiles {
        /// Which metric.
        key: String,
    },
    /// A metric claimed zero samples: nothing was measured, so the line is
    /// not evidence.
    EmptyMetric {
        /// Which metric.
        key: String,
    },
    /// A metric reported a unit this parser does not know.
    UnknownUnit {
        /// Which metric.
        key: String,
        /// The unit as found.
        unit: String,
    },
    /// A complete envelope carried no metrics at all.
    NoMetrics,
    /// Runs being compared did not measure the same metric set.
    InconsistentRuns {
        /// Human-readable description of the disagreement.
        detail: String,
    },
    /// The stream carried no `TINYOS-RESULT/1` line: the fixture never said
    /// whether it passed, so the run is not evidence (`STORY-P1-01-02`).
    MissingResult,
    /// More than one result line appeared, so the verdict is ambiguous.
    RepeatedResult,
    /// The result line was present but not well formed.
    MalformedResult {
        /// What was wrong with it.
        detail: String,
    },
    /// The UART verdict and the QEMU `isa-debug-exit` code disagreed — the
    /// Tier 0 cross-check that establishes the UART bit can be trusted on a
    /// board where it is the only bit there is.
    ResultDisagreesWithExitCode {
        /// What the UART line said.
        uart_ok: bool,
        /// What the exit code said.
        exit_ok: bool,
    },
}

impl fmt::Display for TimingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimingError::UnsupportedVersion { found } => write!(
                formatter,
                "measurement stream declares envelope `{found}`, but this xtask only accepts `{SUPPORTED_ENVELOPE}`"
            ),
            TimingError::UnknownRecordKind { found } => {
                write!(formatter, "unknown measurement record kind `{found}`")
            }
            TimingError::MissingBegin => {
                write!(formatter, "measurement stream has no `BEGIN` line before its first record")
            }
            TimingError::RepeatedBegin => {
                write!(formatter, "measurement stream has a second `BEGIN` inside an open envelope")
            }
            TimingError::MissingEnd => write!(
                formatter,
                "measurement stream ends without an `END` line (truncated output — a crashed guest or a stalled UART)"
            ),
            TimingError::RepeatedEnd => {
                write!(formatter, "measurement stream has more than one `END` line")
            }
            TimingError::MetricCountMismatch { declared, observed } => write!(
                formatter,
                "`END metrics={declared}` disagrees with the {observed} METRIC line(s) actually parsed"
            ),
            TimingError::MissingKey { record, key } => {
                write!(formatter, "{record} record is missing required key `{key}`")
            }
            TimingError::UnknownKey { record, key } => {
                write!(formatter, "{record} record carries key `{key}`, which this envelope version does not define")
            }
            TimingError::DuplicateKey { record, key } => {
                write!(formatter, "{record} record repeats key `{key}`")
            }
            TimingError::NotANumber { key, value } => {
                write!(formatter, "key `{key}` expects a number, found `{value}`")
            }
            TimingError::MalformedField { record, token } => {
                write!(formatter, "{record} record has token `{token}`, which is not a `key=value` pair")
            }
            TimingError::DuplicateMetric { key } => {
                write!(formatter, "two METRIC lines report the same metric `{key}`")
            }
            TimingError::NonMonotonicPercentiles { key } => write!(
                formatter,
                "metric `{key}` reports percentiles that are not ordered min <= p50 <= p99 <= p99.9 <= max"
            ),
            TimingError::EmptyMetric { key } => {
                write!(formatter, "metric `{key}` reports n=0: nothing was measured")
            }
            TimingError::UnknownUnit { key, unit } => {
                write!(formatter, "metric `{key}` reports unknown unit `{unit}`")
            }
            TimingError::NoMetrics => {
                write!(formatter, "measurement stream carries no METRIC lines")
            }
            TimingError::InconsistentRuns { detail } => {
                write!(formatter, "measurement runs are not comparable: {detail}")
            }
            TimingError::MissingResult => write!(
                formatter,
                "measurement stream carries no `{RESULT_SENTINEL}` line: the fixture never reported whether it passed"
            ),
            TimingError::RepeatedResult => write!(
                formatter,
                "measurement stream carries more than one `{RESULT_SENTINEL}` line, so its verdict is ambiguous"
            ),
            TimingError::MalformedResult { detail } => {
                write!(formatter, "malformed `{RESULT_SENTINEL}` line: {detail}")
            }
            TimingError::ResultDisagreesWithExitCode { uart_ok, exit_ok } => write!(
                formatter,
                "the fixture's UART verdict (ok={uart_ok}) disagrees with its isa-debug-exit code (ok={exit_ok})"
            ),
        }
    }
}

/// The sentinel every fixture's pass/fail line starts with.
///
/// Deliberately *not* the `TINYOS-MEAS` sentinel: the verdict is not a
/// measurement, it survives independently of the envelope, and `parse_stream`
/// must keep treating it as ordinary chatter rather than an unknown record
/// kind. Its whole reason for existing is `LE-09` piece 4 — a Raspberry Pi 5
/// has no `isa-debug-exit` port, so a gate that can only read a QEMU exit code
/// can never gate a board.
pub const RESULT_SENTINEL: &str = "TINYOS-RESULT/1";

/// A fixture's own self-consistency verdict, as carried over the UART.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    /// Which fixture reported it.
    pub fixture: String,
    /// Whether every self-check in that fixture held.
    pub ok: bool,
}

/// Parses the one result line out of a captured stream, failing closed.
///
/// Exactly one line must carry [`RESULT_SENTINEL`], with exactly the keys
/// `fixture` and `ok`, and `ok` must be exactly `true` or `false`. Everything
/// else — none, several, an extra key, a missing key, a truthy-looking value
/// like `yes` or `1` — is an error, because a verdict this parser had to guess
/// at is not a verdict.
pub fn parse_result(text: &str) -> Result<RunResult, TimingError> {
    let mut found: Option<RunResult> = None;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if !line.starts_with(RESULT_SENTINEL) {
            continue;
        }
        if found.is_some() {
            return Err(TimingError::RepeatedResult);
        }
        let mut tokens = line.split_whitespace();
        tokens.next();
        let fields = parse_fields("RESULT", tokens)
            .map_err(|error| TimingError::MalformedResult { detail: error.to_string() })?;
        let keys: Vec<&str> = fields.iter().map(|(key, _)| key.as_str()).collect();
        if keys != ["fixture", "ok"] {
            return Err(TimingError::MalformedResult {
                detail: format!(
                    "expected exactly `fixture` then `ok`, found `{}`",
                    keys.join("`, `")
                ),
            });
        }
        let ok = match fields[1].1.as_str() {
            "true" => true,
            "false" => false,
            other => {
                return Err(TimingError::MalformedResult {
                    detail: format!("`ok={other}` is neither `true` nor `false`"),
                })
            }
        };
        found = Some(RunResult { fixture: fields[0].1.clone(), ok });
    }
    found.ok_or(TimingError::MissingResult)
}

/// `BEGIN`'s required keys, in the order the harness emits them.
const BEGIN_KEYS: [&str; 5] = ["tier", "arch", "cycle_source", "overhead_cycles", "cycles_per_us"];

/// `METRIC`'s required keys.
const METRIC_KEYS: [&str; 11] =
    ["domain", "metric", "n", "dropped", "warmup", "min", "p50", "p99", "p99_9", "max", "unit"];

/// Parses one measurement stream (the whole captured serial log) into an
/// [`Envelope`], or fails closed with the specific reason.
pub fn parse_stream(text: &str) -> Result<Envelope, TimingError> {
    let mut envelope: Option<Envelope> = None;
    let mut closed = false;
    let mut seen_keys: BTreeSet<String> = BTreeSet::new();

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if !line.starts_with(SENTINEL) {
            continue;
        }
        let mut tokens = line.split_whitespace();
        let version = tokens.next().unwrap_or_default();
        if version != SUPPORTED_ENVELOPE {
            return Err(TimingError::UnsupportedVersion { found: version.to_string() });
        }
        let kind = tokens.next().unwrap_or_default();
        let fields = parse_fields(kind_label(kind)?, tokens)?;
        match kind {
            "BEGIN" => {
                if envelope.is_some() || closed {
                    return Err(TimingError::RepeatedBegin);
                }
                require_exact_keys("BEGIN", &fields, &BEGIN_KEYS)?;
                envelope = Some(Envelope {
                    tier: field(&fields, "BEGIN", "tier")?.to_string(),
                    arch: field(&fields, "BEGIN", "arch")?.to_string(),
                    cycle_source: field(&fields, "BEGIN", "cycle_source")?.to_string(),
                    overhead_cycles: number(&fields, "BEGIN", "overhead_cycles")?,
                    cycles_per_us: optional_factor(&fields)?,
                    metrics: Vec::new(),
                });
            }
            "METRIC" => {
                if closed {
                    return Err(TimingError::RepeatedEnd);
                }
                let Some(open) = envelope.as_mut() else {
                    return Err(TimingError::MissingBegin);
                };
                require_exact_keys("METRIC", &fields, &METRIC_KEYS)?;
                let record = MetricRecord {
                    domain: field(&fields, "METRIC", "domain")?.to_string(),
                    metric: field(&fields, "METRIC", "metric")?.to_string(),
                    n: number(&fields, "METRIC", "n")?,
                    dropped: number(&fields, "METRIC", "dropped")?,
                    warmup: number(&fields, "METRIC", "warmup")?,
                    min: number(&fields, "METRIC", "min")?,
                    p50: number(&fields, "METRIC", "p50")?,
                    p99: number(&fields, "METRIC", "p99")?,
                    p99_9: number(&fields, "METRIC", "p99_9")?,
                    max: number(&fields, "METRIC", "max")?,
                    unit: field(&fields, "METRIC", "unit")?.to_string(),
                };
                validate_metric(&record)?;
                if !seen_keys.insert(record.key()) {
                    return Err(TimingError::DuplicateMetric { key: record.key() });
                }
                open.metrics.push(record);
            }
            "END" => {
                if closed {
                    return Err(TimingError::RepeatedEnd);
                }
                let Some(open) = envelope.as_ref() else {
                    return Err(TimingError::MissingBegin);
                };
                require_exact_keys("END", &fields, &["metrics"])?;
                let declared = number(&fields, "END", "metrics")?;
                let observed = open.metrics.len() as u64;
                if declared != observed {
                    return Err(TimingError::MetricCountMismatch { declared, observed });
                }
                closed = true;
            }
            other => return Err(TimingError::UnknownRecordKind { found: other.to_string() }),
        }
    }

    let Some(envelope) = envelope else {
        return Err(TimingError::MissingBegin);
    };
    if !closed {
        return Err(TimingError::MissingEnd);
    }
    if envelope.metrics.is_empty() {
        return Err(TimingError::NoMetrics);
    }
    Ok(envelope)
}

/// One metric's behavior across repeated runs — the Story's third acceptance
/// criterion ("stable Tier 0 percentile evidence across repeated runs, with
/// run-to-run variance recorded").
#[derive(Debug, Clone, PartialEq)]
pub struct CrossRun {
    /// `domain/metric` key.
    pub key: String,
    /// Each run's p50, in run order.
    pub p50s: Vec<u64>,
    /// Each run's p99, in run order.
    pub p99s: Vec<u64>,
    /// Each run's max, in run order.
    pub maxes: Vec<u64>,
    /// Coefficient of variation of the p99 across runs, as a percentage —
    /// the figure `PERF-D*-G05` states its own jitter budget against.
    pub p99_cv_percent: f64,
}

/// Computes per-metric cross-run variance over two or more parsed runs.
///
/// Runs that do not measure the same metric set are not comparable, and that
/// is an error rather than an intersection: silently comparing whatever
/// happened to appear in both runs is how a dropped metric becomes invisible.
pub fn compare_runs(runs: &[Envelope]) -> Result<Vec<CrossRun>, TimingError> {
    if runs.len() < 2 {
        return Err(TimingError::InconsistentRuns {
            detail: format!("run-to-run variance needs at least 2 runs, got {}", runs.len()),
        });
    }
    let keys: Vec<String> = runs[0].metrics.iter().map(MetricRecord::key).collect();
    let reference: BTreeSet<&String> = keys.iter().collect();
    for (index, run) in runs.iter().enumerate().skip(1) {
        let run_keys: Vec<String> = run.metrics.iter().map(MetricRecord::key).collect();
        let observed: BTreeSet<&String> = run_keys.iter().collect();
        if observed != reference {
            return Err(TimingError::InconsistentRuns {
                detail: format!(
                    "run {} measures {{{}}}, run 1 measures {{{}}}",
                    index + 1,
                    run_keys.join(", "),
                    keys.join(", ")
                ),
            });
        }
    }

    let mut comparisons = Vec::with_capacity(keys.len());
    for key in keys {
        let mut p50s = Vec::with_capacity(runs.len());
        let mut p99s = Vec::with_capacity(runs.len());
        let mut maxes = Vec::with_capacity(runs.len());
        for run in runs {
            let record = run.metric(&key).ok_or_else(|| TimingError::InconsistentRuns {
                detail: format!("metric `{key}` vanished between the key check and the comparison"),
            })?;
            p50s.push(record.p50);
            p99s.push(record.p99);
            maxes.push(record.max);
        }
        let p99_cv_percent = coefficient_of_variation_percent(&p99s);
        comparisons.push(CrossRun { key, p50s, p99s, maxes, p99_cv_percent });
    }
    Ok(comparisons)
}

/// Population coefficient of variation of `values`, as a percentage. An
/// all-zero series has no meaningful CV and yields 0.0 rather than a NaN that
/// would silently poison a later comparison.
pub fn coefficient_of_variation_percent(values: &[u64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let count = values.len() as f64;
    let mean = values.iter().map(|value| *value as f64).sum::<f64>() / count;
    if mean == 0.0 {
        return 0.0;
    }
    let variance = values.iter().map(|value| (*value as f64 - mean).powi(2)).sum::<f64>() / count;
    variance.sqrt() / mean * 100.0
}

fn kind_label(kind: &str) -> Result<&'static str, TimingError> {
    match kind {
        "BEGIN" => Ok("BEGIN"),
        "METRIC" => Ok("METRIC"),
        "END" => Ok("END"),
        other => Err(TimingError::UnknownRecordKind { found: other.to_string() }),
    }
}

fn parse_fields<'a, I: Iterator<Item = &'a str>>(
    record: &'static str,
    tokens: I,
) -> Result<Vec<(String, String)>, TimingError> {
    let mut fields = Vec::new();
    for token in tokens {
        let Some((key, value)) = token.split_once('=') else {
            return Err(TimingError::MalformedField { record, token: token.to_string() });
        };
        if key.is_empty() || value.is_empty() {
            return Err(TimingError::MalformedField { record, token: token.to_string() });
        }
        if fields.iter().any(|(seen, _): &(String, String)| seen == key) {
            return Err(TimingError::DuplicateKey { record, key: key.to_string() });
        }
        fields.push((key.to_string(), value.to_string()));
    }
    Ok(fields)
}

fn require_exact_keys(
    record: &'static str,
    fields: &[(String, String)],
    expected: &[&'static str],
) -> Result<(), TimingError> {
    for (key, _) in fields {
        if !expected.contains(&key.as_str()) {
            return Err(TimingError::UnknownKey { record, key: key.clone() });
        }
    }
    for key in expected {
        if !fields.iter().any(|(found, _)| found == key) {
            return Err(TimingError::MissingKey { record, key });
        }
    }
    Ok(())
}

fn field<'a>(
    fields: &'a [(String, String)],
    record: &'static str,
    key: &'static str,
) -> Result<&'a str, TimingError> {
    fields
        .iter()
        .find(|(found, _)| found == key)
        .map(|(_, value)| value.as_str())
        .ok_or(TimingError::MissingKey { record, key })
}

fn number(
    fields: &[(String, String)],
    record: &'static str,
    key: &'static str,
) -> Result<u64, TimingError> {
    let value = field(fields, record, key)?;
    value
        .parse::<u64>()
        .map_err(|_| TimingError::NotANumber { key: key.to_string(), value: value.to_string() })
}

fn optional_factor(fields: &[(String, String)]) -> Result<Option<u32>, TimingError> {
    let value = field(fields, "BEGIN", "cycles_per_us")?;
    if value == "unknown" {
        return Ok(None);
    }
    value.parse::<u32>().map(Some).map_err(|_| TimingError::NotANumber {
        key: "cycles_per_us".to_string(),
        value: value.to_string(),
    })
}

fn validate_metric(record: &MetricRecord) -> Result<(), TimingError> {
    if record.n == 0 {
        return Err(TimingError::EmptyMetric { key: record.key() });
    }
    if record.unit != "cycles" {
        return Err(TimingError::UnknownUnit { key: record.key(), unit: record.unit.clone() });
    }
    let ordered = record.min <= record.p50
        && record.p50 <= record.p99
        && record.p99 <= record.p99_9
        && record.p99_9 <= record.max;
    if !ordered {
        return Err(TimingError::NonMonotonicPercentiles { key: record.key() });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEGIN: &str = "TINYOS-MEAS/1 BEGIN tier=T0 arch=x86_64 cycle_source=rdtsc overhead_cycles=26 cycles_per_us=1000";
    const METRIC_D07: &str = "TINYOS-MEAS/1 METRIC domain=D07 metric=pool_alloc_free n=10000 dropped=0 warmup=500 min=40 p50=44 p99=60 p99_9=120 max=900 unit=cycles";
    const METRIC_D04: &str = "TINYOS-MEAS/1 METRIC domain=D04 metric=context_switch n=5000 dropped=0 warmup=100 min=300 p50=340 p99=520 p99_9=900 max=4000 unit=cycles";

    fn stream(lines: &[&str]) -> String {
        let mut text = String::new();
        for line in lines {
            text.push_str(line);
            text.push('\n');
        }
        text
    }

    // Clause 6, happy path: a well-formed stream parses into records.
    #[test]
    fn a_well_formed_stream_parses_into_per_metric_records() {
        let text = stream(&[BEGIN, METRIC_D07, METRIC_D04, "TINYOS-MEAS/1 END metrics=2"]);
        let envelope = parse_stream(&text).expect("stream is well formed");
        assert_eq!(envelope.tier, "T0");
        assert_eq!(envelope.arch, "x86_64");
        assert_eq!(envelope.cycle_source, "rdtsc");
        assert_eq!(envelope.overhead_cycles, 26);
        assert_eq!(envelope.cycles_per_us, Some(1_000));
        assert_eq!(envelope.metrics.len(), 2);
        let pool = envelope.metric("D07/pool_alloc_free").expect("D07 metric present");
        assert_eq!(pool.n, 10_000);
        assert_eq!(pool.p50, 44);
        assert_eq!(pool.p99_9, 120);
        assert_eq!(pool.unit, "cycles");
        assert_eq!(envelope.metric("D04/context_switch").map(|record| record.max), Some(4_000));
    }

    #[test]
    fn an_unknown_timebase_parses_as_no_timebase_not_as_zero() {
        let begin = BEGIN.replace("cycles_per_us=1000", "cycles_per_us=unknown");
        let text = stream(&[&begin, METRIC_D07, "TINYOS-MEAS/1 END metrics=1"]);
        assert_eq!(parse_stream(&text).expect("well formed").cycles_per_us, None);
    }

    #[test]
    fn unrelated_fixture_chatter_on_the_same_uart_is_ignored() {
        let text = stream(&[
            "fixture-measure starting",
            BEGIN,
            "conformance: cycle source ok (span=812)",
            METRIC_D07,
            "TINYOS-MEAS/1 END metrics=1",
            "fixture-measure overall_ok=true",
        ]);
        assert_eq!(parse_stream(&text).expect("well formed").metrics.len(), 1);
    }

    // `TEST-P1-01-03-A` clause 5: the parser is arch-neutral too, or the
    // measurement harness's arch seam is only half real. This is the exact
    // envelope text `hal_arm64`'s own drop-in test asserts `kernel::measure`
    // emits when driven by the `CNTVCT_EL0` cycle source — copied here as a
    // literal because `xtask` is a host binary and cannot depend on the
    // kernel. No parser code changed to accept it, which is the point.
    #[test]
    fn an_aarch64_envelope_parses_with_no_arch_specific_parser_change() {
        let text = stream(&[
            "TINYOS-MEAS/1 BEGIN tier=T1 arch=aarch64 cycle_source=cntvct_el0 overhead_cycles=0 cycles_per_us=54",
            "TINYOS-MEAS/1 METRIC domain=D04 metric=context_switch n=8 dropped=0 warmup=0 min=10 p50=10 p99=10 p99_9=10 max=10 unit=cycles",
            "TINYOS-MEAS/1 END metrics=1",
        ]);
        let envelope = parse_stream(&text).expect("an aarch64 stream is well formed");
        assert_eq!(envelope.tier, "T1");
        assert_eq!(envelope.arch, "aarch64");
        assert_eq!(envelope.cycle_source, "cntvct_el0");
        assert_eq!(envelope.cycles_per_us, Some(54));
        assert_eq!(envelope.metric("D04/context_switch").map(|record| record.p50), Some(10));
    }

    // `TEST-P1-01-02-A` clause 1: the UART-borne pass/fail bit, which is what
    // a board with no isa-debug-exit port will have instead of an exit code.
    #[test]
    fn a_fixtures_uart_verdict_parses_out_of_its_stream() {
        let text = stream(&[
            "fixture-measure phase 1/5 done",
            BEGIN,
            METRIC_D07,
            "TINYOS-MEAS/1 END metrics=1",
            "TINYOS-RESULT/1 fixture=measure ok=true",
        ]);
        assert_eq!(parse_result(&text), Ok(RunResult { fixture: "measure".to_string(), ok: true }));
        let failed = text.replace("ok=true", "ok=false");
        assert_eq!(parse_result(&failed).map(|result| result.ok), Ok(false));
    }

    #[test]
    fn a_stream_with_no_verdict_is_not_evidence() {
        let text = stream(&[BEGIN, METRIC_D07, "TINYOS-MEAS/1 END metrics=1"]);
        assert_eq!(parse_result(&text), Err(TimingError::MissingResult));
    }

    #[test]
    fn two_verdicts_are_ambiguous_and_therefore_an_error() {
        let text = stream(&[
            "TINYOS-RESULT/1 fixture=measure ok=true",
            "TINYOS-RESULT/1 fixture=measure ok=false",
        ]);
        assert_eq!(parse_result(&text), Err(TimingError::RepeatedResult));
    }

    #[test]
    fn a_malformed_verdict_is_an_error_rather_than_a_default() {
        // A value that is neither `true` nor `false` — including one that
        // looks truthy.
        for line in [
            "TINYOS-RESULT/1 fixture=measure ok=yes",
            "TINYOS-RESULT/1 fixture=measure ok=1",
            "TINYOS-RESULT/1 fixture=measure",
            "TINYOS-RESULT/1 ok=true",
            "TINYOS-RESULT/1 fixture=measure ok=true extra=1",
            "TINYOS-RESULT/1 fixture=measure okay=true",
        ] {
            assert!(
                matches!(parse_result(&stream(&[line])), Err(TimingError::MalformedResult { .. })),
                "`{line}` must be rejected"
            );
        }
    }

    #[test]
    fn the_verdict_line_is_not_a_measurement_record_and_never_breaks_the_envelope() {
        // `parse_stream` must keep treating it as chatter: it does not carry
        // the `TINYOS-MEAS` sentinel, and inventing an unknown record kind
        // here would break every existing capture.
        let text = stream(&[
            BEGIN,
            METRIC_D07,
            "TINYOS-MEAS/1 END metrics=1",
            "TINYOS-RESULT/1 fixture=measure ok=true",
        ]);
        assert_eq!(parse_stream(&text).expect("well formed").metrics.len(), 1);
    }

    // Clause 6, the fail-closed cases — each one exactly one error.
    #[test]
    fn an_unknown_envelope_version_is_rejected_not_best_effort_parsed() {
        let text = stream(&[&BEGIN.replace("/1", "/2"), METRIC_D07, "TINYOS-MEAS/1 END metrics=1"]);
        assert_eq!(
            parse_stream(&text),
            Err(TimingError::UnsupportedVersion { found: "TINYOS-MEAS/2".to_string() })
        );
    }

    #[test]
    fn a_metric_before_any_begin_is_an_error() {
        let text = stream(&[METRIC_D07, "TINYOS-MEAS/1 END metrics=1"]);
        assert_eq!(parse_stream(&text), Err(TimingError::MissingBegin));
    }

    #[test]
    fn a_stream_with_no_envelope_at_all_is_an_error() {
        assert_eq!(parse_stream("nothing to see here\n"), Err(TimingError::MissingBegin));
        assert_eq!(parse_stream(""), Err(TimingError::MissingBegin));
    }

    #[test]
    fn a_truncated_stream_is_an_error_not_a_partial_pass() {
        let text = stream(&[BEGIN, METRIC_D07]);
        assert_eq!(parse_stream(&text), Err(TimingError::MissingEnd));
    }

    #[test]
    fn a_stream_truncated_mid_line_is_an_error() {
        let text = format!("{BEGIN}\n{}", &METRIC_D07[..METRIC_D07.len() - 30]);
        assert!(matches!(
            parse_stream(&text),
            Err(TimingError::MissingKey { record: "METRIC", .. })
        ));
    }

    #[test]
    fn a_repeated_begin_is_an_error() {
        let text = stream(&[BEGIN, BEGIN, METRIC_D07, "TINYOS-MEAS/1 END metrics=1"]);
        assert_eq!(parse_stream(&text), Err(TimingError::RepeatedBegin));
    }

    #[test]
    fn a_repeated_end_is_an_error() {
        let text = stream(&[
            BEGIN,
            METRIC_D07,
            "TINYOS-MEAS/1 END metrics=1",
            "TINYOS-MEAS/1 END metrics=1",
        ]);
        assert_eq!(parse_stream(&text), Err(TimingError::RepeatedEnd));
    }

    #[test]
    fn an_end_count_that_disagrees_with_the_metrics_seen_is_an_error() {
        let text = stream(&[BEGIN, METRIC_D07, METRIC_D04, "TINYOS-MEAS/1 END metrics=3"]);
        assert_eq!(
            parse_stream(&text),
            Err(TimingError::MetricCountMismatch { declared: 3, observed: 2 })
        );
    }

    #[test]
    fn a_missing_metric_key_is_an_error() {
        let text =
            stream(&[BEGIN, &METRIC_D07.replace(" p99_9=120", ""), "TINYOS-MEAS/1 END metrics=1"]);
        assert_eq!(
            parse_stream(&text),
            Err(TimingError::MissingKey { record: "METRIC", key: "p99_9" })
        );
    }

    #[test]
    fn an_unknown_metric_key_is_an_error() {
        let text = stream(&[BEGIN, &format!("{METRIC_D07} p42=7"), "TINYOS-MEAS/1 END metrics=1"]);
        assert_eq!(
            parse_stream(&text),
            Err(TimingError::UnknownKey { record: "METRIC", key: "p42".to_string() })
        );
    }

    #[test]
    fn a_duplicated_key_is_an_error() {
        let text = stream(&[BEGIN, &format!("{METRIC_D07} p50=1"), "TINYOS-MEAS/1 END metrics=1"]);
        assert_eq!(
            parse_stream(&text),
            Err(TimingError::DuplicateKey { record: "METRIC", key: "p50".to_string() })
        );
    }

    #[test]
    fn a_non_numeric_value_is_an_error() {
        let text = stream(&[
            BEGIN,
            &METRIC_D07.replace("p99=60", "p99=sixty"),
            "TINYOS-MEAS/1 END metrics=1",
        ]);
        assert_eq!(
            parse_stream(&text),
            Err(TimingError::NotANumber { key: "p99".to_string(), value: "sixty".to_string() })
        );
    }

    #[test]
    fn a_token_that_is_not_a_key_value_pair_is_an_error() {
        let text =
            stream(&[BEGIN, &format!("{METRIC_D07} garbage"), "TINYOS-MEAS/1 END metrics=1"]);
        assert_eq!(
            parse_stream(&text),
            Err(TimingError::MalformedField { record: "METRIC", token: "garbage".to_string() })
        );
    }

    #[test]
    fn two_metrics_with_the_same_key_are_an_error() {
        let text = stream(&[BEGIN, METRIC_D07, METRIC_D07, "TINYOS-MEAS/1 END metrics=2"]);
        assert_eq!(
            parse_stream(&text),
            Err(TimingError::DuplicateMetric { key: "D07/pool_alloc_free".to_string() })
        );
    }

    #[test]
    fn non_monotonic_percentiles_are_an_error() {
        let text = stream(&[
            BEGIN,
            &METRIC_D07.replace("p99=60", "p99=30"),
            "TINYOS-MEAS/1 END metrics=1",
        ]);
        assert_eq!(
            parse_stream(&text),
            Err(TimingError::NonMonotonicPercentiles { key: "D07/pool_alloc_free".to_string() })
        );
    }

    #[test]
    fn a_metric_with_zero_samples_is_an_error() {
        let text =
            stream(&[BEGIN, &METRIC_D07.replace("n=10000", "n=0"), "TINYOS-MEAS/1 END metrics=1"]);
        assert_eq!(
            parse_stream(&text),
            Err(TimingError::EmptyMetric { key: "D07/pool_alloc_free".to_string() })
        );
    }

    #[test]
    fn an_unknown_unit_is_an_error() {
        let text = stream(&[
            BEGIN,
            &METRIC_D07.replace("unit=cycles", "unit=furlongs"),
            "TINYOS-MEAS/1 END metrics=1",
        ]);
        assert_eq!(
            parse_stream(&text),
            Err(TimingError::UnknownUnit {
                key: "D07/pool_alloc_free".to_string(),
                unit: "furlongs".to_string()
            })
        );
    }

    #[test]
    fn an_envelope_with_no_metrics_is_an_error() {
        let text = stream(&[BEGIN, "TINYOS-MEAS/1 END metrics=0"]);
        assert_eq!(parse_stream(&text), Err(TimingError::NoMetrics));
    }

    #[test]
    fn a_missing_begin_key_is_an_error() {
        let text = stream(&[
            &BEGIN.replace(" arch=x86_64", ""),
            METRIC_D07,
            "TINYOS-MEAS/1 END metrics=1",
        ]);
        assert_eq!(
            parse_stream(&text),
            Err(TimingError::MissingKey { record: "BEGIN", key: "arch" })
        );
    }

    #[test]
    fn an_unknown_record_kind_is_an_error() {
        let text = stream(&[BEGIN, "TINYOS-MEAS/1 MEASUREMENT domain=D07", METRIC_D07]);
        assert_eq!(
            parse_stream(&text),
            Err(TimingError::UnknownRecordKind { found: "MEASUREMENT".to_string() })
        );
    }

    // Clause 7: run-to-run variance.
    #[test]
    fn cross_run_comparison_records_per_metric_variance() {
        let run = |p99: u64| {
            let metric = METRIC_D07.replace("p99=60", &format!("p99={p99}"));
            parse_stream(&stream(&[BEGIN, &metric, "TINYOS-MEAS/1 END metrics=1"]))
                .expect("well formed")
        };
        let comparisons = compare_runs(&[run(60), run(66), run(63)]).expect("runs are comparable");
        assert_eq!(comparisons.len(), 1);
        let comparison = &comparisons[0];
        assert_eq!(comparison.key, "D07/pool_alloc_free");
        assert_eq!(comparison.p99s, vec![60, 66, 63]);
        assert_eq!(comparison.p50s, vec![44, 44, 44]);
        // Population CV of {60, 66, 63}: mean 63, sd sqrt(6) ~= 2.449.
        assert!(
            (comparison.p99_cv_percent - 3.887).abs() < 0.01,
            "unexpected CV {}",
            comparison.p99_cv_percent
        );
    }

    #[test]
    fn identical_runs_have_zero_variance() {
        let run = || {
            parse_stream(&stream(&[BEGIN, METRIC_D07, "TINYOS-MEAS/1 END metrics=1"]))
                .expect("well formed")
        };
        let comparisons = compare_runs(&[run(), run()]).expect("runs are comparable");
        assert_eq!(comparisons[0].p99_cv_percent, 0.0);
    }

    #[test]
    fn runs_measuring_different_metric_sets_are_not_comparable() {
        let one = parse_stream(&stream(&[BEGIN, METRIC_D07, "TINYOS-MEAS/1 END metrics=1"]))
            .expect("well formed");
        let two = parse_stream(&stream(&[BEGIN, METRIC_D04, "TINYOS-MEAS/1 END metrics=1"]))
            .expect("well formed");
        assert!(matches!(compare_runs(&[one, two]), Err(TimingError::InconsistentRuns { .. })));
    }

    #[test]
    fn a_single_run_cannot_establish_run_to_run_variance() {
        let one = parse_stream(&stream(&[BEGIN, METRIC_D07, "TINYOS-MEAS/1 END metrics=1"]))
            .expect("well formed");
        assert!(matches!(compare_runs(&[one]), Err(TimingError::InconsistentRuns { .. })));
    }

    #[test]
    fn an_all_zero_series_has_no_variance_rather_than_a_nan() {
        assert_eq!(coefficient_of_variation_percent(&[0, 0, 0]), 0.0);
        assert_eq!(coefficient_of_variation_percent(&[]), 0.0);
    }
}
