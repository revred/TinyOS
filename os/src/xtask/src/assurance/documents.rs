//! Test and Report documents, and their join back to the Stories they cover.
//!
//! The join runs both ways: a Test must name a Story that exists, and a Story
//! reachable from a Report's `Covers:` line must exist too. A Report's verdict
//! is parsed here rather than read by eye, because it is what
//! [`super::status`] cross-checks the covered Story's header against.

use super::*;

pub(super) fn validate_test_coverage(
    test_dir: &Path,
    tests: &BTreeSet<String>,
    contracts: &ContractIndex,
    feature_contracts: &FeatureContractIndex,
) -> Result<(), String> {
    for test in tests {
        let parts: Vec<&str> = test.split('-').collect();
        if parts.len() != 5
            || parts[0] != "TEST"
            || !is_phase_id(parts[1])
            || !is_two_digits(parts[2])
            || !is_two_digits(parts[3])
            || parts[4].len() != 1
            || !parts[4].bytes().all(|byte| byte.is_ascii_uppercase())
        {
            return Err(format!("`{test}` is not a valid TEST-Pn-NN-NN-A id"));
        }
        let story = format!("STORY-{}-{}-{}", parts[1], parts[2], parts[3]);
        let Some(contract) = contracts.details_by_story.get(&story) else {
            return Err(format!("Test `{test}` has no mapped Story `{story}`"));
        };
        let path = test_dir.join(format!("{test}.md"));
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if !contents.lines().any(|line| {
            line.starts_with("Assurance contract:")
                && line.contains("goals/assurance/story-contracts.tsv")
        }) {
            return Err(format!(
                "Test `{test}` has no `Assurance contract:` link to story-contracts.tsv"
            ));
        }
        validate_test_metadata(
            test,
            &contents,
            "Performance domains:",
            &contract.performance_domains,
        )?;
        validate_test_metadata(test, &contents, "Security controls:", &contract.security_controls)?;
        validate_test_metadata(
            test,
            &contents,
            "Containment classes:",
            &contract.containment_classes,
        )?;
        let expected_boundary_tests = feature_contracts
            .boundary_tests_by_feature
            .get(&contract.feature_id)
            .ok_or_else(|| {
                format!(
                    "Test `{test}`'s Feature `{}` has no boundary-test contract",
                    contract.feature_id
                )
            })?;
        validate_test_metadata(test, &contents, "Boundary tests:", expected_boundary_tests)?;
        let expected_protection_domains = feature_contracts
            .protection_domains_by_feature
            .get(&contract.feature_id)
            .ok_or_else(|| {
                format!(
                    "Test `{test}`'s Feature `{}` has no Protection Domain contract",
                    contract.feature_id
                )
            })?;
        validate_test_metadata(
            test,
            &contents,
            "Protection Domain contracts:",
            expected_protection_domains,
        )?;
        let expected_code_admission = feature_contracts
            .code_admission_by_feature
            .get(&contract.feature_id)
            .ok_or_else(|| {
                format!(
                    "Test `{test}`'s Feature `{}` has no code-admission contract",
                    contract.feature_id
                )
            })?;
        validate_test_metadata(test, &contents, "Code admission gates:", expected_code_admission)?;
        validate_test_metadata(
            test,
            &contents,
            "Assurance state:",
            &BTreeSet::from([contract.state.clone()]),
        )?;
    }
    Ok(())
}

pub(super) fn validate_test_metadata(
    test: &str,
    contents: &str,
    field: &str,
    expected: &BTreeSet<String>,
) -> Result<(), String> {
    let line = contents
        .lines()
        .find(|line| line.starts_with(field))
        .ok_or_else(|| format!("Test `{test}` has no `{field}` field"))?;
    let mut actual = BTreeSet::new();
    for (index, value) in line.split('`').enumerate() {
        if index % 2 == 1 && !value.is_empty() && !actual.insert(value.to_string()) {
            return Err(format!("Test `{test}` repeats `{value}` in its `{field}` field"));
        }
    }
    if &actual != expected {
        return Err(format!(
            "Test `{test}` `{field}` metadata does not match its assurance contract; expected {}, found {}",
            join_owned_ids(expected),
            join_owned_ids(&actual)
        ));
    }
    Ok(())
}

pub(super) fn validate_report_coverage(
    report_dir: &Path,
    reports: &BTreeSet<String>,
    tests: &BTreeSet<String>,
    stories: &BTreeSet<String>,
) -> Result<(), String> {
    for report in reports {
        let path = report_dir.join(format!("{report}.md"));
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let covered = contents
            .lines()
            .find(|line| line.contains("Test(s) covered:"))
            .ok_or_else(|| format!("Report `{report}` has no `Test(s) covered:` field"))?;
        let names_a_test = tests.iter().any(|test| covered.contains(test));
        let names_a_story = stories.iter().any(|story| covered.contains(story));
        if !names_a_test && !names_a_story {
            return Err(format!(
                "Report `{report}`'s `Test(s) covered:` field references no mapped Story or Test"
            ));
        }
    }
    Ok(())
}

/// The Story ids a Report's `Test(s) covered:` line claims — named directly,
/// or through a covered Test's id (`TEST-P0-01-08-A` covers `STORY-P0-01-08`).
pub(super) fn covered_story_ids(covered_line: &str) -> BTreeSet<String> {
    let mut stories = BTreeSet::new();
    for (position, _) in
        covered_line.match_indices("STORY-").chain(covered_line.match_indices("TEST-"))
    {
        let token: String = covered_line[position..]
            .chars()
            .take_while(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
            .collect();
        if let Some(rest) = token.strip_prefix("TEST-") {
            // A Test id is its Story id plus a trailing letter.
            if let Some((story_part, suffix)) = rest.rsplit_once('-') {
                if suffix.len() == 1 && suffix.chars().all(|c| c.is_ascii_alphabetic()) {
                    stories.insert(format!("STORY-{story_part}"));
                }
            }
        } else {
            stories.insert(token);
        }
    }
    stories
}

/// The Report's verdict: the first bolded token after its `## Result` heading,
/// lowercased — `Some("pass")` for `**Pass**, all five clauses`.
///
/// Deliberately narrow (`LE-65`): a Report with no `## Result` section — the
/// 2026-07-26 generation — or whose Result opens with anything unbolded
/// extracts no verdict and triggers nothing. A gate that guessed at prose
/// would produce false refusals, and false refusals teach people to bypass
/// gates.
pub(super) fn report_result_verdict(contents: &str) -> Option<String> {
    let mut in_result = false;
    for line in contents.lines() {
        if line.trim_start().starts_with("## ") {
            in_result = line.trim() == "## Result";
            continue;
        }
        if !in_result || line.trim().is_empty() {
            continue;
        }
        let opener = line.trim().strip_prefix("**")?;
        let verdict: String =
            opener.chars().take_while(|character| character.is_ascii_alphabetic()).collect();
        return if verdict.is_empty() { None } else { Some(verdict.to_ascii_lowercase()) };
    }
    None
}
