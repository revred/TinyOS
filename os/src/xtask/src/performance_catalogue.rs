//! Integrity validation for the 625-test performance catalogue.

use std::collections::BTreeSet;
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
