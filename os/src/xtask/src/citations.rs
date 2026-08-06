//! `check-citations` — a spine id cited in Rust must resolve to a filed document.
//!
//! `LE-73`. `kernel::udp_wire` opened with ``//! ... (`STORY-P1-10-03`)`` and no
//! such Story existed: no `goals/stories/STORY-P1-10-03.md`, no contract row, no
//! Test document. So a fully implemented and tested module — 8 tests, the UDP
//! wrapper that makes the spoor stream readable on a machine with no capture
//! driver — was joined to the assurance spine by a citation that resolved to
//! nothing.
//!
//! **Why every existing gate was blind to it.** The spine gates the documents
//! that exist *against each other*: Story against Feature, Test against Story,
//! contract row against both. A source file citing an id that was never filed
//! participates in none of those relations. The register is the spine and the
//! prose is a doc comment in the code, and nothing had ever read the second and
//! checked it against the first. This is the `LE-65`/`LE-70` prose-versus-register
//! class one layer further out.
//!
//! **It found a second instance on its first run**, which is the same receipt
//! `check-lints` earned under `LE-77`: `kernel::spoor_wire` cited
//! `STORY-P1-09-16`, equally unfiled, and its real owner is `STORY-P1-10-01`
//! whose description is that module's design. One defect reported by a human
//! reading a doc comment; the second found by the gate written to close the
//! first.
//!
//! # Scope, and why it is drawn here
//!
//! **Doc comments only** (`//!` and `///`). An ordinary `//` comment is a note to
//! the next reader; a doc comment is published API text, and it is where this
//! project puts the citation that joins code to the spine. The narrower scope is
//! also what keeps the gate honest: `xtask`'s own negative tests construct
//! synthetic ids like `STORY-P9-99-99` in string literals to prove the spine
//! refuses them, and a gate that flagged its own fixtures would be a gate
//! nobody runs — `LE-72`'s lesson applied to itself.
//!
//! **Brace shorthand is skipped, and counted.** `TEST-P1-09-0{1,2,3}-A` is one
//! reference to three documents and is deliberately abbreviated. It is not
//! resolved, and the count of skipped shorthands is reported rather than
//! dropped, because a gate that silently ignores part of its subject reports on
//! a prefix while looking like it reported on all of it — `LE-77` exactly.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// The one file whose doc comments name unfiled ids on purpose.
///
/// This module documents the `LE-73` defect by naming the two ids that caused
/// it, and both are unfiled by definition — that is what made them defects. A
/// gate cannot describe its own subject and also refuse to mention it, so this
/// file is exempt from itself.
///
/// **One entry, and a test holds it to one.** An exemption list is exactly the
/// place a real defect would go to hide, so it is not permitted to grow without
/// someone deleting an assertion and explaining why.
const SELF_DOCUMENTING: [&str; 1] = ["os/src/xtask/src/citations.rs"];

/// The three id families filed as markdown documents under `goals/`.
///
/// `LE-*` is deliberately absent: loose ends live in a TSV, not as documents,
/// and `check-spine-files` already holds that register to contiguous ids.
const FAMILIES: [(&str, &str); 3] =
    [("STORY-", "goals/stories"), ("FEAT-", "goals/features"), ("TEST-", "goals/tests")];

/// One citation found in a doc comment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Citation {
    /// The cited id, e.g. `STORY-P1-10-03`.
    pub id: String,
    /// Repository-relative path of the citing file.
    pub file: String,
    /// 1-based line number.
    pub line: usize,
}

/// What the gate examined, so a caller prints evidence of coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CitationSummary {
    /// Rust files scanned.
    pub file_count: usize,
    /// Citations resolved against a filed document.
    pub citation_count: usize,
    /// Distinct ids cited.
    pub distinct_ids: usize,
    /// Brace-shorthand references skipped, reported rather than dropped.
    pub shorthand_skipped: usize,
}

/// True where `byte` can appear inside a spine id.
fn is_id_byte(byte: u8) -> bool {
    byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'-'
}

/// Extracts every spine-id citation from one file's doc comments.
///
/// Returns the citations and the number of brace shorthands skipped.
pub fn citations_in(source: &str, file: &str) -> (Vec<Citation>, usize) {
    let mut found = Vec::new();
    let mut shorthand = 0;

    for (zero_based, raw) in source.lines().enumerate() {
        let trimmed = raw.trim_start();
        // Doc comments only. `///` and `//!` both start with `//` plus one of
        // the two markers; a bare `//` is a note, not published text.
        if !(trimmed.starts_with("///") || trimmed.starts_with("//!")) {
            continue;
        }

        let bytes = raw.as_bytes();
        for (prefix, _) in FAMILIES {
            let mut from = 0;
            while let Some(offset) = raw[from..].find(prefix) {
                let start = from + offset;
                from = start + prefix.len();

                // A prefix that is itself the tail of a longer word is not a
                // citation (`SUBTEST-...` must not read as `TEST-...`).
                if start > 0 && is_id_byte(bytes[start - 1]) {
                    continue;
                }

                let mut end = start + prefix.len();
                while end < bytes.len() && is_id_byte(bytes[end]) {
                    end += 1;
                }

                // Brace shorthand: one reference to several documents,
                // deliberately abbreviated. Counted, never resolved.
                if end < bytes.len() && bytes[end] == b'{' {
                    shorthand += 1;
                    continue;
                }

                let id = &raw[start..end];
                // A bare or truncated prefix (`STORY-P1`, `FEAT-`) is prose
                // about the family, not a citation of a document.
                if !is_well_formed(id, prefix) {
                    continue;
                }

                found.push(Citation {
                    id: id.to_string(),
                    file: file.to_string(),
                    line: zero_based + 1,
                });
            }
        }
    }

    (found, shorthand)
}

/// True for a complete id of `prefix`'s family, and **exact per family**:
///
/// ```text
///   FEAT-P<n>-<nn>              FEAT-P1-10
///   STORY-P<n>-<nn>-<nn>        STORY-P1-10-03
///   TEST-P<n>-<nn>-<nn>-<A>     TEST-P1-10-03-A
/// ```
///
/// Every one of the 209 filed documents matches its family's shape with no
/// exceptions, so the grammar is derived from the register rather than guessed.
///
/// **Why exact rather than permissive.** A first version accepted "the phase then
/// two or more numeric segments", which was wrong in both directions at once. It
/// admitted `TEST-P1-09-0` — the truncated head of the brace shorthand
/// `TEST-P1-09-0{1,2,3}-A` — and, far worse, it *rejected* `FEAT-P1-10` for
/// having only two segments, so every Feature citation in the tree was silently
/// classified as prose and never resolved at all. A gate that skips a whole id
/// family while reporting success is `LE-77` in a new place, and this one was
/// caught by a unit test rather than by a reader, which is the only reason it is
/// a comment and not a loose end.
///
/// The looseness has a purpose that survives: prose legitimately names a family
/// or a partial id — "every `STORY-P1` document" — and treating those as
/// unresolved citations would make the gate cry wolf until someone turned it off.
/// Requiring the exact shape serves both ends.
fn is_well_formed(id: &str, prefix: &str) -> bool {
    let Some(rest) = id.strip_prefix(prefix) else { return false };
    let segments: Vec<&str> = rest.split('-').collect();

    let expected_segments = match prefix {
        "FEAT-" => 2,
        "STORY-" => 3,
        "TEST-" => 4,
        _ => return false,
    };
    if segments.len() != expected_segments {
        return false;
    }

    // Segment 0 is the phase: `P` and at least one digit.
    let Some(phase) = segments[0].strip_prefix('P') else { return false };
    if phase.is_empty() || !phase.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }

    // A `TEST-` id ends in a single-letter clause; every other segment is
    // exactly two digits. Two digits and not "one or more" is what refuses
    // `TEST-P1-09-0`.
    let last = segments.len() - 1;
    for (index, segment) in segments.iter().enumerate().skip(1) {
        if prefix == "TEST-" && index == last {
            let bytes = segment.as_bytes();
            if bytes.len() != 1 || !bytes[0].is_ascii_uppercase() {
                return false;
            }
        } else if segment.len() != 2 || !segment.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    true
}

/// Every filed document id, by family prefix.
fn filed_ids(repo_root: &Path) -> Result<BTreeSet<String>, String> {
    let mut ids = BTreeSet::new();
    for (prefix, directory) in FAMILIES {
        let path = repo_root.join(directory.replace('/', std::path::MAIN_SEPARATOR_STR));
        let entries = fs::read_dir(&path)
            .map_err(|error| format!("failed to read {}: {error}", directory))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("failed to walk {directory}: {error}"))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(stem) = name.strip_suffix(".md") {
                if stem.starts_with(prefix) {
                    ids.insert(stem.to_string());
                }
            }
        }
    }
    Ok(ids)
}

/// Collects every `.rs` file under `os/src`, skipping build output.
fn rust_sources(root: &Path, into: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root)
        .map_err(|error| format!("failed to read {}: {error}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to walk {}: {error}", root.display()))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if name == "target" || name == ".git" {
                continue;
            }
            rust_sources(&path, into)?;
        } else if name.ends_with(".rs") {
            into.push(path);
        }
    }
    Ok(())
}

/// Checks every spine-id citation in every Rust doc comment under `os/src`.
///
/// # Errors
///
/// Returns every unresolved citation together, rather than the first — a gate
/// that stops at one failure reports on a prefix of its subject (`LE-77`).
pub fn check_citations(repo_root: &Path) -> Result<CitationSummary, String> {
    let filed = filed_ids(repo_root)?;
    let source_root = repo_root.join("os").join("src");
    let mut files = Vec::new();
    rust_sources(&source_root, &mut files)?;
    files.sort();

    let mut unresolved: BTreeMap<String, Vec<Citation>> = BTreeMap::new();
    let mut citation_count = 0;
    let mut shorthand_skipped = 0;
    let mut distinct = BTreeSet::new();

    for path in &files {
        let relative =
            path.strip_prefix(repo_root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        let source = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {relative}: {error}"))?;
        if SELF_DOCUMENTING.contains(&relative.as_str()) {
            continue;
        }
        let (found, shorthand) = citations_in(&source, &relative);
        shorthand_skipped += shorthand;
        for citation in found {
            citation_count += 1;
            distinct.insert(citation.id.clone());
            if !filed.contains(&citation.id) {
                unresolved.entry(citation.id.clone()).or_default().push(citation);
            }
        }
    }

    if !unresolved.is_empty() {
        let mut message = format!(
            "{} cited id(s) resolve to no filed document. A doc comment is the only place this \
             project joins code to the spine, and an id that was never filed participates in \
             none of the spine's document-against-document checks (`LE-73`):",
            unresolved.len()
        );
        for (id, sites) in &unresolved {
            for site in sites {
                message.push_str(&format!("\n  {id} — cited at {}:{}", site.file, site.line));
            }
            message.push_str(
                "\n    fix: file the document, or renumber the citation to the id that does own \
                 the module. Do not delete the citation — an uncited module is the same defect \
                 with nothing left to find it by.",
            );
        }
        return Err(message);
    }

    Ok(CitationSummary {
        file_count: files.len(),
        citation_count,
        distinct_ids: distinct.len(),
        shorthand_skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf()
    }

    /// The `LE-73` defect itself, in the shape it was found in.
    #[test]
    fn a_citation_in_a_module_doc_comment_is_found() {
        let source = "//! Spoors on the wire, unformatted (`STORY-P1-10-03`).\n";
        let (found, _) = citations_in(source, "udp_wire.rs");
        assert_eq!(found.len(), 1, "the module doc citation must be extracted");
        assert_eq!(found[0].id, "STORY-P1-10-03");
        assert_eq!(found[0].line, 1);
    }

    #[test]
    fn an_item_doc_comment_is_scanned_too() {
        let source = "/// Beacon state (`STORY-P1-09-03`).\npub struct S;\n";
        let (found, _) = citations_in(source, "gem.rs");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "STORY-P1-09-03");
    }

    /// The scope decision, tested rather than merely documented: `xtask`'s own
    /// negative tests build synthetic ids to prove the spine refuses them, and a
    /// gate that flagged its own fixtures would be turned off.
    #[test]
    fn an_ordinary_comment_and_a_string_literal_are_not_citations() {
        let source = "// TEST-P0-03-01-PERF-A: a note to the next reader\n\
                      let id = \"STORY-P9-99-99\";\n";
        let (found, _) = citations_in(source, "assurance.rs");
        assert!(found.is_empty(), "only doc comments are citations, found {found:?}");
    }

    /// `TEST-P1-09-0{1,2,3}-A` in `hal-arm64::gem` — one reference to three
    /// documents. Skipped, and the skip is counted so it cannot be silent.
    #[test]
    fn brace_shorthand_is_skipped_and_counted_not_silently_dropped() {
        let source = "//! `STORY-P1-09-03` (beacon) — `TEST-P1-09-0{1,2,3}-A`.\n";
        let (found, shorthand) = citations_in(source, "gem.rs");
        assert_eq!(shorthand, 1, "the shorthand must be counted");
        assert_eq!(found.len(), 1, "the well-formed sibling citation still resolves");
        assert_eq!(found[0].id, "STORY-P1-09-03");
    }

    /// Prose about a family is not a citation of a document. Without this the
    /// gate cries wolf on every sentence naming a phase and gets disabled.
    #[test]
    fn a_bare_or_partial_prefix_is_prose_not_a_citation() {
        let source = "//! Every STORY-P1 document, and the FEAT- family generally.\n";
        let (found, _) = citations_in(source, "lib.rs");
        assert!(found.is_empty(), "partial ids are prose, found {found:?}");
    }

    /// Each family's exact shape, and the two defects a permissive grammar had:
    /// it admitted the truncated `TEST-P1-09-0` and rejected every `FEAT-` id.
    #[test]
    fn each_family_has_its_own_exact_shape() {
        assert!(is_well_formed("TEST-P1-10-03-A", "TEST-"));
        assert!(is_well_formed("STORY-P1-10-03", "STORY-"));
        assert!(is_well_formed("FEAT-P1-10", "FEAT-"), "a two-segment Feature id is well-formed");

        assert!(!is_well_formed("STORY-P1", "STORY-"));
        assert!(!is_well_formed("TEST-P1-09-0", "TEST-"), "the brace-shorthand truncation");
        assert!(!is_well_formed("STORY-PX-01-01", "STORY-"));
        // A Story shape is not a Test shape and vice versa.
        assert!(!is_well_formed("STORY-P1-10-03-A", "STORY-"));
        assert!(!is_well_formed("TEST-P1-10-03", "TEST-"));
        assert!(!is_well_formed("FEAT-P1-10-03", "FEAT-"));
    }

    /// The regression that matters most: every Feature citation must actually be
    /// resolved, not silently classified as prose.
    #[test]
    fn a_feature_citation_is_resolved_and_not_skipped_as_prose() {
        let source = "//! Implements `FEAT-P1-10` on the board.\n";
        let (found, _) = citations_in(source, "gem.rs");
        assert_eq!(found.len(), 1, "a Feature id must be extracted, found {found:?}");
        assert_eq!(found[0].id, "FEAT-P1-10");
    }

    /// The grammar is derived from the register, so it must still describe it.
    /// If a future id shape is filed, this fails rather than that id going
    /// unchecked.
    #[test]
    fn every_filed_document_id_matches_its_family_shape() {
        let filed = filed_ids(&repo_root()).expect("the goals tree is readable");
        assert!(filed.len() > 200, "209 documents were filed when this was written");
        for id in &filed {
            let prefix = FAMILIES
                .iter()
                .map(|(prefix, _)| *prefix)
                .find(|prefix| id.starts_with(prefix))
                .expect("a filed id carries a family prefix");
            assert!(
                is_well_formed(id, prefix),
                "{id} is filed but the citation grammar would read it as prose, so a citation \
                 of it would never be resolved"
            );
        }
    }

    /// A prefix that is the tail of a longer word must not read as a citation.
    #[test]
    fn a_prefix_inside_a_longer_token_is_not_a_citation() {
        let source = "//! See SUBTEST-P1-10-03 elsewhere.\n";
        let (found, _) = citations_in(source, "lib.rs");
        assert!(found.is_empty(), "found {found:?}");
    }

    /// The exemption list is the one place a real defect could hide, so its size
    /// is asserted. Growing it requires deleting this assertion deliberately.
    #[test]
    fn only_this_module_is_exempt_from_itself() {
        assert_eq!(
            SELF_DOCUMENTING,
            ["os/src/xtask/src/citations.rs"],
            "an exemption list that grows quietly is a place for the next LE-73 to hide"
        );
    }

    /// The gate's own subject: the committed tree must pass. This is the
    /// assertion that failed when written — `udp_wire` cited an unfiled
    /// `STORY-P1-10-03` and `spoor_wire` an unfiled `STORY-P1-09-16`.
    #[test]
    fn the_committed_tree_has_no_unresolved_citation() {
        let summary = check_citations(&repo_root()).expect("every cited id must be filed");
        assert!(summary.citation_count > 100, "the tree cites more than a hundred ids");
        assert!(summary.file_count > 50, "os/src holds more than fifty Rust files");
    }
}
