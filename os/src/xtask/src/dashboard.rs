//! `LE-30` — the dashboard stops being hand-maintained.
//!
//! [`goals/index.html`](../../../../goals/index.html) is the page a reader
//! meets first, and every number on it was copied there by hand from
//! `check-assurance-spine`'s output. It drifted three times in one day when the
//! row was raised, and **nine consecutive sessions have now paid the
//! hand-sync price** rather than the one-Story price. Handover 41A re-synced it
//! and watched two of its own figures go stale *while the sync was being
//! written* — Reports 46 → 47 and loose ends 44 → 46.
//!
//! There are two kinds of content on that page and they need different
//! treatment, so this module does two different things:
//!
//! 1. **The stat tiles are generated.** They are pure spine arithmetic with no
//!    editorial content, so they are emitted between markers and byte-compared.
//!    `cargo run -p xtask -- emit-dashboard` prints the correct block.
//! 2. **The prose is gated, not generated.** The page's argument is written by
//!    people and must stay that way, so this module extracts only the
//!    *claims* — the spine-count sentence, and every Story's status badge —
//!    and refuses the ones that disagree with the spine.
//!
//! The badge check is `LE-44`'s rule one document along, and it found the same
//! defect class on first contact: **seven badges read `VERIFIED` for Stories
//! whose own header says `Functionally Verified`**, which is a weaker state
//! carrying assurance debt a reader of the stronger word would not go looking
//! for. One of the seven was written by the session that built the `LE-44`
//! gate, which is the argument for the machine rather than against it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::assurance::ArtifactStatus;

/// Where the generated block starts and ends inside `goals/index.html`.
const BEGIN_MARKER: &str =
    "<!-- BEGIN GENERATED stat-row: cargo run -p xtask -- emit-dashboard -->";
const END_MARKER: &str = "<!-- END GENERATED stat-row -->";

/// Where the generated Overall-progress block starts and ends (`STORY-P0-01-09`).
///
/// A second region rather than an extension of the first: the two blocks sit in
/// different sections of the page and must be pasteable independently.
const PROGRESS_BEGIN_MARKER: &str =
    "<!-- BEGIN GENERATED overall-progress: cargo run -p xtask -- emit-dashboard -->";
const PROGRESS_END_MARKER: &str = "<!-- END GENERATED overall-progress -->";

/// The spine numbers the dashboard renders.
///
/// Every field is computed by `check-assurance-spine` on the run that builds
/// this, so the page cannot disagree with the checker that produced it — which
/// is the entire failure mode `LE-30` describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardFacts {
    /// Performance catalogue cells (625).
    pub catalogue_cells: usize,
    /// Containment classes.
    pub containment_classes: usize,
    /// Cross-class boundary tests.
    pub boundary_tests: usize,
    /// Protection Domain invariants.
    pub protection_domains: usize,
    /// Remote-code admission gates.
    pub code_admission_gates: usize,
    /// C0–C4 communication paths.
    pub class_paths: usize,
    /// Stories with a contract row.
    pub stories: usize,
    /// Security controls.
    pub security_controls: usize,
    /// Application/platform targets.
    pub application_targets: usize,
    /// Whole-system landing zones.
    pub landing_zones: usize,
    /// Stories whose assurance state is `verified`.
    pub assurance_verified: usize,
    /// Release gates carrying dated evidence.
    pub evidenced_gates: usize,
    /// Release gates in play.
    pub in_play_gates: usize,
    /// Of those, the ones reachable with no board.
    pub reachable_gates: usize,
    /// Of those, the ones that can actually be closed: in a domain whose
    /// subsystem exists, and not barred by `G04`.
    ///
    /// `LE-84`'s finding, published rather than left in a subcommand. The raw
    /// `evidenced/in_play` ratio invites *"5% done, 435 to go"*, which is
    /// wrong in both directions at once — it overstates the work remaining,
    /// because half the denominator cannot be closed by construction, and it
    /// understates the indictment, because against the denominator that CAN be
    /// closed the figure is far worse.
    pub closable_gates: usize,
    /// Of the closable gates carrying nothing, the ones blocked by neither an
    /// absent mechanism nor anything else: measurement work available today,
    /// on a laptop, with no decision pending.
    pub measurable_today: usize,
    /// Stories belonging to `EPIC-P0` or `EPIC-P1`.
    ///
    /// `LE-108`. The prose paragraph states this figure and the one below, and
    /// until 2026-08-07 nothing checked either: the sentence read *"Of the 52
    /// `EPIC-P0`/`EPIC-P1` Stories, 43 are Verified or Functionally
    /// Verified"* while the tree held 85 and 72. Thirty-three Stories of drift
    /// in a sentence sitting two lines below counts that ARE gated, which is
    /// what makes it worth gating rather than merely correcting.
    pub p0p1_stories: usize,
    /// Of those, the ones whose own `Status:` header reads `Verified` or
    /// `Functionally Verified`.
    pub p0p1_settled: usize,
    /// Platforms in the qualification register.
    pub platforms: usize,
    /// Of those, platforms holding a secure-world qualification record.
    ///
    /// Published beside [`Self::assurance_verified`] because it is the reason
    /// that number is what it is: every in-play domain carries one `G04`
    /// bound-class gate, `ADR 0005` bars `G04` while this count is zero, and
    /// every Story selects at least one domain. Assurance `verified` is
    /// therefore unreachable for every Story in the project until this moves —
    /// a locked door, not a backlog, and the dashboard said nothing about it.
    pub qualified_platforms: usize,
    /// Features with a containment contract.
    pub features: usize,
    /// Test documents.
    pub tests: usize,
    /// Report documents.
    pub reports: usize,
    /// Loose-end rows, closed and open together.
    pub loose_ends: usize,
    /// Loose ends still open.
    pub open_loose_ends: usize,
    /// Roadmap Epics (`EPIC-P*` on disk or in the backlog phase table).
    pub epics_total: usize,
    /// Of those, Epics with at least one Story contract row.
    pub epics_decomposed: usize,
    /// Stories whose own `Status:` header state is `Verified`.
    pub stories_verified: usize,
    /// Stories whose own `Status:` header state is `Functionally Verified`.
    pub stories_functionally_verified: usize,
    /// Stories whose own `Status:` header state is `Specified`.
    pub stories_specified: usize,
    /// Stories whose own `Status:` header state is `In progress`.
    pub stories_in_progress: usize,
}

/// Renders the generated stat-row block, markers included.
pub fn emit_stat_row(facts: &DashboardFacts) -> String {
    let mut block = String::from(BEGIN_MARKER);
    block.push('\n');
    for (value, label) in [
        (format!("{}", facts.catalogue_cells), "Performance catalogue cells"),
        (
            format!("{}&nbsp;+&nbsp;{}", facts.containment_classes, facts.boundary_tests),
            "Containment classes + boundary tests",
        ),
        (
            format!(
                "{}&nbsp;+&nbsp;{}&nbsp;+&nbsp;{}",
                facts.protection_domains, facts.code_admission_gates, facts.class_paths
            ),
            "PD + code gates + class paths",
        ),
        (format!("{}&nbsp;/&nbsp;{}", facts.stories, facts.stories), "Stories mapped by CI"),
        (format!("{}", facts.security_controls), "Security controls"),
        (
            format!("{}&nbsp;+&nbsp;{}", facts.application_targets, facts.landing_zones),
            "Application targets + landing zones",
        ),
        (
            format!("{}&nbsp;/&nbsp;{}", facts.assurance_verified, facts.stories),
            "Stories assurance-verified",
        ),
        (
            format!("{}&nbsp;/&nbsp;{}", facts.qualified_platforms, facts.platforms),
            "Platforms qualified — ADR 0005 bars every G04 until one is",
        ),
        (
            format!("{}&nbsp;/&nbsp;{}", facts.evidenced_gates, facts.in_play_gates),
            "Release gates with dated evidence",
        ),
        (
            format!("{}&nbsp;/&nbsp;{}", facts.reachable_gates, facts.in_play_gates),
            "Release gates reachable with no board",
        ),
        (
            format!("{}&nbsp;/&nbsp;{}", facts.evidenced_gates, facts.closable_gates),
            "Release gates with evidence, against the denominator that can be closed",
        ),
        (
            format!("{}", facts.measurable_today),
            "Gates unmeasured and measurable today — no board, no decision",
        ),
    ] {
        block.push_str(&format!(
            "  <div class=\"stat\"><div class=\"n\">{value}</div><div class=\"l\">{label}</div></div>\n"
        ));
    }
    block.push_str(END_MARKER);
    block
}

/// Renders the generated Overall-progress block, markers included
/// (`STORY-P0-01-09`).
///
/// Four tiles, replacing the four the page carried by hand from 2026-07-28 to
/// 2026-08-01 — during which the tabstrip beneath them drifted twice. The
/// labels are chosen here, deliberately, not derived: the values are
/// statistics, the wording is not.
pub fn emit_overall_progress(facts: &DashboardFacts) -> String {
    let mut block = String::from(PROGRESS_BEGIN_MARKER);
    block.push('\n');
    for (value, label) in [
        (
            format!("{}&nbsp;/&nbsp;{}", facts.epics_decomposed, facts.epics_total),
            "Epics decomposed",
        ),
        (format!("{}", facts.features), "Features in the spine"),
        (
            format!(
                "{}&nbsp;/&nbsp;{}",
                facts.stories_verified + facts.stories_functionally_verified,
                facts.stories
            ),
            "Stories functionally verified",
        ),
        (format!("{}", facts.tests), "Test docs in the spine"),
    ] {
        block.push_str(&format!(
            "  <div class=\"stat\"><div class=\"n\">{value}</div><div class=\"l\">{label}</div></div>\n"
        ));
    }
    block.push_str(PROGRESS_END_MARKER);
    block
}

/// The progress bar's width: the integer-rounded percentage of Stories whose
/// header state is `Verified` or `Functionally Verified`.
///
/// Derived rather than hand-tuned, because a hand-tuned percentage is a
/// decoration pretending to be a statistic (`STORY-P0-01-09`).
pub fn progress_bar_percent(facts: &DashboardFacts) -> usize {
    if facts.stories == 0 {
        return 0;
    }
    let functionally_verified = facts.stories_verified + facts.stories_functionally_verified;
    (functionally_verified * 100 + facts.stories / 2) / facts.stories
}

/// The roadmap Epic population: `EPIC-P*` documents on disk plus the `EPIC-P*`
/// rows of the backlog's phase table, as a union.
///
/// Horizon Epics (`EPIC-H*`) are excluded on the backlog's own authority: its
/// *Destination horizons* section states their ids "are not inserted into the
/// numbered critical path and do not imply sequence", so they are not part of
/// the denominator the page's Epics-decomposed tile argues about. Only the
/// table above that heading is read.
pub fn roadmap_epics(epic_doc_ids: &BTreeSet<String>, backlog: &str) -> BTreeSet<String> {
    let mut epics: BTreeSet<String> =
        epic_doc_ids.iter().filter(|id| id.starts_with("EPIC-P")).cloned().collect();
    for line in backlog.lines() {
        if line.starts_with("## Destination horizons") {
            break;
        }
        let Some(row) = line.strip_prefix('|') else { continue };
        let Some(first_cell) = row.split('|').next() else { continue };
        let Some(position) = first_cell.find("EPIC-P") else { continue };
        let id: String = first_cell[position..]
            .chars()
            .take_while(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
            .collect();
        epics.insert(id);
    }
    epics
}

/// How many roadmap Epics have at least one Story contract row.
///
/// The Story id carries its Epic token — `STORY-P0-01-08` belongs to
/// `EPIC-P0` — so decomposition is derived from the contracts register rather
/// than asserted by whoever last counted.
pub fn decomposed_epics(roadmap: &BTreeSet<String>, story_ids: &BTreeSet<String>) -> usize {
    let with_stories: BTreeSet<String> = story_ids
        .iter()
        .filter_map(|id| id.strip_prefix("STORY-"))
        .filter_map(|rest| rest.split('-').next())
        .map(|epic| format!("EPIC-{epic}"))
        .collect();
    roadmap.iter().filter(|epic| with_stories.contains(*epic)).count()
}

/// What the dashboard check looked at, so a caller can print coverage rather
/// than a bare "ok".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardSummary {
    /// Story status badges cross-checked against `list-status`.
    pub badges_checked: usize,
}

/// Refuses a dashboard that disagrees with the spine.
///
/// The checks run in the order a reader meets their subjects on the page: the
/// tabstrip, the Overall-progress tiles and bar, the footnote beneath them,
/// the Epic-denominator sentence, then `-08`'s assurance tiles, spine
/// sentence, and Story badges.
pub fn check_dashboard(
    repo_root: &Path,
    facts: &DashboardFacts,
    statuses: &[ArtifactStatus],
) -> Result<DashboardSummary, String> {
    let path = repo_root.join("goals").join("index.html");
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    check_tabstrip(&contents, facts)?;
    check_progress_region(&contents, facts)?;
    check_progress_bar(&contents, facts)?;
    check_footnote_counts(&contents, facts)?;
    check_epic_denominator(&contents, facts)?;
    check_generated_region(&contents, facts)?;
    check_block_placement(&contents)?;
    check_no_empty_stat_row(&contents)?;
    check_spine_sentence(&contents, facts)?;
    let badges_checked = check_story_badges(&contents, statuses)?;

    Ok(DashboardSummary { badges_checked })
}

/// A marked region must be byte-identical to what the emitter produces.
///
/// Shared by both generated regions. The three failure modes get three
/// messages, because a missing region, a truncated region and a stale region
/// are different defects and a single message for all would misdirect.
fn check_marked_region(
    contents: &str,
    begin: &str,
    end: &str,
    expected: &str,
    subject: &str,
) -> Result<(), String> {
    let normalised = contents.replace("\r\n", "\n");

    let Some(start) = normalised.find(begin) else {
        return Err(format!(
            "goals/index.html carries no `{begin}` marker. The {subject} are generated: \
             run `cargo run -p xtask -- emit-dashboard` and paste the block (LE-30)"
        ));
    };
    let Some(end_offset) = normalised[start..].find(end) else {
        return Err(format!(
            "goals/index.html opens the generated {subject} region but never closes it with \
             `{end}`"
        ));
    };
    let found = &normalised[start..start + end_offset + end.len()];
    if found != expected {
        return Err(format!(
            "goals/index.html's generated {subject} are stale. Run \
             `cargo run -p xtask -- emit-dashboard` and replace the block between the markers. \
             Expected:\n{expected}\n\nFound:\n{found}"
        ));
    }
    Ok(())
}

/// The generated tiles must be byte-identical to what the emitter produces.
fn check_generated_region(contents: &str, facts: &DashboardFacts) -> Result<(), String> {
    check_marked_region(contents, BEGIN_MARKER, END_MARKER, &emit_stat_row(facts), "stat tiles")
}

/// The generated Overall-progress tiles, same contract (`STORY-P0-01-09`).
fn check_progress_region(contents: &str, facts: &DashboardFacts) -> Result<(), String> {
    check_marked_region(
        contents,
        PROGRESS_BEGIN_MARKER,
        PROGRESS_END_MARKER,
        &emit_overall_progress(facts),
        "Overall-progress tiles",
    )
}

/// The `<h2>` heading a marker sits beneath, if any (`LE-70`).
///
/// Scans backwards from the marker to the nearest preceding `<h2>`, which is
/// the section the block renders under as far as a reader is concerned.
fn enclosing_heading<'a>(contents: &'a str, marker: &str) -> Option<&'a str> {
    let marker_at = contents.find(marker)?;
    let heading_at = contents[..marker_at].rfind("<h2>")? + "<h2>".len();
    let heading_end = contents[heading_at..].find("</h2>")? + heading_at;
    Some(contents[heading_at..heading_end].trim())
}

/// Every generated block must render under the heading it belongs to (`LE-70`).
///
/// The byte-compare in [`check_marked_region`] verifies a block's *content*
/// wherever it happens to sit. On 2026-08-03 the `overall-progress` block was
/// moved into the *Assurance release status* section, stacking above the
/// stat-row block and leaving *Overall progress* with an empty `stat-row` div.
/// The four headline tiles were invisible for four days across sixteen commits
/// and every gate passed on every one of them — the spine was run immediately
/// before and immediately after the corrective move and could not tell the
/// broken page from the fixed one. Content was never the problem; placement was.
fn check_block_placement(contents: &str) -> Result<(), String> {
    for (marker, expected, label) in [
        (PROGRESS_BEGIN_MARKER, "Overall progress", "Overall-progress tiles"),
        (BEGIN_MARKER, "Assurance release status", "stat tiles"),
    ] {
        let Some(heading) = enclosing_heading(contents, marker) else {
            return Err(format!(
                "goals/index.html: the generated {label} block sits under no `<h2>` section \
                 at all (LE-70)"
            ));
        };
        if !heading.starts_with(expected) {
            return Err(format!(
                "goals/index.html: the generated {label} block renders under `{heading}`, but \
                 belongs under `{expected}`. A block byte-compares correctly wherever it sits, \
                 so placement is checked separately (LE-70)"
            ));
        }
    }
    Ok(())
}

/// No `stat-row` container may be empty (`LE-70`).
///
/// The visible symptom of a relocated block is a heading with nothing under it.
/// Checking for the hole as well as for the block's location means either half
/// of the 2026-08-03 defect is caught on its own.
fn check_no_empty_stat_row(contents: &str) -> Result<(), String> {
    const OPEN: &str = "<div class=\"stat-row\">";
    for (offset, _) in contents.match_indices(OPEN) {
        let rest = contents[offset + OPEN.len()..].trim_start();
        if rest.starts_with("</div>") {
            return Err(
                "goals/index.html carries an empty `stat-row` container — a section heading with \
                 no tiles under it, which is how a relocated generated block looks to a reader \
                 (LE-70)"
                    .to_string(),
            );
        }
    }
    Ok(())
}

/// The tabstrip's two inline counts, gated rather than generated: one number
/// inside a one-line label is cheaper to extract than to generate, and the
/// label's markup is layout a generator has no business owning
/// (`STORY-P0-01-09`).
fn check_tabstrip(contents: &str, facts: &DashboardFacts) -> Result<(), String> {
    let expected_epics =
        format!("Epics <span class=\"count\">{} decomposed", facts.epics_decomposed);
    if !contents.contains(&expected_epics) {
        return Err(format!(
            "goals/index.html's tabstrip does not state the current decomposed-Epic count. \
             Expected the Epics tab label to open with `{expected_epics}` (STORY-P0-01-09)"
        ));
    }
    let expected_loose =
        format!("Loose ends <span class=\"count\">{} open</span>", facts.open_loose_ends);
    if !contents.contains(&expected_loose) {
        return Err(format!(
            "goals/index.html's tabstrip does not state the current open loose-end count. \
             Expected `{expected_loose}` (STORY-P0-01-09)"
        ));
    }
    Ok(())
}

/// The progress bar's width must be the derived Stories ratio, not a hand-tuned
/// percentage (`STORY-P0-01-09`).
fn check_progress_bar(contents: &str, facts: &DashboardFacts) -> Result<(), String> {
    let expected =
        format!("<div class=\"bar-fill\" style=\"width:{}%\">", progress_bar_percent(facts));
    if !contents.contains(&expected) {
        return Err(format!(
            "goals/index.html's progress bar width is stale. Expected `{expected}` — the \
             integer-rounded percentage of Stories functionally verified (STORY-P0-01-09)"
        ));
    }
    Ok(())
}

/// The footnote's four state counts, gated; the sentence and its date stay
/// editorial (`STORY-P0-01-09`).
///
/// The date is deliberately not machine-stamped: an emitter-written date would
/// fail the byte-compare on every day boundary with no content change. The
/// counts moving forces the sentence to be re-edited, which moves the date.
fn check_footnote_counts(contents: &str, facts: &DashboardFacts) -> Result<(), String> {
    let expected = format!(
        "{} <code>Verified</code> + {} <code>Functionally Verified</code> of {} Stories, \
         {} <code>Specified</code>, {} <code>In progress</code>",
        facts.stories_verified,
        facts.stories_functionally_verified,
        facts.stories,
        facts.stories_specified,
        facts.stories_in_progress
    );
    if !contents.contains(&expected) {
        return Err(format!(
            "goals/index.html's list-status footnote does not state the current Story state \
             counts. Expected `{expected}` (STORY-P0-01-09)"
        ));
    }
    Ok(())
}

/// The Epic-denominator claim, gated against the Epics on disk so the next
/// written Epic cannot leave the page claiming the old denominator
/// (`STORY-P0-01-09`).
fn check_epic_denominator(contents: &str, facts: &DashboardFacts) -> Result<(), String> {
    let expected = format!("The Epic denominator is now {}", facts.epics_total);
    if !contents.contains(&expected) {
        return Err(format!(
            "goals/index.html does not state the current roadmap-Epic denominator. Expected \
             `{expected}` — EPIC-P* documents on disk plus the backlog phase table \
             (STORY-P0-01-09)"
        ));
    }
    Ok(())
}

/// The prose sentence that restates the spine counts.
///
/// Gated rather than generated: the paragraph around it is an argument, and
/// this module has no business rewriting arguments. What it can do is refuse
/// the four numbers inside it when they stop being true.
fn check_spine_sentence(contents: &str, facts: &DashboardFacts) -> Result<(), String> {
    let expected_counts = format!(
        "<strong>{} Features / {} Stories / {} Tests / {} Reports</strong>",
        facts.features, facts.stories, facts.tests, facts.reports
    );
    if !contents.contains(&expected_counts) {
        return Err(format!(
            "goals/index.html does not state the current spine counts. Expected the sentence to \
             carry `{expected_counts}` (LE-30)"
        ));
    }
    let expected_loose = format!(
        "<strong>{} loose ends ({} open)</strong>",
        facts.loose_ends, facts.open_loose_ends
    );
    if !contents.contains(&expected_loose) {
        return Err(format!(
            "goals/index.html does not state the current loose-end counts. Expected \
             `{expected_loose}` (LE-30)"
        ));
    }
    // `LE-108`, and the same rule one sentence along. This one drifted by 33
    // Stories while sitting two lines below counts that were already gated,
    // which is the argument for gating it rather than correcting it again.
    let expected_population =
        format!("Of the {} <code>EPIC-P0</code>/<code>EPIC-P1</code> Stories", facts.p0p1_stories);
    if !contents.contains(&expected_population) {
        return Err(format!(
            "goals/index.html does not state the current EPIC-P0/EPIC-P1 Story population. \
             Expected `{expected_population}` (LE-108)"
        ));
    }
    let expected_settled =
        format!("<strong>{} are Verified or Functionally Verified</strong>", facts.p0p1_settled);
    if !contents.contains(&expected_settled) {
        return Err(format!(
            "goals/index.html does not state how many EPIC-P0/EPIC-P1 Stories are settled. \
             Expected `{expected_settled}` (LE-108)"
        ));
    }
    Ok(())
}

/// Every `STORY-* <span class="badge …">TEXT</span>` pair, checked against that
/// Story's own `Status:` header.
///
/// `LE-44`'s rule, one document along. The badge text may carry a tier in
/// parentheses — `VERIFIED (Tier 0 + Host)` — because that is genuinely extra
/// information; what it may not do is name a *different state*.
fn check_story_badges(contents: &str, statuses: &[ArtifactStatus]) -> Result<usize, String> {
    let by_id: BTreeMap<&str, &ArtifactStatus> =
        statuses.iter().map(|status| (status.id.as_str(), status)).collect();

    let mut checked = 0;
    for (story_id, badge) in story_badges(contents) {
        let Some(status) = by_id.get(story_id.as_str()) else {
            return Err(format!(
                "goals/index.html badges `{story_id}`, which has no Story document"
            ));
        };
        let expected = badge_for_state(&status.state).ok_or_else(|| {
            format!(
                "goals/index.html: `{story_id}` has state `{}`, which has no badge \
                     spelling; add one rather than guessing",
                status.state
            )
        })?;
        // The badge may append a tier in parentheses but must open with the
        // state it is claiming.
        if !badge.starts_with(expected) {
            return Err(format!(
                "goals/index.html badges `{story_id}` as `{badge}`, but that Story's own \
                 `Status:` header says `{}`, whose badge is `{expected}`. The Story is \
                 authoritative about its own state (LE-30, LE-44's rule one document along)",
                status.state
            ));
        }
        checked += 1;
    }

    if checked == 0 {
        return Err("goals/index.html carries no Story status badges to check".to_string());
    }
    Ok(checked)
}

/// The badge spelling for each `Status:` state.
///
/// Explicit rather than uppercasing the state, so that adding a state to the
/// vocabulary is a decision about how the dashboard should say it rather than a
/// string transformation nobody reviewed.
fn badge_for_state(state: &str) -> Option<&'static str> {
    match state {
        "Verified" => Some("VERIFIED"),
        "Functionally Verified" => Some("FUNCTIONALLY VERIFIED"),
        "Functionally complete" => Some("FUNCTIONALLY COMPLETE"),
        "Complete" => Some("COMPLETE"),
        "In progress" => Some("IN PROGRESS"),
        "Specified" => Some("SPECIFIED"),
        _ => None,
    }
}

/// Scans for `<a href="stories/STORY-….md">…</a> <span class="badge …">TEXT</span>`.
///
/// A hand-rolled scan rather than a regex: `xtask` has no regex dependency and
/// the shape is fixed by the page's own markup. A Story link *not* immediately
/// followed by a badge is ignored — the page links Stories inside prose
/// constantly and only the badged ones are making a state claim.
fn story_badges(contents: &str) -> Vec<(String, String)> {
    const LINK_PREFIX: &str = "<a href=\"stories/STORY-";
    const BADGE_PREFIX: &str = "<span class=\"badge ";

    let mut found = Vec::new();
    let mut rest = contents;
    while let Some(position) = rest.find(LINK_PREFIX) {
        rest = &rest[position + LINK_PREFIX.len()..];
        let Some(dot) = rest.find(".md\">") else { continue };
        let story_id = format!("STORY-{}", &rest[..dot]);
        let after_link = &rest[dot..];
        let Some(close) = after_link.find("</a>") else { continue };
        let tail = after_link[close + "</a>".len()..].trim_start();
        let Some(badge_open) = tail.strip_prefix(BADGE_PREFIX) else { continue };
        let Some(gt) = badge_open.find('>') else { continue };
        let text = &badge_open[gt + 1..];
        let Some(end) = text.find("</span>") else { continue };
        found.push((story_id, text[..end].to_string()));
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> DashboardFacts {
        DashboardFacts {
            catalogue_cells: 625,
            containment_classes: 5,
            boundary_tests: 20,
            protection_domains: 14,
            code_admission_gates: 14,
            class_paths: 25,
            stories: 59,
            security_controls: 20,
            application_targets: 19,
            landing_zones: 9,
            assurance_verified: 0,
            evidenced_gates: 11,
            in_play_gates: 391,
            reachable_gates: 345,
            closable_gates: 220,
            measurable_today: 125,
            p0p1_stories: 85,
            p0p1_settled: 72,
            platforms: 5,
            qualified_platforms: 0,
            features: 23,
            tests: 46,
            reports: 47,
            loose_ends: 46,
            open_loose_ends: 28,
            epics_total: 12,
            epics_decomposed: 4,
            stories_verified: 30,
            stories_functionally_verified: 11,
            stories_specified: 12,
            stories_in_progress: 6,
        }
    }

    fn status(id: &str, state: &str) -> ArtifactStatus {
        ArtifactStatus { id: id.to_string(), state: state.to_string(), detail: String::new() }
    }

    fn page(badges: &str) -> String {
        format!(
            "{}\n<strong>23 Features / 59 Stories / 46 Tests / 47 Reports</strong> plus \
             <strong>46 loose ends (28 open)</strong>\n{badges}",
            emit_stat_row(&facts())
        )
    }

    // --- the generated region -----------------------------------------------

    #[test]
    fn a_stale_generated_tile_is_refused_and_the_fix_is_printed() {
        let stale = page("").replace("11&nbsp;/&nbsp;391", "10&nbsp;/&nbsp;391");
        let error = check_generated_region(&stale, &facts()).expect_err("a stale tile must fail");
        assert!(error.contains("emit-dashboard"), "{error}");
        assert!(error.contains("11&nbsp;/&nbsp;391"), "the fix is in the message: {error}");
    }

    #[test]
    fn a_page_with_no_markers_is_refused() {
        let error = check_generated_region("<html></html>", &facts())
            .expect_err("a page without the region must fail");
        assert!(error.contains("carries no"), "{error}");
    }

    #[test]
    fn an_unclosed_generated_region_is_refused() {
        let truncated = page("").replace(END_MARKER, "");
        let error =
            check_generated_region(&truncated, &facts()).expect_err("an unclosed region must fail");
        assert!(error.contains("never closes it"), "{error}");
    }

    #[test]
    fn the_emitted_block_is_what_the_check_accepts() {
        check_generated_region(&page(""), &facts()).expect("emitter and checker agree");
    }

    #[test]
    fn crlf_line_endings_do_not_fail_the_byte_comparison() {
        let windows = page("").replace('\n', "\r\n");
        check_generated_region(&windows, &facts()).expect("a CRLF checkout is not a defect");
    }

    // --- the Overall-progress region (STORY-P0-01-09) -------------------------

    /// A page whose generated Overall-progress region and gated numerics all
    /// agree with `facts()`. 41 of 59 Stories functionally verified is 69.49%,
    /// so the derived bar width is 69.
    fn progress_page() -> String {
        format!(
            "<label>Epics <span class=\"count\">4 decomposed &middot; P2 partial</span></label>\n\
             <label>Loose ends <span class=\"count\">28 open</span></label>\n\
             {}\n\
             <div class=\"bar-fill\" style=\"width:69%\"></div>\n\
             30 <code>Verified</code> + 11 <code>Functionally Verified</code> of 59 Stories, \
             12 <code>Specified</code>, 6 <code>In progress</code>\n\
             The Epic denominator is now 12: as the backlog says.",
            emit_overall_progress(&facts())
        )
    }

    #[test]
    fn a_stale_overall_progress_tile_is_refused_and_the_fix_is_printed() {
        let stale = progress_page().replace("4&nbsp;/&nbsp;12", "3&nbsp;/&nbsp;12");
        let error = check_progress_region(&stale, &facts()).expect_err("a stale tile must fail");
        assert!(error.contains("emit-dashboard"), "{error}");
        assert!(error.contains("4&nbsp;/&nbsp;12"), "the fix is in the message: {error}");
    }

    #[test]
    fn a_page_with_no_overall_progress_markers_is_refused() {
        let error = check_progress_region("<html></html>", &facts())
            .expect_err("a page without the region must fail");
        assert!(error.contains("carries no"), "{error}");
    }

    #[test]
    fn an_unclosed_overall_progress_region_is_refused() {
        let truncated = progress_page().replace(PROGRESS_END_MARKER, "");
        let error =
            check_progress_region(&truncated, &facts()).expect_err("an unclosed region must fail");
        assert!(error.contains("never closes it"), "{error}");
    }

    #[test]
    fn the_emitted_overall_progress_block_is_what_the_check_accepts() {
        check_progress_region(&progress_page(), &facts()).expect("emitter and checker agree");
    }

    #[test]
    fn crlf_line_endings_do_not_fail_the_overall_progress_comparison() {
        let windows = progress_page().replace('\n', "\r\n");
        check_progress_region(&windows, &facts()).expect("a CRLF checkout is not a defect");
    }

    // --- the gated numerics that survived -08 (STORY-P0-01-09) ----------------

    // --- LE-70: placement, not just content ---------------------------------

    /// The page as it should be: each generated block under its own heading.
    fn placed_page() -> String {
        format!(
            "<details><summary><h2>Overall progress</h2></summary>\n\
             <div class=\"stat-row\">\n{}\n</div>\n</details>\n\
             <details><summary><h2>Assurance release status <span class=\"badge\">DEBT</span>\
             </h2></summary>\n<div class=\"stat-row\">\n{}\n</div>\n</details>",
            emit_overall_progress(&facts()),
            emit_stat_row(&facts())
        )
    }

    /// The page as commit 3849ece actually left it: the `overall-progress`
    /// block relocated into *Assurance release status*, stacked above the
    /// stat-row block, with *Overall progress* left holding an empty div.
    fn relocated_page() -> String {
        format!(
            "<details><summary><h2>Overall progress</h2></summary>\n\
             <div class=\"stat-row\">\n</div>\n</details>\n\
             <details><summary><h2>Assurance release status <span class=\"badge\">DEBT</span>\
             </h2></summary>\n<div class=\"stat-row\">\n{}\n\n{}\n</div>\n</details>",
            emit_overall_progress(&facts()),
            emit_stat_row(&facts())
        )
    }

    #[test]
    fn a_correctly_placed_page_is_accepted() {
        check_block_placement(&placed_page()).expect("each block sits under its own heading");
        check_no_empty_stat_row(&placed_page()).expect("no container is empty");
    }

    /// Both blocks still byte-compare perfectly in the relocated page — which
    /// is exactly why the content gate could not see the defect for four days.
    #[test]
    fn the_relocated_page_still_passes_every_content_check() {
        let page = relocated_page();
        check_generated_region(&page, &facts()).expect("stat tiles are byte-identical");
        check_progress_region(&page, &facts()).expect("progress tiles are byte-identical");
    }

    #[test]
    fn a_block_rendered_under_the_wrong_heading_is_refused() {
        let error = check_block_placement(&relocated_page())
            .expect_err("a relocated block must be refused");
        assert!(error.contains("Assurance release status"), "names where it wrongly sits: {error}");
        assert!(error.contains("Overall progress"), "names where it belongs: {error}");
    }

    /// `LE-84`'s other half. The decomposition existed only inside
    /// `xtask assurance-status`, which nobody runs; the dashboard published
    /// the raw `evidenced/in_play` ratio alone, and four consecutive handovers
    /// quoted it without saying that half its denominator cannot be closed by
    /// construction. A reader who sees only that ratio concludes the wrong
    /// thing in both directions, so the closable denominator ships beside it.
    ///
    /// The fixture's `closable_gates` is deliberately different from its
    /// `in_play_gates`: a test that let them be equal would pass against a
    /// dashboard that had quietly published the same ratio twice, which is the
    /// exact regression this asserts against.
    #[test]
    fn the_stat_row_publishes_the_closable_denominator_beside_the_raw_one() {
        let facts = facts();
        assert_ne!(
            facts.closable_gates, facts.in_play_gates,
            "the fixture must distinguish the two denominators or this test proves nothing"
        );
        let row = emit_stat_row(&facts);

        let raw = format!("{}&nbsp;/&nbsp;{}", facts.evidenced_gates, facts.in_play_gates);
        let closable = format!("{}&nbsp;/&nbsp;{}", facts.evidenced_gates, facts.closable_gates);
        assert!(row.contains(&raw), "the in-play ratio must still be published: {row}");
        assert!(row.contains(&closable), "the closable ratio must be published too: {row}");
        assert!(
            row.contains("denominator that can be closed"),
            "the closable tile must say what makes it different: {row}"
        );
    }

    /// The two numbers that explain the other tiles rather than restating them:
    /// what is available today, and why `assurance_verified` cannot move.
    #[test]
    fn the_stat_row_publishes_what_is_actionable_and_what_is_locked() {
        let facts = facts();
        let row = emit_stat_row(&facts);

        assert!(
            row.contains("<div class=\"n\">125</div>"),
            "the measurable-today count must be published as its own figure: {row}"
        );
        assert!(
            row.contains("no board, no decision"),
            "the measurable-today tile must say what is NOT blocking it: {row}"
        );

        let qualified = format!("{}&nbsp;/&nbsp;{}", facts.qualified_platforms, facts.platforms);
        assert!(row.contains(&qualified), "the qualification count must be published: {row}");
        assert!(
            row.contains("ADR 0005"),
            "the qualification tile must name the decision that bars G04, because that is \
             the reason `Stories assurance-verified` reads zero and cannot move: {row}"
        );
    }

    /// `LE-108`. The prose sentence carried `52` Stories and `43` settled
    /// while the tree held `85` and `72`, and it sat two lines below the spine
    /// and loose-end counts that `LE-30` already gates — so the drift was not
    /// merely possible, it was invisible beside numbers that could not drift.
    ///
    /// The fixture below is the sentence as it was actually committed, stale
    /// figures and all. A test written against a made-up string would pass
    /// against a check that matched nothing.
    #[test]
    fn the_spine_sentence_refuses_a_stale_story_population() {
        // The spine and loose-end counts here MATCH the fixture deliberately,
        // so the only thing that can fail is the new check. With stale leading
        // counts the earlier gate fires first and this test would pass while
        // asserting nothing about the sentence it is named for.
        const STALE: &str = "Re-synced <strong>23 Features / 59 Stories / 46 Tests / 47 Reports</strong>, \
             plus <strong>46 loose ends (28 open)</strong>. Of the 52 <code>EPIC-P0</code>/\
             <code>EPIC-P1</code> Stories, <strong>43 are Verified or Functionally Verified</strong>, \
             <strong>2 are In progress</strong> and 7 are Specified.";
        let facts = facts();
        assert!(
            check_spine_sentence(
                &STALE.replace("Of the 52", "Of the 85").replace(
                    "<strong>43 are Verified or Functionally Verified</strong>",
                    "<strong>72 are Verified or Functionally Verified</strong>"
                ),
                &facts
            )
            .is_ok(),
            "with both figures corrected the sentence must pass, or this test is measuring \
             one of the earlier checks instead"
        );

        let error = check_spine_sentence(STALE, &facts)
            .expect_err("a sentence naming 52 Stories must be refused when the tree holds 85");
        assert!(error.contains("Of the 85"), "the error must name what it expected: {error}");
        assert!(error.contains("LE-108"), "the error must cite its row: {error}");

        // And the settled count is refused independently of the population, or
        // a sentence that fixed one number and not the other would pass.
        let population_fixed = STALE.replace("Of the 52", "Of the 85");
        let error = check_spine_sentence(&population_fixed, &facts)
            .expect_err("43 settled must still be refused when the tree holds 72");
        assert!(
            error.contains("<strong>72 are Verified or Functionally Verified</strong>"),
            "the second check must name the settled count it expected: {error}"
        );
    }

    /// The positive half: the committed page satisfies the gate it ships with.
    /// Without this, a check that rejected *everything* would pass the test
    /// above and redden the build for the whole repository.
    ///
    /// The facts are **derived from the live tree**, never written here as
    /// literals: a hard-coded population is stale the moment any session adds
    /// an `EPIC-P0`/`EPIC-P1` Story, and this test's first version proved it —
    /// it pinned 85 and went red when a concurrent session's Story made the
    /// regenerated page (correctly) say 86. A count of how much work exists is
    /// a floor, never a total (`CONCURRENT_SESSIONS.md`), and a test literal is
    /// the same defect as the five hand-synced spine counts of 2026-07-28.
    #[test]
    fn the_committed_page_states_the_current_story_population() {
        let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf();
        let page = std::fs::read_to_string(repo_root.join("goals").join("index.html"))
            .expect("goals/index.html must be readable");
        let facts = crate::assurance::dashboard_facts(&repo_root)
            .expect("the live spine must yield dashboard facts");
        let expected_population = format!(
            "Of the {} <code>EPIC-P0</code>/<code>EPIC-P1</code> Stories",
            facts.p0p1_stories
        );
        assert!(
            page.contains(&expected_population),
            "the committed page must carry `{expected_population}`"
        );
        check_spine_sentence(&page, &facts)
            .expect("the committed page must pass the gate it ships with");
    }

    #[test]
    fn an_empty_stat_row_is_refused() {
        let error =
            check_no_empty_stat_row(&relocated_page()).expect_err("an empty container must fail");
        assert!(error.contains("empty `stat-row`"), "unexpected error: {error}");
    }

    #[test]
    fn a_block_under_no_section_at_all_is_refused() {
        let orphaned = format!("<html>{}</html>", emit_overall_progress(&facts()));
        let error =
            check_block_placement(&orphaned).expect_err("an orphaned block must be refused");
        assert!(error.contains("no `<h2>` section"), "unexpected error: {error}");
    }

    /// The committed page must satisfy the gate it ships with.
    #[test]
    fn the_committed_page_places_every_generated_block_correctly() {
        let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf();
        let contents = fs::read_to_string(repo_root.join("goals").join("index.html"))
            .expect("the committed dashboard must be readable");
        check_block_placement(&contents).expect("every block sits under its own heading");
        check_no_empty_stat_row(&contents).expect("no container is empty");
    }

    #[test]
    fn a_stale_tabstrip_epic_count_is_refused_and_the_fix_is_printed() {
        let stale = progress_page().replace("4 decomposed", "3 decomposed");
        let error = check_tabstrip(&stale, &facts()).expect_err("a stale Epic count must fail");
        assert!(error.contains("4 decomposed"), "the fix is in the message: {error}");
    }

    #[test]
    fn a_stale_tabstrip_loose_end_count_is_refused() {
        let stale = progress_page().replace("28 open", "26 open");
        let error =
            check_tabstrip(&stale, &facts()).expect_err("a stale loose-end count must fail");
        assert!(error.contains("28 open"), "the fix is in the message: {error}");
    }

    #[test]
    fn an_agreeing_tabstrip_is_accepted() {
        check_tabstrip(&progress_page(), &facts()).expect("agreement is accepted");
    }

    #[test]
    fn a_hand_tuned_bar_width_is_refused_and_the_derived_width_is_accepted() {
        check_progress_bar(&progress_page(), &facts()).expect("the derived width is accepted");
        let stale = progress_page().replace("width:69%", "width:75%");
        let error = check_progress_bar(&stale, &facts()).expect_err("a hand-tuned width must fail");
        assert!(error.contains("width:69%"), "the fix is in the message: {error}");
    }

    #[test]
    fn the_bar_width_is_the_integer_rounded_story_ratio() {
        assert_eq!(progress_bar_percent(&facts()), 69, "41 of 59 is 69.49%");
        let mut two_thirds = facts();
        two_thirds.stories = 3;
        two_thirds.stories_verified = 2;
        two_thirds.stories_functionally_verified = 0;
        assert_eq!(progress_bar_percent(&two_thirds), 67, "2 of 3 rounds up");
        let mut empty = facts();
        empty.stories = 0;
        empty.stories_verified = 0;
        empty.stories_functionally_verified = 0;
        assert_eq!(progress_bar_percent(&empty), 0, "no Stories is 0%, not a division fault");
    }

    #[test]
    fn stale_footnote_state_counts_are_refused_and_the_fix_is_printed() {
        check_footnote_counts(&progress_page(), &facts()).expect("agreeing counts are accepted");
        let stale = progress_page().replace(
            "11 <code>Functionally Verified</code>",
            "9 <code>Functionally Verified</code>",
        );
        let error =
            check_footnote_counts(&stale, &facts()).expect_err("stale state counts must fail");
        assert!(error.contains("11 <code>Functionally Verified</code>"), "{error}");
    }

    #[test]
    fn a_stale_epic_denominator_is_refused_and_the_fix_is_printed() {
        check_epic_denominator(&progress_page(), &facts()).expect("an agreeing claim is accepted");
        let stale = progress_page().replace("denominator is now 12", "denominator is now 11");
        let error =
            check_epic_denominator(&stale, &facts()).expect_err("a stale denominator must fail");
        assert!(error.contains("The Epic denominator is now 12"), "{error}");
    }

    // --- the Epic population, derived from disk (STORY-P0-01-09) --------------

    #[test]
    fn the_roadmap_population_is_the_union_of_docs_and_the_phase_table() {
        let docs = BTreeSet::from(["EPIC-P0".to_string(), "EPIC-H2".to_string()]);
        let backlog = "| Epic | Roadmap phase |\n\
                       |---|---|\n\
                       | [`EPIC-P1`](EPIC-P1.md) — **decomposed** | Phase 1 |\n\
                       | `EPIC-P1_5` | Phase 1.5 |\n\
                       ## Destination horizons\n\
                       | `EPIC-H1` | Games |\n";
        let roadmap = roadmap_epics(&docs, backlog);
        assert_eq!(
            roadmap,
            BTreeSet::from(
                ["EPIC-P0".to_string(), "EPIC-P1".to_string(), "EPIC-P1_5".to_string(),]
            ),
            "P0 from disk, P1 and P1_5 from the table; horizon Epics excluded on both sides"
        );
    }

    #[test]
    fn an_epic_is_decomposed_when_a_story_contract_row_belongs_to_it() {
        let roadmap =
            BTreeSet::from(["EPIC-P0".to_string(), "EPIC-P1".to_string(), "EPIC-P2".to_string()]);
        let stories = BTreeSet::from([
            "STORY-P0-01-01".to_string(),
            "STORY-P0-02-01".to_string(),
            "STORY-P2-01-01".to_string(),
            "STORY-P9-01-01".to_string(),
        ]);
        assert_eq!(
            decomposed_epics(&roadmap, &stories),
            2,
            "P0 counts once, P2 counts, P1 has no Story, P9 is outside the population"
        );
    }

    // --- the gated prose ------------------------------------------------------

    #[test]
    fn a_stale_spine_sentence_is_refused() {
        let stale = page("").replace("47 Reports", "46 Reports");
        let error = check_spine_sentence(&stale, &facts()).expect_err("a stale count must fail");
        assert!(error.contains("47 Reports"), "{error}");
    }

    #[test]
    fn a_stale_loose_end_count_is_refused() {
        let stale = page("").replace("46 loose ends (28 open)", "44 loose ends (26 open)");
        let error =
            check_spine_sentence(&stale, &facts()).expect_err("a stale loose-end count must fail");
        assert!(error.contains("loose-end counts"), "{error}");
    }

    // --- the badges, LE-44's rule one document along ---------------------------

    fn badge(story: &str, class: &str, text: &str) -> String {
        format!(
            "<a href=\"stories/{story}.md\">{story}</a> <span class=\"badge {class}\">{text}</span>"
        )
    }

    /// The defect this check found on first contact with the real page, in the
    /// direction it actually occurred.
    #[test]
    fn a_badge_overstating_functionally_verified_as_verified_is_refused() {
        let contents = badge("STORY-P0-01-05", "verified", "VERIFIED");
        let statuses = [status("STORY-P0-01-05", "Functionally Verified")];
        let error = check_story_badges(&contents, &statuses)
            .expect_err("an overstated badge must be refused");
        assert!(error.contains("FUNCTIONALLY VERIFIED"), "{error}");
    }

    #[test]
    fn a_badge_naming_a_story_that_does_not_exist_is_refused() {
        let contents = badge("STORY-P9-99-99", "verified", "VERIFIED");
        let error = check_story_badges(&contents, &[status("STORY-P0-01-01", "Verified")])
            .expect_err("an unknown Story must be refused");
        assert!(error.contains("no Story document"), "{error}");
    }

    /// The acceptance cases. Without them every refusal above would also pass
    /// against a check that refused unconditionally.
    #[test]
    fn an_agreeing_badge_is_accepted_with_or_without_a_tier_suffix() {
        let contents = format!(
            "{}\n{}\n{}",
            badge("STORY-P0-01-01", "verified", "VERIFIED"),
            badge("STORY-P1-01-01", "verified", "FUNCTIONALLY VERIFIED (Tier 0 + Host)"),
            badge("STORY-P1-07-01", "blocked", "IN PROGRESS"),
        );
        let statuses = [
            status("STORY-P0-01-01", "Verified"),
            status("STORY-P1-01-01", "Functionally Verified"),
            status("STORY-P1-07-01", "In progress"),
        ];
        assert_eq!(check_story_badges(&contents, &statuses).expect("agreement is accepted"), 3);
    }

    /// A Story mentioned in prose is not making a state claim, so it is not
    /// checked. Without this the page could not link a Story in a sentence.
    #[test]
    fn a_story_linked_in_prose_without_a_badge_is_not_a_claim() {
        let contents = "see <a href=\"stories/STORY-P0-01-01.md\">STORY-P0-01-01</a> for the \
                        argument";
        let error = check_story_badges(contents, &[status("STORY-P0-01-01", "Verified")])
            .expect_err("a page with no badges at all is itself suspicious");
        assert!(error.contains("no Story status badges"), "{error}");
    }

    #[test]
    fn every_status_state_has_a_badge_spelling() {
        for state in [
            "Verified",
            "Functionally Verified",
            "Functionally complete",
            "Complete",
            "In progress",
            "Specified",
        ] {
            assert!(badge_for_state(state).is_some(), "{state} has no badge spelling");
        }
    }
}
