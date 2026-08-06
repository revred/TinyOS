//! The `Status:` header of every Epic, Feature and Story, and the three checks
//! that hold a header to something outside itself.
//!
//! A well-formed header is the cheap half. The three expensive ones each caught
//! a real drift: `LE-44` (a Feature's Stories table disagreeing with the
//! Story's own header), `LE-65` (a Story reading `Specified` for four days
//! after its Report recorded Pass), and `06A` §4.2 — the same check in the
//! opposite direction, where a Story says every criterion is met *and* says it
//! is unfinished. Seven `EPIC-P1` Stories did exactly that on 2026-08-05.

use super::*;

/// The controlled vocabulary a `Status:` header may open with.
///
/// Ordered longest-first so that `Functionally Verified` is never truncated to
/// `Verified` by a prefix match.
pub(super) const STATUS_STATES: [&str; 6] = [
    "Functionally Verified",
    "Functionally complete",
    "In progress",
    "Specified",
    "Complete",
    "Verified",
];

/// One artifact's machine-readable state, extracted from its `Status:` header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactStatus {
    /// `EPIC-P1`, `FEAT-P1-04`, `STORY-P1-04-03`.
    pub id: String,
    /// One of [`STATUS_STATES`].
    pub state: String,
    /// Everything after the state — dates, tiers, Reports, caveats.
    pub detail: String,
}

/// Extracts the state from a `Status:` header line.
///
/// The header stays human-readable prose; only its opening is constrained. The
/// state runs from `**` to the first terminator, so
/// `Status: **Verified** (locally; CI run pending)` and
/// `Status: **Specified, not yet started. Gated on `LE-09`.**` both parse.
pub(super) fn parse_status_line(line: &str) -> Result<(String, String), String> {
    let body = line
        .strip_prefix("Status:")
        .ok_or_else(|| "status line must start with `Status:`".to_string())?
        .trim_start();
    let body = body
        .strip_prefix("**")
        .ok_or_else(|| "status line must open with a bold state, `Status: **...`".to_string())?;

    for state in STATUS_STATES {
        let Some(rest) = body.strip_prefix(state) else {
            continue;
        };
        // A state must be followed by a terminator, never by more word
        // characters — otherwise `Complete` would match `Completely rewritten`.
        let terminated = rest.is_empty()
            || rest.starts_with("**")
            || rest.starts_with(" —")
            || rest.starts_with(',')
            || rest.starts_with(" (")
            || rest.starts_with('.');
        if terminated {
            // The remainder is prose meant for a reader, so strip the bold
            // markers and the separator that joined it to the state.
            let detail = rest
                .trim()
                .trim_start_matches("**")
                .trim()
                .trim_start_matches(['—', ','])
                .trim()
                .trim_end_matches("**")
                .trim()
                .to_string();
            return Ok((state.to_string(), detail));
        }
    }

    Err(format!(
        "status must open with one of {}; found `{}`",
        STATUS_STATES.join(", "),
        body.chars().take(40).collect::<String>()
    ))
}

/// Reads and validates the `Status:` header of every Epic, Feature and Story.
///
/// The headers were previously free prose in fourteen distinct shapes, so the
/// dashboard had to be hand-maintained against seventy documents and drifted.
/// Constraining only the opening keeps the prose while making the state
/// queryable.
pub(super) fn validate_status_headers(repo_root: &Path) -> Result<Vec<ArtifactStatus>, String> {
    let mut statuses = Vec::new();
    for (directory, prefix) in [("epics", "EPIC-"), ("features", "FEAT-"), ("stories", "STORY-")] {
        let path = repo_root.join("goals").join(directory);
        let mut paths = Vec::new();
        collect_markdown(&path, &mut paths)?;
        paths.sort();
        for file in paths {
            let Some(stem) = file.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            // `backlog.md` carries a status line but is a table of undecomposed
            // rows rather than an artifact with an id, so it is not constrained.
            if !stem.starts_with(prefix) {
                continue;
            }
            let contents = fs::read_to_string(&file)
                .map_err(|error| format!("failed to read {}: {error}", file.display()))?;
            let line = contents
                .lines()
                .find(|line| line.starts_with("Status:"))
                .ok_or_else(|| format!("{stem}: no `Status:` header"))?;
            let (state, detail) =
                parse_status_line(line).map_err(|error| format!("{stem}: {error}"))?;
            statuses.push(ArtifactStatus { id: stem.to_string(), state, detail });
        }
    }
    if statuses.is_empty() {
        return Err("no Epic, Feature or Story documents found".to_string());
    }
    Ok(statuses)
}

/// Reads every artifact status for `list-status`.
pub fn artifact_statuses(repo_root: &Path) -> Result<Vec<ArtifactStatus>, String> {
    validate_status_headers(repo_root)
}

/// Extracts the Story id and status cell from one Feature Stories-table row.
///
/// Rows look like `| [`STORY-P1-07-01`](../stories/….md) | summary | status |`.
/// Anything that is not such a row — the header, the `|---|` rule, prose
/// containing a pipe — yields `None` rather than an error, because a Feature's
/// body is prose and only this one table shape is constrained.
pub(super) fn parse_feature_story_row(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if !line.starts_with("| [`STORY-") {
        return None;
    }
    let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
    if cells.len() < 3 {
        return None;
    }
    let id = cells[0].trim_start_matches("[`").split('`').next()?.to_string();
    Some((id, cells[cells.len() - 1].to_string()))
}

/// Every `criterion N` / `criteria N and M` number a status cell or header
/// mentions.
///
/// A set, not a list: `criteria 3 and 4` and `criteria 4 and 3` say the same
/// thing, and a check that disagreed about that would be noise rather than a
/// gate.
pub(super) fn criterion_numbers(text: &str) -> BTreeSet<u32> {
    let lowered = text.to_ascii_lowercase();
    let mut numbers = BTreeSet::new();
    let mut rest = lowered.as_str();
    while let Some(position) = rest.find("criteri") {
        rest = &rest[position..];
        let after_keyword = match (rest.strip_prefix("criteria"), rest.strip_prefix("criterion")) {
            (Some(tail), _) => tail,
            (None, Some(tail)) => tail,
            (None, None) => {
                rest = &rest["criteri".len()..];
                continue;
            }
        };
        // Numbers run until the first token that is neither a number nor one of
        // the words that join them, so `criteria 3 and 4 need a board` stops at
        // `need` rather than swallowing the sentence.
        for token in after_keyword.split(|c: char| c.is_whitespace() || c == ',') {
            let token = token.trim_matches(|c: char| !c.is_ascii_alphanumeric());
            if token.is_empty() || token == "and" {
                continue;
            }
            match token.parse::<u32>() {
                Ok(number) => {
                    numbers.insert(number);
                }
                Err(_) => break,
            }
        }
        rest = after_keyword;
    }
    numbers
}

/// Cross-checks every Feature Stories-table row against the referenced Story's
/// own `Status:` header — `LE-44`.
///
/// `check-assurance-spine` already validated all 84 status headers, and already
/// read every Feature document; what it never did was compare the two. So a
/// Feature and its Story could disagree about a Story's state indefinitely with
/// every gate green, which is exactly what happened twice: `FEAT-P1-07` said
/// `STORY-P1-07-01` needed a board for *"criteria 2 and 4"* where the Story said
/// 3 and 4 — understating it on precisely the criterion that produces `Q1`
/// qualification evidence — and `FEAT-P1-03` recorded `STORY-P1-03-02` as
/// `Verified` for four days while the Story's own header still read
/// `In progress`.
///
/// Two things are compared and nothing else. The **state word exactly**:
/// `Functionally Verified` and `Verified` are distinct states in this project's
/// vocabulary, one carrying assurance debt whose reader will not go looking for
/// it, so they do not satisfy each other. And the **criterion numbers as a
/// set**. Everything else in both cells stays free prose.
pub(super) fn validate_feature_story_tables(
    repo_root: &Path,
    statuses: &[ArtifactStatus],
) -> Result<usize, String> {
    let by_id: BTreeMap<&str, &ArtifactStatus> =
        statuses.iter().map(|status| (status.id.as_str(), status)).collect();

    let feature_dir = repo_root.join("goals").join("features");
    let mut paths = Vec::new();
    collect_markdown(&feature_dir, &mut paths)?;
    paths.sort();

    let mut checked = 0;
    for file in paths {
        let Some(stem) = file.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if !stem.starts_with("FEAT-") {
            continue;
        }
        let contents = fs::read_to_string(&file)
            .map_err(|error| format!("failed to read {}: {error}", file.display()))?;
        for line in contents.lines() {
            let Some((story_id, cell)) = parse_feature_story_row(line) else {
                continue;
            };
            let Some(status) = by_id.get(story_id.as_str()) else {
                return Err(format!(
                    "{stem}: Stories table names `{story_id}`, which has no Story document"
                ));
            };
            compare_feature_story_row(stem, &story_id, &cell, status)?;
            checked += 1;
        }
    }

    if checked == 0 {
        return Err("no Feature Stories-table rows found to cross-check".to_string());
    }
    Ok(checked)
}

/// One Feature-table row against one Story header — the comparison itself,
/// separated from the file walk so it can be driven directly by tests.
///
/// `TEST-P0-01-07-A` clause 2 is why: against the committed tree this function
/// returns `Ok` for all 59 rows, so a green run says nothing about whether it
/// can reject. The tests supply the disagreements the tree no longer has.
pub(super) fn compare_feature_story_row(
    feature: &str,
    story_id: &str,
    cell: &str,
    status: &ArtifactStatus,
) -> Result<(), String> {
    // Some tables bold the state and some do not; both are prose choices, so
    // the cell is normalised to the header shape rather than one of them being
    // declared wrong.
    let normalised = format!("Status: **{}", cell.trim_start_matches("**"));
    let cell_state = parse_status_line(&normalised)
        .map_err(|error| {
            format!(
                "{feature}: the Stories-table status for `{story_id}` (`{cell}`) does not open \
                 with a known state: {error}"
            )
        })?
        .0;
    if cell_state != status.state {
        return Err(format!(
            "{feature}: Stories table records `{story_id}` as `{cell_state}`, but that Story's \
             own `Status:` header says `{}`. The Story is authoritative about its own state \
             (LE-44)",
            status.state
        ));
    }

    let table_criteria = criterion_numbers(cell);
    let story_criteria = criterion_numbers(&status.detail);
    if table_criteria != story_criteria {
        return Err(format!(
            "{feature}: Stories table says `{story_id}` blocks on criteria {table_criteria:?}, \
             but that Story's own header says {story_criteria:?} (LE-44)"
        ));
    }
    Ok(())
}

/// `LE-65`: a `Specified` Story header cannot outlive the Story's own passing
/// Report.
///
/// `STORY-P0-01-08` read *Specified* for four days after `REPORT-2026-07-28-11`
/// recorded Pass on all five clauses, with every gate green — because every
/// gate compares sideways (badge to header, Feature table to header) and
/// nothing compared the header to the Story's own evidence.
///
/// `In progress` is deliberately **not** refused: `REPORT-2026-07-30-01`
/// records PASS while its `FEAT-P2` Stories stay `In progress` with their
/// performance numbers stated as open debt. A passing Report beside an
/// `In progress` header is honest partial delivery; a passing Report beside
/// "not started" is a contradiction whatever fraction it covers.
pub(super) fn validate_specified_headers_against_reports(
    report_dir: &Path,
    reports: &BTreeSet<String>,
    statuses: &[ArtifactStatus],
) -> Result<usize, String> {
    let state_by_story: BTreeMap<&str, &str> = statuses
        .iter()
        .filter(|status| status.id.starts_with("STORY-"))
        .map(|status| (status.id.as_str(), status.state.as_str()))
        .collect();

    let mut passing_reports = 0;
    for report in reports {
        let path = report_dir.join(format!("{report}.md"));
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if specified_story_refusal(report, &contents, &state_by_story)? {
            passing_reports += 1;
        }
    }
    Ok(passing_reports)
}

/// The `LE-65` decision for one Report, exposed for tests.
///
/// `Ok(true)` when the Report records a pass (and contradicts no `Specified`
/// header), `Ok(false)` when it carries no readable verdict.
pub(super) fn specified_story_refusal(
    report: &str,
    contents: &str,
    state_by_story: &BTreeMap<&str, &str>,
) -> Result<bool, String> {
    if report_result_verdict(contents).as_deref() != Some("pass") {
        return Ok(false);
    }
    let covered = contents
        .lines()
        .find(|line| line.contains("Test(s) covered:"))
        .map(covered_story_ids)
        .unwrap_or_default();
    for story in covered {
        if state_by_story.get(story.as_str()).copied() == Some("Specified") {
            return Err(format!(
                "`{story}`'s own `Status:` header says `Specified`, but `{report}` records \
                 a passing result for it. A filed passing Report contradicts \"not started\" \
                 outright: re-verify the Report's evidence against the current tree, then \
                 advance the header — verify, don't inherit (Handover 35's rule; LE-65)"
            ));
        }
    }
    Ok(true)
}

/// `LE-65`'s other half: a Story may not claim every criterion is met and
/// still call itself unfinished.
///
/// `STORY-P0-01-10` closed the direction where a `Specified` header outlives a
/// passing Report. This is the direction [`06A`](../../../../session/hand-2026-08-05/06A-nothing-is-verified-and-the-reason-is-not-velocity.md)
/// §4.2 named and left open, and the 2026-08-05 closing pass is the argument
/// for it: **seven `EPIC-P1` Stories carried headers that said, in their own
/// words, that every acceptance criterion was Green on silicon — and all seven
/// still read `In progress`.** Two of them had said so since 2026-08-03. The
/// register therefore showed 31 Stories in flight under an Epic whose Stories
/// had, by their own account, finished.
///
/// Nothing caught it because every existing gate compares *sideways* — badge to
/// header, Feature table to header, header to Report — and a document that
/// contradicts *itself* agrees with all of its neighbours. `06A` §4.1 had to be
/// executed by a human reading 31 documents; this exists so it does not have to
/// be a third time.
///
/// **The rule is deliberately narrow, because a gate that guesses at prose
/// teaches people to bypass gates.** It fires only when all three hold:
///
/// 1. the state is `In progress` or `Specified` — a state that asserts the work
///    is not finished;
/// 2. the detail contains one of a short list of **exact** all-criteria-met
///    claims, every one of them lifted verbatim from a real header this pass
///    found; and
/// 3. the detail names **no** outstanding gap.
///
/// Condition 3 is what makes this safe, and it is also `06A` §4.1's own
/// grammar: a Story that is not finished must name the one missing thing. A
/// header that says "every criterion Green **except** the board capture" names
/// its gap and is accepted unchanged. So the gate does not demand that Stories
/// advance — it demands that they be *one of the two permitted things*, and it
/// refuses only the third option `06A` §4.1 says does not exist.
pub(super) fn all_criteria_met_claim(detail: &str) -> Option<&'static str> {
    // Verbatim from headers found on 2026-08-05. Kept as fixed phrases rather
    // than a pattern: a looser rule would start deciding what English means,
    // and `07A` §2.4's grammar defect is the standing argument against that.
    const CLAIMS: [&str; 6] = [
        "every criterion green",
        "every acceptance criterion",
        "all criteria met",
        "all four acceptance criteria met",
        "all five acceptance criteria are now green",
        "all seven acceptance criteria met",
    ];
    let lowered = detail.to_ascii_lowercase();
    CLAIMS.into_iter().find(|claim| lowered.contains(claim))
}

/// Phrases by which a header names something still outstanding.
///
/// Presence of any one of these means the Story took `06A` §4.1's second
/// option — state the one missing thing — and the gate stands down. Broad on
/// purpose: a false *negative* here costs one unadvanced header that a reader
/// will still see, while a false *positive* costs a refused build on a document
/// that was honest.
pub(super) fn names_an_outstanding_gap(detail: &str) -> bool {
    const GAPS: [&str; 12] = [
        "not met",
        "unmet",
        "missing thing",
        "still owed",
        "awaits",
        "await ",
        "needs a board",
        "need a board",
        "needs the board",
        "blocked on",
        "no board evidence",
        "except",
    ];
    let lowered = detail.to_ascii_lowercase();
    GAPS.into_iter().any(|gap| lowered.contains(gap))
}

/// Applies the rule above to every Story header, reporting all offenders
/// together.
///
/// All together rather than the first, for `07A` §2.4's reason: a gate that
/// stops at one failure turns a closing pass into one push per Story.
pub(super) fn validate_unclaimed_satisfied_stories(
    statuses: &[ArtifactStatus],
) -> Result<usize, String> {
    let mut refused = Vec::new();
    let mut checked = 0;
    for status in statuses.iter().filter(|status| status.id.starts_with("STORY-")) {
        if status.state != "In progress" && status.state != "Specified" {
            continue;
        }
        checked += 1;
        let Some(claim) = all_criteria_met_claim(&status.detail) else {
            continue;
        };
        if names_an_outstanding_gap(&status.detail) {
            continue;
        }
        refused.push(format!("  `{}` says `{}` but reads `{}`", status.id, claim, status.state));
    }

    if refused.is_empty() {
        return Ok(checked);
    }
    Err(format!(
        "a Story that says every criterion is met may not also say it is unfinished \
         (LE-65's other half, 06A §4.2):\n{}\n\
         There are exactly two honest options and no third: advance the header to \
         `Verified`, citing the Report or BOARD VERDICT — or write the ONE missing \
         thing into the header as a sentence. \"Still in progress\" without naming \
         the missing item is not an option (06A §4.1). If a gap exists, say what it \
         is and this gate stands down.",
        refused.join("\n")
    ))
}

#[cfg(test)]
mod le65_second_half_tests {
    use super::*;

    fn story(id: &str, state: &str, detail: &str) -> ArtifactStatus {
        ArtifactStatus { id: id.to_string(), state: state.to_string(), detail: detail.to_string() }
    }

    /// The incident, verbatim. `STORY-P1-09-07`'s header as it stood from
    /// 2026-08-03 until the 2026-08-05 closing pass: it says every criterion is
    /// Green and it says `In progress`, in the same sentence.
    #[test]
    fn the_2026_08_03_headers_that_said_green_and_in_progress_are_refused() {
        let statuses = vec![story(
            "STORY-P1-09-07",
            "In progress",
            "every criterion Green 2026-08-03: the count was taken through the case seam the \
             same evening it was written, and it read 3. Not Verified pending the assurance pass.",
        )];
        let error = validate_unclaimed_satisfied_stories(&statuses)
            .expect_err("a Story cannot be finished and unfinished at once");
        assert!(error.contains("STORY-P1-09-07"), "{error}");
        assert!(error.contains("no third"), "the fix direction is named: {error}");
    }

    /// Every one of the seven the closing pass found, so this test fails if the
    /// claim list stops recognising any of them.
    #[test]
    fn all_seven_of_the_2026_08_05_offenders_are_caught() {
        let offenders = [
            ("STORY-P1-07-08", "every criterion Green 2026-08-03: the lamp pulsed."),
            ("STORY-P1-07-09", "every criterion Green 2026-08-03: the canvas painted."),
            ("STORY-P1-09-07", "every criterion Green 2026-08-03: it read 3."),
            ("STORY-P1-09-12", "every criterion Green on silicon: both arms ran."),
            ("STORY-P1-09-13", "every criterion Green on silicon 2026-08-04."),
            ("STORY-P1-09-14", "every criterion Green on silicon 2026-08-04."),
            ("STORY-P1-09-15", "every criterion Green on silicon 2026-08-04 ~02:07."),
        ];
        for (id, detail) in offenders {
            let statuses = vec![story(id, "In progress", detail)];
            let error = validate_unclaimed_satisfied_stories(&statuses)
                .expect_err("an all-met claim under `In progress` must be refused");
            assert!(error.contains(id), "{id} missing from the refusal: {error}");
        }
    }

    /// `STORY-P1-07-03`'s pre-pass header used a different wording for the same
    /// claim, and a list that only knew one phrasing would have missed it.
    #[test]
    fn the_other_phrasing_of_the_same_claim_is_caught_too() {
        let statuses = vec![story(
            "STORY-P1-07-03",
            "In progress",
            "Every acceptance criterion of this Story now has silicon evidence. \
             The evidence channel is the canvas.",
        )];
        validate_unclaimed_satisfied_stories(&statuses)
            .expect_err("a differently-worded all-met claim is the same contradiction");
    }

    /// **The clause that makes this safe.** A header that claims everything is
    /// Green *and* names what is still outstanding has taken `06A` §4.1's
    /// second option, and must pass untouched — otherwise the gate would punish
    /// exactly the honesty it exists to produce.
    #[test]
    fn a_header_that_names_its_gap_is_accepted() {
        let honest = [
            "every criterion Green on silicon except criterion 4, which awaits a board capture",
            "every acceptance criterion Green on the host; criterion 7 is not met and no board \
             has emitted one",
            "every criterion Green 2026-08-03 — still owed: the Report amendment",
            "every acceptance criterion met bar the cable-out capture, which needs the board",
        ];
        for detail in honest {
            let statuses = vec![story("STORY-P1-09-01", "In progress", detail)];
            let checked = validate_unclaimed_satisfied_stories(&statuses)
                .unwrap_or_else(|e| panic!("naming a gap must be accepted: {detail}\n{e}"));
            assert_eq!(checked, 1, "the Story is still counted as checked");
        }
    }

    /// An advanced Story is not this gate's business whatever its prose says.
    #[test]
    fn an_advanced_header_is_not_examined() {
        let statuses = vec![story(
            "STORY-P1-09-07",
            "Verified",
            "every criterion Green 2026-08-03, and nothing outstanding.",
        )];
        let checked = validate_unclaimed_satisfied_stories(&statuses)
            .expect("a Verified Story makes no contradictory claim");
        assert_eq!(checked, 0, "only unfinished states are examined");
    }

    /// A Story making no all-met claim is silent to this gate — the ordinary
    /// case, and the reason a green run says little on its own.
    #[test]
    fn an_ordinary_in_progress_header_passes() {
        let statuses = vec![story(
            "STORY-P1-10-05",
            "In progress",
            "implemented 2026-08-05 after LE-75; host-Green. The sensing half only.",
        )];
        validate_unclaimed_satisfied_stories(&statuses).expect("no claim, no contradiction");
    }

    /// Every offender is reported in one run, not the first alone — `07A` §2.4's
    /// rule, which exists so a closing pass is one read rather than one push
    /// per Story.
    #[test]
    fn every_offender_is_reported_together() {
        let statuses = vec![
            story("STORY-P1-09-12", "In progress", "every criterion Green on silicon."),
            story("STORY-P1-09-13", "In progress", "every criterion Green on silicon."),
            story("STORY-P1-09-14", "In progress", "every criterion Green on silicon."),
        ];
        let error = validate_unclaimed_satisfied_stories(&statuses).expect_err("three offenders");
        for id in ["STORY-P1-09-12", "STORY-P1-09-13", "STORY-P1-09-14"] {
            assert!(error.contains(id), "{id} missing from the combined report: {error}");
        }
    }

    /// The committed tree must satisfy its own gate. This is the assertion that
    /// would have been red before the 2026-08-05 closing pass and is green
    /// after it — the closing pass and this gate check each other.
    #[test]
    fn the_committed_tree_has_no_unclaimed_satisfied_story() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf();
        let statuses =
            validate_status_headers(&repo_root).expect("every committed Status: must parse");
        validate_unclaimed_satisfied_stories(&statuses)
            .expect("no committed Story may claim every criterion met and still read unfinished");
    }
}

#[cfg(test)]
mod le65_tests {
    use super::*;

    fn report(covered: &str, result_opener: &str) -> String {
        format!(
            "# REPORT-2026-07-28-11 — fixture\n\n\
             **Test(s) covered:** {covered}\n\n## Result\n\n{result_opener}, all five clauses.\n"
        )
    }

    fn states<'a>(pairs: &[(&'a str, &'a str)]) -> BTreeMap<&'a str, &'a str> {
        pairs.iter().copied().collect()
    }

    /// The `-08` incident's exact shape: a filed Report recording Pass above a
    /// header still reading Specified, four days along.
    #[test]
    fn a_specified_header_with_a_passing_report_is_refused() {
        let contents = report("`TEST-P0-01-08-A` (`STORY-P0-01-08` — the dashboard)", "**Pass**");
        let error = specified_story_refusal(
            "REPORT-2026-07-28-11",
            &contents,
            &states(&[("STORY-P0-01-08", "Specified")]),
        )
        .expect_err("the -08 incident must be refused");
        assert!(error.contains("STORY-P0-01-08"), "{error}");
        assert!(error.contains("verify, don't inherit"), "the fix direction is named: {error}");
    }

    /// Coverage through the Test id alone refuses the same — a Report need not
    /// name the Story directly to contradict its header.
    #[test]
    fn coverage_through_the_test_id_is_the_same_claim() {
        let contents = report("`TEST-P0-01-08-A`", "**Pass**");
        specified_story_refusal(
            "REPORT-2026-07-28-11",
            &contents,
            &states(&[("STORY-P0-01-08", "Specified")]),
        )
        .expect_err("a Test id resolves to its Story");
    }

    /// The acceptance case beside the refusal: the corrected header.
    #[test]
    fn the_same_report_beside_an_advanced_header_is_accepted() {
        let contents = report("`TEST-P0-01-08-A` (`STORY-P0-01-08`)", "**Pass**");
        let passing = specified_story_refusal(
            "REPORT-2026-07-28-11",
            &contents,
            &states(&[("STORY-P0-01-08", "Functionally Verified")]),
        )
        .expect("an advanced header agrees with its Report");
        assert!(passing, "the Report still counts as cross-checked");
    }

    /// The `REPORT-2026-07-30-01` precedent: In progress beside a passing
    /// Report is honest partial delivery and is deliberately not refused.
    #[test]
    fn in_progress_beside_a_passing_report_is_deliberately_accepted() {
        let contents = report("`STORY-P2-04-01`", "**PASS on all four**");
        specified_story_refusal(
            "REPORT-2026-07-30-01",
            &contents,
            &states(&[("STORY-P2-04-01", "In progress")]),
        )
        .expect("stated-debt partial delivery is not a contradiction");
    }

    /// A Report with no readable verdict — the 2026-07-26 generation — triggers
    /// nothing, even above a Specified header.
    #[test]
    fn a_resultless_report_extracts_no_verdict_and_refuses_nothing() {
        let contents = "# REPORT — fixture\n\n**Test(s) covered:** `STORY-P0-01-08`\n\nProse only.";
        let passing = specified_story_refusal(
            "REPORT-2026-07-26-01",
            contents,
            &states(&[("STORY-P0-01-08", "Specified")]),
        )
        .expect("no verdict, no refusal");
        assert!(!passing, "a resultless Report is not cross-checked");
    }

    /// An unbolded or non-pass opener is not a verdict.
    #[test]
    fn only_a_bolded_pass_opener_is_a_verdict() {
        assert_eq!(
            report_result_verdict("## Result\n\n**Pass**, all clauses").as_deref(),
            Some("pass")
        );
        assert_eq!(
            report_result_verdict("## Result\n\n**PASS on all four**").as_deref(),
            Some("pass")
        );
        assert_eq!(
            report_result_verdict("## Result\n\n**Blocked** on the adapter").as_deref(),
            Some("blocked"),
            "other verdicts are read, they just never match pass"
        );
        assert_eq!(report_result_verdict("## Result\n\nPass, but unbolded"), None);
        assert_eq!(report_result_verdict("## Something else\n\n**Pass**"), None);
    }

    /// Both spellings of coverage resolve to Story ids; the trailing Test
    /// letter is stripped, everything else is not a Test id.
    #[test]
    fn covered_story_ids_resolve_direct_and_test_spellings() {
        let ids = covered_story_ids(
            "**Test(s) covered:** `TEST-P0-01-08-A` (`STORY-P0-01-09` — sibling), TEST-P1_5-01-01-B",
        );
        assert_eq!(
            ids,
            BTreeSet::from([
                "STORY-P0-01-08".to_string(),
                "STORY-P0-01-09".to_string(),
                "STORY-P1_5-01-01".to_string(),
            ])
        );
    }
}
