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

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::assurance::ArtifactStatus;

/// Where the generated block starts and ends inside `goals/index.html`.
const BEGIN_MARKER: &str =
    "<!-- BEGIN GENERATED stat-row: cargo run -p xtask -- emit-dashboard -->";
const END_MARKER: &str = "<!-- END GENERATED stat-row -->";

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
            format!("{}&nbsp;/&nbsp;{}", facts.evidenced_gates, facts.in_play_gates),
            "Release gates with dated evidence",
        ),
        (
            format!("{}&nbsp;/&nbsp;{}", facts.reachable_gates, facts.in_play_gates),
            "Release gates reachable with no board",
        ),
    ] {
        block.push_str(&format!(
            "  <div class=\"stat\"><div class=\"n\">{value}</div><div class=\"l\">{label}</div></div>\n"
        ));
    }
    block.push_str(END_MARKER);
    block
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
/// Three checks, in the order a reader meets them on the page.
pub fn check_dashboard(
    repo_root: &Path,
    facts: &DashboardFacts,
    statuses: &[ArtifactStatus],
) -> Result<DashboardSummary, String> {
    let path = repo_root.join("goals").join("index.html");
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    check_generated_region(&contents, facts)?;
    check_spine_sentence(&contents, facts)?;
    let badges_checked = check_story_badges(&contents, statuses)?;

    Ok(DashboardSummary { badges_checked })
}

/// The generated tiles must be byte-identical to what the emitter produces.
fn check_generated_region(contents: &str, facts: &DashboardFacts) -> Result<(), String> {
    let expected = emit_stat_row(facts);
    let normalised = contents.replace("\r\n", "\n");

    let Some(start) = normalised.find(BEGIN_MARKER) else {
        return Err(format!(
            "goals/index.html carries no `{BEGIN_MARKER}` marker. The stat tiles are generated: \
             run `cargo run -p xtask -- emit-dashboard` and paste the block (LE-30)"
        ));
    };
    let Some(end_offset) = normalised[start..].find(END_MARKER) else {
        return Err(format!(
            "goals/index.html opens the generated region but never closes it with `{END_MARKER}`"
        ));
    };
    let found = &normalised[start..start + end_offset + END_MARKER.len()];
    if found != expected {
        return Err(format!(
            "goals/index.html's generated stat tiles are stale. Run \
             `cargo run -p xtask -- emit-dashboard` and replace the block between the markers. \
             Expected:\n{expected}\n\nFound:\n{found}"
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
            features: 23,
            tests: 46,
            reports: 47,
            loose_ends: 46,
            open_loose_ends: 28,
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
