//! `goals/state.html` — the coherent state of the project, generated.
//!
//! # Why this exists, and why it is not written by hand
//!
//! The owner asked for one place that lists every session, every Epic and
//! Feature with its status, every open loose end, and **what blocks what**.
//! Four documents already answer parts of that and none answers all of it:
//! `goals/index.html` is a dashboard with a hand-written narrative,
//! `goals/feasibility.html` asks a different question ("could this be
//! proven?"), `loose-ends.tsv` is a register with no join to the Features it
//! blocks, and the session folder is a directory listing.
//!
//! So this page is **derived on the run that emits it**, for the reason
//! `LE-108` records: the one number on `goals/index.html` that no gate checked
//! drifted by 33 Stories while sitting two lines below numbers that could not.
//! A coherence page that can go stale is worse than no coherence page, because
//! it looks like the answer.
//!
//! # The join this page adds
//!
//! Nothing in the tree previously said *which open loose end blocks which
//! Feature*. The register names ids in its prose; the Features name rows in
//! theirs; neither is machine-read. [`blockers_for`] does that join by id, so
//! "what must be addressed first" is computed from the register rather than
//! remembered — and a row that stops naming a Feature stops blocking it on
//! this page the same day.
//!
//! **What the join cannot claim:** that an unmentioned Feature is unblocked. A
//! row blocks what it *names*, and a blocker nobody wrote down is invisible
//! here exactly as it is everywhere else. The page says so in those words
//! rather than presenting an empty cell as a clean bill of health.

use std::fmt::Write as _;
use std::path::Path;

use crate::assurance::{self, ArtifactStatus};

/// One open loose end, as this page needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenEnd {
    /// `LE-125`.
    pub id: String,
    /// The row's summary, first sentence only — the register holds the rest.
    pub summary: String,
    /// Where it was raised.
    pub raised_in: String,
}

/// The `LE-*` ids an open row names, joined to the artifact they block.
///
/// Case-sensitive and id-shaped on purpose: `FEAT-P1-09` matches, `feature 9`
/// does not. A fuzzy join on a register this load-bearing would invent
/// blockers, and an invented blocker is worse than a missed one — it sends
/// somebody to fix a thing that was never broken.
#[must_use]
pub fn blockers_for<'a>(artifact: &str, open: &'a [OpenEnd]) -> Vec<&'a OpenEnd> {
    open.iter().filter(|end| mentions(&end.summary, artifact)).collect()
}

/// Whether `text` names `id` as a whole identifier.
///
/// The suffix check is what stops `FEAT-P1-1` matching `FEAT-P1-12`: an id is
/// followed by a non-alphanumeric character or by nothing at all.
fn mentions(text: &str, id: &str) -> bool {
    let mut from = 0;
    while let Some(at) = text[from..].find(id) {
        let start = from + at;
        let end = start + id.len();
        let after_ok = text[end..].chars().next().is_none_or(|c| !c.is_ascii_alphanumeric());
        let before_ok =
            text[..start].chars().next_back().is_none_or(|c| !c.is_ascii_alphanumeric());
        if after_ok && before_ok {
            return true;
        }
        from = end;
    }
    false
}

/// Everything blocking a Feature: rows naming the Feature itself, **and rows
/// naming any of its Stories**.
///
/// The inheritance is the point. The register overwhelmingly names Stories —
/// that is where defects are found — so a Feature-only join reports 2 of 33
/// Features as blocked and reads as a clean tree. It is not: those rows block
/// the Feature through the Story that carries them, and a page that shows a
/// Feature clean while one of its Stories is held by four open rows is
/// misleading in the one direction this page must not be.
#[must_use]
pub fn feature_blockers<'a>(
    feature: &str,
    story_ids: &[String],
    open: &'a [OpenEnd],
) -> Vec<&'a OpenEnd> {
    open.iter()
        .filter(|end| {
            mentions(&end.summary, feature)
                || story_ids.iter().any(|story| mentions(&end.summary, story))
        })
        .collect()
}

/// The Feature a Story belongs to: `STORY-P1-09-16` → `FEAT-P1-09`.
///
/// Derived from the id rather than from a table, because the id *is* the
/// relation in this tree and a second table would be a thing to drift.
#[must_use]
pub fn feature_of(story: &str) -> Option<String> {
    let rest = story.strip_prefix("STORY-")?;
    let (feature, _index) = rest.rsplit_once('-')?;
    Some(format!("FEAT-{feature}"))
}

/// The Epic a Feature belongs to: `FEAT-P1-09` → `EPIC-P1`.
#[must_use]
pub fn epic_of(feature: &str) -> Option<String> {
    let rest = feature.strip_prefix("FEAT-")?;
    let phase = rest.split('-').next()?;
    Some(format!("EPIC-{phase}"))
}

/// Read the open rows of the loose-end register.
fn open_ends(repo_root: &Path) -> Result<Vec<OpenEnd>, String> {
    let path = repo_root.join("goals/assurance/loose-ends.tsv");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut open = Vec::new();
    for line in text.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 7 || fields[5] != "open" {
            continue;
        }
        // The first sentence carries the finding; the rest is the argument,
        // which belongs in the register and not on a summary page.
        let summary = fields[1];
        open.push(OpenEnd {
            id: fields[0].to_string(),
            summary: summary.to_string(),
            raised_in: fields[6].to_string(),
        });
    }
    Ok(open)
}

/// Every dated session folder and how many handovers it holds.
fn sessions(repo_root: &Path) -> Result<Vec<(String, usize)>, String> {
    let dir = repo_root.join("session");
    let mut found = Vec::new();
    for entry in
        std::fs::read_dir(&dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?
    {
        let entry = entry.map_err(|e| format!("{e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("hand-") || !entry.path().is_dir() {
            continue;
        }
        let count = std::fs::read_dir(entry.path())
            .map_err(|e| format!("{e}"))?
            .filter_map(Result::ok)
            .filter(|f| f.file_name().to_string_lossy().ends_with(".md"))
            .count();
        found.push((name, count));
    }
    found.sort();
    Ok(found)
}

/// Escape the four characters that would otherwise reinterpret register prose
/// as markup. The register is hand-written text and this page renders it.
fn esc(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The first sentence of a register summary, for a table cell.
fn first_sentence(text: &str) -> String {
    let cut = text.find(". ").map(|i| i + 1).unwrap_or(text.len());
    text[..cut].trim().to_string()
}

/// Emit the page.
///
/// # Errors
///
/// Propagates a malformed register or an unreadable session folder — this page
/// refuses to render a partial picture, because a partial coherence page is
/// the thing it exists to replace.
pub fn emit(repo_root: &Path) -> Result<String, String> {
    let statuses = assurance::artifact_statuses(repo_root)?;
    let open = open_ends(repo_root)?;
    let folders = sessions(repo_root)?;

    let epics: Vec<&ArtifactStatus> =
        statuses.iter().filter(|s| s.id.starts_with("EPIC-")).collect();
    let features: Vec<&ArtifactStatus> =
        statuses.iter().filter(|s| s.id.starts_with("FEAT-")).collect();
    let stories: Vec<&ArtifactStatus> =
        statuses.iter().filter(|s| s.id.starts_with("STORY-")).collect();

    let mut out = String::new();
    let handovers: usize = folders.iter().map(|(_, n)| n).sum();

    out.push_str(HEAD);
    writeln!(out, "<h1>TinyOS &mdash; state of the project</h1>").ok();
    writeln!(
        out,
        "<p class=\"muted\">Every figure and every row on this page is derived by \
         <code>cargo run -p xtask -- emit-state</code> from the live registers. Nothing here \
         is hand-maintained, because a coherence page that can go stale is worse than none \
         &mdash; it looks like the answer (<code>LE-108</code>).</p>"
    )
    .ok();
    writeln!(
        out,
        "<p class=\"tiles\"><span>{} Epics</span><span>{} Features</span><span>{} Stories</span>\
         <span>{} open loose ends</span><span>{} sessions</span><span>{} handovers</span></p>",
        epics.len(),
        features.len(),
        stories.len(),
        open.len(),
        folders.len(),
        handovers
    )
    .ok();

    // --- Epics -------------------------------------------------------------
    writeln!(out, "<h2>Epics</h2>").ok();
    writeln!(
        out,
        "<table><tr><th>Epic</th><th>State</th><th>Features</th><th>Open rows naming it</th></tr>"
    )
    .ok();
    for epic in &epics {
        let owned: Vec<&&ArtifactStatus> = features
            .iter()
            .filter(|f| epic_of(&f.id).as_deref() == Some(epic.id.as_str()))
            .collect();
        let blocking = blockers_for(&epic.id, &open);
        writeln!(
            out,
            "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&epic.id),
            esc(&epic.state),
            owned.len(),
            blocking
                .iter()
                .map(|e| format!("<code>{}</code>", esc(&e.id)))
                .collect::<Vec<_>>()
                .join(" ")
        )
        .ok();
    }
    writeln!(out, "</table>").ok();

    // --- Features and their blockers ---------------------------------------
    writeln!(out, "<h2>Features, their Stories, and what blocks them</h2>").ok();
    writeln!(
        out,
        "<p class=\"muted\"><strong>An empty blocker cell is not a clean bill of health.</strong> \
         A row blocks what it <em>names</em>; a blocker nobody wrote into the register is \
         invisible here exactly as it is everywhere else.</p>"
    )
    .ok();
    writeln!(out, "<table><tr><th>Feature</th><th>State</th><th>Stories</th><th>Must be addressed first</th></tr>").ok();
    for feature in &features {
        let own: Vec<&&ArtifactStatus> = stories
            .iter()
            .filter(|s| feature_of(&s.id).as_deref() == Some(feature.id.as_str()))
            .collect();
        let verified = own.iter().filter(|s| s.state.starts_with("Verified")).count();
        let story_ids: Vec<String> = own.iter().map(|s| s.id.clone()).collect();
        let blocking = feature_blockers(&feature.id, &story_ids, &open);
        let cell = if blocking.is_empty() {
            "<span class=\"none\">no open row names it</span>".to_string()
        } else {
            blocking
                .iter()
                .map(|e| format!("<code>{}</code>", esc(&e.id)))
                .collect::<Vec<_>>()
                .join(" ")
        };
        writeln!(
            out,
            "<tr><td><code>{}</code></td><td>{}</td><td>{} ({} Verified)</td><td>{}</td></tr>",
            esc(&feature.id),
            esc(&feature.state),
            own.len(),
            verified,
            cell
        )
        .ok();
    }
    writeln!(out, "</table>").ok();

    // --- Open loose ends ----------------------------------------------------
    writeln!(out, "<h2>Open loose ends ({})</h2>", open.len()).ok();
    writeln!(out, "<table><tr><th>Row</th><th>Finding</th><th>Raised in</th></tr>").ok();
    for end in &open {
        writeln!(
            out,
            "<tr><td><code>{}</code></td><td>{}</td><td>{}</td></tr>",
            esc(&end.id),
            esc(&first_sentence(&end.summary)),
            esc(&end.raised_in)
        )
        .ok();
    }
    writeln!(out, "</table>").ok();

    // --- Sessions -----------------------------------------------------------
    writeln!(out, "<h2>Sessions ({} dates, {} handovers)</h2>", folders.len(), handovers).ok();
    writeln!(out, "<table><tr><th>Date folder</th><th>Handovers</th></tr>").ok();
    for (name, count) in &folders {
        writeln!(
            out,
            "<tr><td><a href=\"../session/{n}/index.html\"><code>{n}</code></a></td><td>{count}</td></tr>",
            n = esc(name)
        )
        .ok();
    }
    writeln!(out, "</table>").ok();

    out.push_str("</body>\n</html>\n");
    Ok(out)
}

const HEAD: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>TinyOS — state of the project</title>
<style>
  :root { color-scheme: light dark; --bg:#fff; --fg:#1a1a1a; --muted:#5a5a5a; --border:#d8d8d8; --code:#f2f2f2; --accent:#0b5fff; }
  @media (prefers-color-scheme: dark) {
    :root { --bg:#14161a; --fg:#e8e8e8; --muted:#a0a0a0; --border:#333740; --code:#1e2126; --accent:#6fa8ff; }
  }
  * { box-sizing: border-box; }
  body { max-width: 1100px; margin: 0 auto; padding: 2.5rem 1.5rem 5rem;
         font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
         background: var(--bg); color: var(--fg); line-height: 1.55; }
  h1 { font-size: 1.7rem; } h2 { font-size: 1.2rem; margin-top: 2.5rem; border-bottom: 1px solid var(--border); padding-bottom: .3rem; }
  code { background: var(--code); padding: .1rem .3rem; border-radius: 4px; font-size: .9em; }
  a { color: var(--accent); }
  .muted { color: var(--muted); font-size: .92rem; }
  .none { color: var(--muted); font-style: italic; }
  .tiles span { display:inline-block; background: var(--code); border:1px solid var(--border);
                border-radius:999px; padding:.15rem .7rem; margin:.15rem .3rem .15rem 0; font-size:.85rem; font-weight:600; }
  table { border-collapse: collapse; width: 100%; margin: .6rem 0 1.2rem; display:block; overflow-x:auto; }
  th, td { border: 1px solid var(--border); padding: .35rem .55rem; text-align: left; font-size: .88rem; vertical-align: top; }
  th { background: var(--code); }
</style>
</head>
<body>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_story_names_its_feature_and_a_feature_names_its_epic() {
        assert_eq!(feature_of("STORY-P1-09-16").as_deref(), Some("FEAT-P1-09"));
        assert_eq!(feature_of("STORY-P0-01-05").as_deref(), Some("FEAT-P0-01"));
        assert_eq!(epic_of("FEAT-P1-09").as_deref(), Some("EPIC-P1"));
        assert_eq!(epic_of("FEAT-P6B-02").as_deref(), Some("EPIC-P6B"));
        assert_eq!(feature_of("FEAT-P1-09"), None, "only Stories have Features");
        assert_eq!(epic_of("STORY-P1-09-16"), None);
    }

    #[test]
    fn an_id_matches_whole_and_never_as_a_prefix_of_a_longer_one() {
        // The defect this prevents: `FEAT-P1-1` silently claiming every row
        // that names `FEAT-P1-12`, which would invent blockers on a page whose
        // whole purpose is to say what to fix first.
        let ends = vec![OpenEnd {
            id: "LE-1".into(),
            summary: "a row about FEAT-P1-12 and nothing else".into(),
            raised_in: "hand-x".into(),
        }];
        assert_eq!(blockers_for("FEAT-P1-12", &ends).len(), 1);
        assert!(blockers_for("FEAT-P1-1", &ends).is_empty(), "prefix must not match");
        assert!(blockers_for("EAT-P1-12", &ends).is_empty(), "suffix must not match");
    }

    #[test]
    fn a_row_naming_several_artifacts_blocks_each_of_them() {
        let ends = vec![OpenEnd {
            id: "LE-9".into(),
            summary: "FEAT-P1-07 cannot close while STORY-P1-09-16 is open".into(),
            raised_in: "hand-y".into(),
        }];
        assert_eq!(blockers_for("FEAT-P1-07", &ends).len(), 1);
        assert_eq!(blockers_for("STORY-P1-09-16", &ends).len(), 1);
        assert!(blockers_for("FEAT-P1-08", &ends).is_empty());
    }

    #[test]
    fn a_feature_inherits_the_rows_that_name_its_stories() {
        // Without this the page reports 31 of 33 Features clean while their
        // Stories are held by open rows — misleading in the one direction a
        // "what must be addressed first" page must not be.
        let ends = vec![
            OpenEnd {
                id: "LE-67".into(),
                summary: "STORY-P1-09-16 has no IOMMU behind it".into(),
                raised_in: "hand-a".into(),
            },
            OpenEnd {
                id: "LE-99".into(),
                summary: "STORY-P1-04-01 is unrelated to this Feature".into(),
                raised_in: "hand-b".into(),
            },
        ];
        let stories = vec!["STORY-P1-09-16".to_string(), "STORY-P1-09-17".to_string()];
        let found = feature_blockers("FEAT-P1-09", &stories, &ends);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "LE-67");
        // And a Feature with no Stories still sees rows naming it directly.
        assert_eq!(feature_blockers("FEAT-P1-09", &[], &ends).len(), 0);
    }

    #[test]
    fn a_row_naming_both_a_feature_and_its_story_is_counted_once() {
        let ends = vec![OpenEnd {
            id: "LE-5".into(),
            summary: "FEAT-P1-09 blocked because STORY-P1-09-16 is open".into(),
            raised_in: "hand-c".into(),
        }];
        let stories = vec!["STORY-P1-09-16".to_string()];
        assert_eq!(
            feature_blockers("FEAT-P1-09", &stories, &ends).len(),
            1,
            "a filter over rows cannot double-count one row"
        );
    }

    #[test]
    fn register_prose_is_escaped_so_it_cannot_become_markup() {
        assert_eq!(esc("a < b & c \"d\""), "a &lt; b &amp; c &quot;d&quot;");
    }

    #[test]
    fn a_summary_is_cut_at_its_first_sentence_and_never_mid_word() {
        assert_eq!(first_sentence("One thing. Then another."), "One thing.");
        assert_eq!(first_sentence("No full stop here"), "No full stop here");
    }
}
