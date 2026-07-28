//! Integrity validation for the 625-test performance catalogue.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const EXPECTED_HEADER: &str =
    "id\tdomain\tguardrail\ttitle\tphase\ttier\treadiness\tgate\tstatus\tmetric\ttarget\tsafety_invariant";
const AXIS_SIZE: usize = 25;
const EXPECTED_TESTS: usize = AXIS_SIZE * AXIS_SIZE;
const FIELD_COUNT: usize = 12;

/// Summary returned after the committed catalogue passes every integrity
/// check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogueSummary {
    /// Number of individually-addressable performance tests.
    pub test_count: usize,
}

/// Validates `goals/performance/catalogue.tsv` relative to `repo_root`.
pub fn check_catalogue(repo_root: &Path) -> Result<CatalogueSummary, String> {
    let path = repo_root.join("goals").join("performance").join("catalogue.tsv");
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    validate_contents(&contents)
}

fn validate_contents(contents: &str) -> Result<CatalogueSummary, String> {
    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "performance catalogue is empty".to_string())?
        .trim_end_matches('\r');
    if header != EXPECTED_HEADER {
        return Err(format!("unexpected header; expected exactly `{EXPECTED_HEADER}`"));
    }

    let mut ids = BTreeSet::new();
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            return Err(format!("line {line_number}: blank rows are not allowed"));
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != FIELD_COUNT {
            return Err(format!(
                "line {line_number}: expected {FIELD_COUNT} tab-separated fields, found {}",
                fields.len()
            ));
        }
        if fields.iter().any(|field| field.trim().is_empty()) {
            return Err(format!("line {line_number}: every field must be non-empty"));
        }

        let id = fields[0];
        let domain = fields[1];
        let guardrail = fields[2];
        validate_axis_id(domain, 'D', line_number)?;
        validate_axis_id(guardrail, 'G', line_number)?;

        let expected_id = format!("PERF-{domain}-{guardrail}");
        if id != expected_id {
            return Err(format!("line {line_number}: id `{id}` does not match `{expected_id}`"));
        }
        if !ids.insert(id.to_string()) {
            return Err(format!("line {line_number}: duplicate id `{id}`"));
        }

        let gate = fields[7];
        if !matches!(gate, "release" | "claim") {
            return Err(format!(
                "line {line_number}: gate must be `release` or `claim`, found `{gate}`"
            ));
        }
        let status = fields[8];
        if status != "specified" {
            return Err(format!(
                "line {line_number}: committed catalogue status must be `specified`, found `{status}`"
            ));
        }
    }

    for domain in 1..=AXIS_SIZE {
        for guardrail in 1..=AXIS_SIZE {
            let expected = format!("PERF-D{domain:02}-G{guardrail:02}");
            if !ids.contains(&expected) {
                return Err(format!("missing catalogue cell `{expected}`"));
            }
        }
    }
    if ids.len() != EXPECTED_TESTS {
        return Err(format!("expected exactly {EXPECTED_TESTS} unique tests, found {}", ids.len()));
    }

    Ok(CatalogueSummary { test_count: ids.len() })
}

/// The catalogue `readiness` values naming a subsystem that **does not exist
/// yet**, so not one of the domain's 25 guardrails can be closed against it.
///
/// Handover 25 established the list by refusing to record `PERF-Dnn-G11`
/// evidence for exactly these readinesses, on the grounds that the absence of a
/// heap in unwritten code is evidence about nothing. `LE-35` is the rule that
/// refusal implied and nobody wrote down; [`crate::assurance`] enforces it.
pub const UNIMPLEMENTED_READINESS: [&str; 4] = ["design", "stand-in-only", "specified", "unbuilt"];

/// Maps each `Dnn` to the `readiness` its catalogue rows declare.
///
/// A domain's 25 rows must agree with one another — readiness is a property of
/// the subsystem, not of an individual guardrail — so disagreement is an error
/// rather than a value this function picks a winner from.
pub fn domain_readiness(repo_root: &Path) -> Result<BTreeMap<String, String>, String> {
    let path = repo_root.join("goals").join("performance").join("catalogue.tsv");
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    readiness_from_contents(&contents)
}

fn readiness_from_contents(contents: &str) -> Result<BTreeMap<String, String>, String> {
    let mut readiness: BTreeMap<String, String> = BTreeMap::new();
    for (zero_based_index, raw_line) in contents.lines().skip(1).enumerate() {
        let line_number = zero_based_index + 2;
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != FIELD_COUNT {
            return Err(format!(
                "line {line_number}: expected {FIELD_COUNT} tab-separated fields, found {}",
                fields.len()
            ));
        }
        let domain = fields[1];
        let value = fields[6];
        match readiness.get(domain) {
            Some(existing) if existing != value => {
                return Err(format!(
                    "line {line_number}: `{domain}` declares readiness `{value}` here and \
                     `{existing}` on an earlier row; readiness is a property of the subsystem and \
                     must agree across all 25 of a domain's guardrails"
                ));
            }
            Some(_) => {}
            None => {
                readiness.insert(domain.to_string(), value.to_string());
            }
        }
    }
    Ok(readiness)
}

/// How many release gates are in play, and how many of them a board would move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleaseGateReach {
    /// Release gates in the domains at least one Story selects — the
    /// denominator the dashboard has always shown.
    pub in_play: usize,
    /// Of those, the ones whose tier names `Host` or `T0`: reachable with the
    /// hardware this project already has.
    pub reachable: usize,
    /// Of those, the ones naming only `T1`/`T2`: a board moves these and
    /// nothing else does.
    pub hardware_only: usize,
}

/// Splits the in-play release gates into what a board would move and what it
/// would not.
///
/// The distinction was computed by hand in
/// `session/hand-2026-07-28/41A-the-dashboard-as-a-work-order.md` and is
/// derived here instead, because a ratio that argues about where effort should
/// go is exactly the kind of number that must not be an assertion in a
/// document nobody re-checks (`LE-30`).
///
/// `G24`/`G25` are excluded by the `gate` column: they are *claim* gates
/// (Linux and RTOS comparisons) that run only after the absolute release gates
/// pass, so they are not part of the release denominator.
pub fn release_gate_reach(
    repo_root: &Path,
    in_play_domains: &BTreeSet<String>,
) -> Result<ReleaseGateReach, String> {
    let path = repo_root.join("goals").join("performance").join("catalogue.tsv");
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    reach_from_contents(&contents, in_play_domains)
}

fn reach_from_contents(
    contents: &str,
    in_play_domains: &BTreeSet<String>,
) -> Result<ReleaseGateReach, String> {
    let mut reach = ReleaseGateReach { in_play: 0, reachable: 0, hardware_only: 0 };
    for (zero_based_index, raw_line) in contents.lines().skip(1).enumerate() {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != FIELD_COUNT {
            return Err(format!(
                "line {}: expected {FIELD_COUNT} tab-separated fields, found {}",
                zero_based_index + 2,
                fields.len()
            ));
        }
        if !in_play_domains.contains(fields[1]) || fields[7] != "release" {
            continue;
        }
        reach.in_play += 1;
        let tier = fields[5];
        // `HIL` is deliberately *not* treated as reachable. The HIL rigs are
        // CAN/USB hardware-in-the-loop deferred to Phase 3 and this project has
        // none, so a `Host+T0+HIL` row is reachable on the strength of its
        // `Host`/`T0` half and a `T1+T2+HIL` row is not reachable at all.
        if tier.contains("Host") || tier.contains("T0") {
            reach.reachable += 1;
        } else {
            reach.hardware_only += 1;
        }
    }
    Ok(reach)
}

fn validate_axis_id(value: &str, prefix: char, line_number: usize) -> Result<(), String> {
    let bytes = value.as_bytes();
    if bytes.len() != 3 || bytes[0] != prefix as u8 || !bytes[1..].iter().all(u8::is_ascii_digit) {
        return Err(format!(
            "line {line_number}: `{value}` is not a valid {prefix}01..{prefix}25 axis id"
        ));
    }
    let number = value[1..]
        .parse::<usize>()
        .map_err(|error| format!("line {line_number}: invalid axis id `{value}`: {error}"))?;
    if !(1..=AXIS_SIZE).contains(&number) {
        return Err(format!("line {line_number}: `{value}` falls outside {prefix}01..{prefix}25"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn valid_row(domain: usize, guardrail: usize) -> String {
        format!(
            "PERF-D{domain:02}-G{guardrail:02}\tD{domain:02}\tG{guardrail:02}\ttitle\tP0\tT0\tprototype\trelease\tspecified\tmetric\ttarget\tsafety"
        )
    }

    fn complete_fixture() -> String {
        let mut fixture = String::from(EXPECTED_HEADER);
        fixture.push('\n');
        for domain in 1..=AXIS_SIZE {
            for guardrail in 1..=AXIS_SIZE {
                fixture.push_str(&valid_row(domain, guardrail));
                fixture.push('\n');
            }
        }
        fixture
    }

    #[test]
    fn complete_cross_product_passes() {
        let summary = validate_contents(&complete_fixture()).expect("complete fixture is valid");
        assert_eq!(summary.test_count, 625);
    }

    #[test]
    fn missing_cross_product_cell_fails() {
        let mut fixture = complete_fixture();
        let missing = format!("{}\n", valid_row(25, 25));
        fixture = fixture.replace(&missing, "");
        let error = validate_contents(&fixture).expect_err("missing cell must fail");
        assert!(error.contains("PERF-D25-G25"));
    }

    #[test]
    fn duplicate_id_fails() {
        let mut fixture = complete_fixture();
        fixture.push_str(&valid_row(1, 1));
        fixture.push('\n');
        let error = validate_contents(&fixture).expect_err("duplicate id must fail");
        assert!(error.contains("duplicate id"));
    }

    #[test]
    fn empty_required_field_fails() {
        let fixture = complete_fixture().replace("\tD01\tG01\ttitle\t", "\tD01\tG01\t\t");
        let error = validate_contents(&fixture).expect_err("empty title must fail");
        assert!(error.contains("every field must be non-empty"));
    }

    // `TEST-P0-01-07-A` clause 3: the readiness lookup the `LE-35` gate reads.
    #[test]
    fn readiness_disagreeing_across_one_domains_rows_fails() {
        let fixture = complete_fixture().replace(
            "PERF-D01-G02\tD01\tG02\ttitle\tP0\tT0\tprototype\t",
            "PERF-D01-G02\tD01\tG02\ttitle\tP0\tT0\tdesign\t",
        );
        let error = readiness_from_contents(&fixture).expect_err("disagreeing readiness must fail");
        assert!(error.contains("D01"), "{error}");
        assert!(error.contains("must agree"), "{error}");
    }

    #[test]
    fn committed_catalogue_readiness_matches_what_the_le_35_gate_assumes() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf();
        let readiness = domain_readiness(&repo_root).expect("committed catalogue readiness parses");
        assert_eq!(readiness.len(), AXIS_SIZE);
        // Named rather than counted: these are the exact domains Handover 25
        // refused to record `G11` for, and the gate is only as good as this
        // list staying honest.
        assert_eq!(readiness.get("D17").map(String::as_str), Some("design"));
        assert_eq!(readiness.get("D02").map(String::as_str), Some("unbuilt"));
        assert_eq!(readiness.get("D10").map(String::as_str), Some("stand-in-only"));
        assert_eq!(readiness.get("D12").map(String::as_str), Some("specified"));
        assert_eq!(readiness.get("D01").map(String::as_str), Some("prototype"));
    }

    // `TEST-P0-01-08-A` clause 1: the reachability split, checked against the
    // hand count in Handover 41A §2 — 391 in play, 345 reachable, 46 needing a
    // board. That document did the arithmetic by hand and asked for it to be
    // verified rather than trusted; this is the verification, and from here it
    // is derived rather than asserted.
    #[test]
    fn committed_catalogue_reachability_matches_the_hand_count_in_41a() {
        let in_play: BTreeSet<String> = [
            "D01", "D02", "D03", "D04", "D05", "D06", "D07", "D08", "D09", "D10", "D11", "D12",
            "D13", "D14", "D22", "D24", "D25",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        let reach =
            release_gate_reach(&repo_root(), &in_play).expect("committed catalogue is readable");
        assert_eq!(reach.in_play, 391, "17 in-play domains x 23 release guardrails");
        assert_eq!(reach.reachable, 345, "Host or T0 in tier");
        assert_eq!(reach.hardware_only, 46, "T1/T2 only — a board moves these and nothing else");
        assert_eq!(reach.reachable + reach.hardware_only, reach.in_play);
    }

    #[test]
    fn claim_gates_are_excluded_from_the_release_denominator() {
        let one = BTreeSet::from(["D01".to_string()]);
        let reach = release_gate_reach(&repo_root(), &one).expect("catalogue is readable");
        // 25 guardrails, of which G24 and G25 are `claim`.
        assert_eq!(reach.in_play, 23);
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf()
    }

    #[test]
    fn committed_catalogue_is_complete() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf();
        let summary = check_catalogue(&repo_root).expect("committed catalogue must be valid");
        assert_eq!(summary.test_count, 625);
    }
}
