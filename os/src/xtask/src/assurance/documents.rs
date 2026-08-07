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

/// The Epic document a Feature belongs to: `FEAT-P1-11` → `EPIC-P1`.
///
/// Derived rather than declared, because the id already carries the phase and
/// a second mapping is a second thing to keep in step (`LE-80`'s family).
pub(super) fn epic_of_feature(feature: &str) -> Option<String> {
    let rest = feature.strip_prefix("FEAT-")?;
    let phase = rest.split('-').next()?;
    (!phase.is_empty()).then(|| format!("EPIC-{phase}"))
}

/// `LE-107`: every Feature is named in the prose of the Epic that owns it.
///
/// **The register was right and the front door was wrong.** `EPIC-P1`'s
/// Features table jumped `FEAT-P1-10` straight to `FEAT-P1-12` and its
/// `Status:` header stopped enumerating at `FEAT-P1-10`, while
/// `feature-contracts.tsv`, the Feature documents and every spine count were
/// correct throughout — for four handovers, in the one document a new reader
/// opens first. Status headers are machine-checked for grammar, for agreement
/// with the rows beneath them and for citations resolving; **nothing asked
/// whether an Epic names its own Features.**
///
/// Two properties this check needs, both learned from gates already in this
/// tree:
///
/// - **It scans the whole document, never a named section.** The drift
///   appeared in two different places — a table row and a prose header — and a
///   section-scoped check would have found neither.
/// - **An Epic with no Features fails rather than passes.** A check that can
///   be satisfied by finding nothing reports on a prefix of its subject while
///   looking like it covered all of it (`LE-77`).
///
/// # What it cannot see, found by mutating the real file rather than a fixture
///
/// **It asserts the Feature is named *somewhere*, not that it is named
/// everywhere it should be.** The 2026-08-06 drift had two halves and this
/// catches only the stronger one:
///
/// - `FEAT-P1-11` was absent from the Features table **and** the `Status:`
///   header — **caught**, and verified by deleting every mention from
///   `EPIC-P1.md` itself and reading the error.
/// - `FEAT-P1-12` was in the table and missing from the header — **not
///   caught**, because one mention satisfies this check.
///
/// Asserting per-location would mean parsing an Epic's prose for which
/// sentences enumerate Features, and a header that legitimately stops
/// enumerating (as most Epics' do) would then have to be distinguished from one
/// that drifted — a judgement, not a scan. So the honest scope is *mentioned at
/// all*, which is where the four-handover drift lived, and the error message
/// names the second half explicitly so a reader fixing one does not leave the
/// other. Stated here because a gate believed to cover more than it does is
/// worse than no gate.
/// Phrases that turn a loose-end citation into a **live dependency claim**
/// rather than a historical reference (`LE-112`).
///
/// The distinction is the whole check, and getting it wrong in either
/// direction is worse than not having it. `EPIC-P1`'s header says *"the
/// hardware-tier Feature that **closes** `LE-09`"* — `LE-09` is closed, which
/// makes that sentence **correct**. `EPIC-P2`'s says *"**Blocked on** a storage
/// decision (`LE-48`)"* — also closed, and that sentence is **wrong**. A rule
/// reading "every cited row must be open" would flag the correct one and get
/// itself switched off, which is the failure mode this tree keeps re-learning.
const BLOCKING_CLAIMS: [&str; 5] =
    ["blocked on", "blocks on", "gated on", "waiting on", "depends on"];

/// Phrases that turn a blocking sentence into a **historical** one.
///
/// Found by fixing the two defects this check surfaced: every honest
/// correction says what the Epic *was* blocked on and that the row has since
/// closed, so the corrected header still contains a blocking phrase beside the
/// id. **Past tense reads exactly like present tense to a text scan**, and
/// contorting the prose to dodge the needle would be writing for the gate
/// rather than for the reader.
///
/// So a citation is a live blocking claim only when its clause makes one and
/// does **not** acknowledge the closure. The check is therefore *"claims to be
/// blocked and does not know the row closed"*, which is the actual defect —
/// `EPIC-P2` did not know for nine days.
const CLOSURE_ACKNOWLEDGED: [&str; 4] = ["closed", "no longer", "was recorded", "since taken"];

/// `LE-112`: a document claiming to be blocked on a loose end must cite an
/// **open** one.
///
/// `EPIC-P2` declared itself *"Blocked on a storage decision (`LE-48`)"* for
/// **nine days** after `LE-48` closed — so every session triaging what to work
/// on read the operator command environment as gated on a decision that had
/// already been taken, and the feasibility report read that state straight out
/// of the header. `Status:` headers are machine-checked for grammar, for
/// agreement with the rows beneath them, and for citations resolving to real
/// documents; **nothing read a loose-end id out of one and asked whether that
/// row was still open.**
fn blocking_citations(header: &str) -> Vec<String> {
    let lowered = header.to_ascii_lowercase();
    let mut found = Vec::new();
    for token in loose_end_tokens(header) {
        // The claim has to be near the citation, not merely somewhere in a
        // header that may run for a paragraph. The clause is delimited by the
        // sentence the id sits in.
        let Some(at) = lowered.find(&token.to_ascii_lowercase()) else { continue };
        let clause_start = lowered[..at].rfind(['.', ';']).map_or(0, |index| index + 1);
        // The clause runs to the end of the sentence, not merely up to the id:
        // an acknowledgement of closure usually follows the citation rather
        // than preceding it ("blocked on `LE-48`, which closed on ...").
        let clause_end = lowered[at..].find(['.', ';']).map_or(lowered.len(), |i| at + i);
        let clause = &lowered[clause_start..clause_end];
        let claims_block = BLOCKING_CLAIMS.iter().any(|claim| clause.contains(claim));
        let knows_it_closed = CLOSURE_ACKNOWLEDGED.iter().any(|note| clause.contains(note));
        if claims_block && !knows_it_closed {
            found.push(token);
        }
    }
    found
}

pub(super) fn validate_epics_enumerate_their_features(
    epic_dir: &Path,
    features: &BTreeSet<String>,
    open_loose_ends: &BTreeSet<String>,
) -> Result<usize, String> {
    let mut by_epic: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for feature in features {
        let epic = epic_of_feature(feature)
            .ok_or_else(|| format!("`{feature}` is not a well-formed Feature id"))?;
        by_epic.entry(epic).or_default().insert(feature.clone());
    }

    let mut missing: Vec<String> = Vec::new();
    let mut stale_blockers: Vec<String> = Vec::new();
    for (epic, owned) in &by_epic {
        if owned.is_empty() {
            return Err(format!(
                "`{epic}` has no Features; an Epic that owns nothing would satisfy this check \
                 vacuously (`LE-77`)"
            ));
        }
        let path = epic_dir.join(format!("{epic}.md"));
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        for feature in owned {
            if !contents.contains(feature.as_str()) {
                missing.push(format!(
                    "`{epic}` does not name `{feature}` anywhere in its document, but \
                     feature-contracts.tsv says it owns it"
                ));
            }
        }

        // `LE-112`, in the same pass over the same file: the two questions are
        // about the same line, so they read it once.
        let header = contents
            .lines()
            .find(|line| line.starts_with("Status:"))
            .ok_or_else(|| format!("`{epic}` has no `Status:` header"))?;
        // Anti-vacuity: a header that mentions `LE-` must yield at least one
        // id, or a parse that silently found nothing would pass exactly like a
        // header with nothing to find (`LE-77`).
        if header.contains("LE-") && loose_end_tokens(header).is_empty() {
            return Err(format!(
                "`{epic}`'s `Status:` header contains `LE-` but no id parsed out of it; the \
                 extractor found nothing where there is something, which is indistinguishable \
                 from a header with no citation and must not pass as one"
            ));
        }
        for cited in blocking_citations(header) {
            if !open_loose_ends.contains(&cited) {
                stale_blockers.push(format!(
                    "`{epic}`'s `Status:` header declares itself blocked on `{cited}`, which is \
                     CLOSED. Every session triaging what to work on reads that header, so a \
                     stale blocker is a live mis-direction rather than a stale sentence \
                     (`LE-112`)"
                ));
            }
        }
    }

    // Two defect classes, reported separately. They share a pass over the file
    // because they are two questions about one document, but a reader fixing
    // one must not be handed the other's instruction.
    if !missing.is_empty() {
        return Err(format!(
            "{} Feature(s) missing from their Epic's own prose. The registers can be right while \
             the document a reader starts from is wrong, and until 2026-08-07 nothing checked it \
             (`LE-107`):\n  {}\n    fix: add the Feature to the Epic's Features table AND to its \
             `Status:` header if that header enumerates Features — the 2026-08-06 drift was in \
             both places and correcting only one leaves the other wrong.",
            missing.len(),
            missing.join("\n  ")
        ));
    }
    if !stale_blockers.is_empty() {
        return Err(format!(
            "{} Epic(s) declare themselves blocked on a CLOSED loose end (`LE-112`):\n  {}\n    \
             fix: correct the clause. What replaces it is a judgement about what the Epic now \
             wants, which belongs to whoever owns it — but the *factual* half is not a \
             judgement: the row closed, and saying so is recording history rather than deciding \
             a direction.",
            stale_blockers.len(),
            stale_blockers.join("\n  ")
        ));
    }

    Ok(by_epic.len())
}
