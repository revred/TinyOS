//! `check-metric-labels` — a fixture metric's domain must be selected by the
//! Story it names.
//!
//! `LE-91`. Nothing machine-checked which performance domain a fixture metric
//! was labelled with, so a metric could be labelled to fit the contract its
//! Story happened to select rather than to name its own subject — and when
//! that happens the number is never read against the gate whose subject it is.
//!
//! **The demonstrated instance.** From 2026-08-05 to 2026-08-06
//! `fixture_measure_arm64` emitted `spoor_stamp_park_rung_per_op_of_8`,
//! `spoor_drain_full_ring_frame_of_181` and
//! `spoor_announce_certificate_frame_of_3` as `domain=D07`, because
//! `STORY-P1-10-02`'s contract selected only `D07`. `D07` is fixed-capacity
//! pool allocation; `D11` is spoor stamp and journal, which is exactly and
//! only what those three measure. The numbers reached the wire, a Report, a
//! handover and the Story's own status header, and were **never once compared
//! to `D11`'s targets** — which the stamp misses by 1.9× at the median
//! (`PERF-D11-G01`). A wrong label is not a naming defect; it is an unread
//! gate.
//!
//! # Why the obvious check would have been wrong
//!
//! *"A fixture's domains must be a subset of its owning Story's contract"* is
//! false and would have been wrong to assert. One fixture serves six domains
//! at once — `fixture_measure_arm64` emits `REF`, `D02`, `D04`, `D05`, `D07`
//! and `D11` — while `list-fixtures` maps a whole fixture to **one** owning
//! `TEST`. The unit that has a single owner is the *metric*, not the fixture,
//! so that is the unit this gate checks.
//!
//! # What is actually checked
//!
//! Each metric declares its domain **and** its owning Story at one site, in a
//! [`kernel::measure::MetricLabel`] table beside the code that emits it. This
//! module parses those declarations out of the fixture sources — rather than
//! keeping a copy of them, which is how `LE-80`'s host-side mirror came apart
//! — and asserts:
//!
//! 1. every declared Story has a row in `story-contracts.tsv`;
//! 2. every declared domain is either `REF` or a well-formed `D01`..`D25`;
//! 3. every `Dnn` is **selected by the named Story's contract**;
//! 4. metric names are unique within one fixture (they must be, within one
//!    report);
//! 5. **no `Metric` is constructed outside a declaration** — the completeness
//!    half, without which a new fixture could emit an unlabelled metric and
//!    this gate would pass while covering none of it (`LE-77`);
//! 6. a `guardrail-evidence.tsv` row whose note names any declared metric
//!    names **at least one metric of its own domain**.
//!
//! Rule 3 is the same rule `check-assurance-spine` already applies to
//! `guardrail-evidence.tsv` — a Story may only record evidence in a domain its
//! contract selects — moved one level earlier, from where a number is *filed*
//! to where it is *produced*.
//!
//! **A failure does not always mean the label is wrong.** If the domain names
//! the subject correctly and the Story does not select it, the contract is
//! what is wrong. Bending the label back to fit is the defect this gate
//! exists to prevent, and the error message says so.
//!
//! # Rules 1–3 would not have caught the instance that produced `LE-91`
//!
//! Stated plainly, because a gate believed to cover more than it does is
//! worse than no gate. The bent label was **consistent with its contract**:
//! `D07`, under a Story whose contract selected only `D07`. A rule that holds
//! a label against a contract has nothing to object to there — which is the
//! whole shape of the defect, a domain chosen *from* what the contract already
//! allowed.
//!
//! What was inconsistent was the label against **the gate the number was
//! filed as evidence for**: `PERF-D11-G01`, `G02` and `G03` were every one of
//! them read from `spoor_stamp_park_rung_per_op_of_8` while that metric said
//! `D07`. That disagreement is mechanical, and rule 6 is it. Rules 1–3 remove
//! the *incentive* — a domain is no longer picked at a site where the contract
//! is the only nearby constraint — and rule 6 catches the case where someone
//! picks wrongly anyway and then files the number against the right gate.
//!
//! Neither rule can decide that a domain names a metric's true subject. That
//! is a judgement about what the code measures, and no text scan holds it.
//!
//! # What this gate cannot see, stated rather than implied
//!
//! It is a **text scan, not a call graph** — the same limit `LE-99`'s guard
//! records about itself. Specifically:
//!
//! - A `Metric` built by a macro expansion, or by a function that takes a
//!   computed `&str`, is invisible to rule 5. What rule 5 catches is the
//!   realistic shape: a struct literal written by hand at an emit site, which
//!   is how all six fixtures did it before `LE-91`.
//! - A renaming re-export (`use kernel::measure::Metric as M;`) slips the
//!   needle.
//! - Code inside a `#[cfg(test)]` region is deliberately out of scope: host
//!   tests construct `Metric` with synthetic names to exercise the envelope
//!   writer, and a gate that flagged its own subject's tests would be a gate
//!   nobody runs (`LE-72`'s lesson, applied to itself).
//!
//! The declarations themselves have no such hole: the fixture reads its label
//! *from* the table this module parses, so a declaration and the emitted
//! `METRIC` line cannot disagree.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// The one file whose *code* names `MetricLabel {` on purpose.
///
/// This module's error messages have to quote the shape they are about —
/// ``format!("`MetricLabel {{` is never closed")`` is a string literal, not a
/// declaration — and dropping comment tails does not reach inside a string.
/// A gate cannot describe its own subject and also refuse to mention it, so
/// this file is exempt from itself; `citations.rs` carries the identical
/// exemption for the identical reason.
///
/// **One entry, and a test holds it to one.** An exemption list is exactly
/// where a real defect would go to hide.
const SELF_DOCUMENTING: [&str; 1] = ["os/src/xtask/src/metric_labels.rs"];

/// The one place a `Metric` may be built from a struct literal, and how many
/// times.
///
/// [`kernel::measure`] defines the type and holds `Metric::labelled`, which is
/// the sanctioned constructor every fixture goes through — that function's
/// body is necessarily a struct literal. Exempting the *file* outright would
/// leave a stray literal beside it unchecked, so the count is pinned: one, and
/// a second one fails this gate rather than joining a quiet allowlist.
const SANCTIONED_CONSTRUCTION: [(&str, usize); 1] = [("os/src/kernel/src/measure.rs", 1)];

/// The reference denominator's pseudo-domain.
///
/// `REF` is not one of the catalogue's 25 domains and has no target column of
/// its own — it is the fixed-integer loop every ratio is divided by. It is
/// exempt from the contract check **by name and by test**, rather than by
/// falling through a `Dnn` shape check into silence, because a domain-shaped
/// exemption that nothing asserts is where the next mislabel would hide.
const REFERENCE_DOMAIN: &str = "REF";

/// The 25-domain axis of `goals/performance/catalogue.tsv`.
const AXIS_SIZE: usize = 25;

/// One `MetricLabel { .. }` declaration read out of a fixture source.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Declaration {
    /// Repository-relative path of the declaring file.
    pub file: String,
    /// 1-based line of the `MetricLabel` token.
    pub line: usize,
    /// `Dnn`, or [`REFERENCE_DOMAIN`].
    pub domain: String,
    /// The `STORY-Pn-NN-NN` whose contract must select `domain`.
    pub story: String,
    /// The metric name as it appears on the `METRIC` line.
    pub name: String,
}

/// A `Metric { .. }` built somewhere no declaration covers — rule 5's finding.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnlabelledSite {
    /// Repository-relative path.
    pub file: String,
    /// 1-based line.
    pub line: usize,
}

/// What the gate examined, so a caller prints evidence of coverage rather
/// than a bare "ok".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricLabelSummary {
    /// Rust files scanned.
    pub file_count: usize,
    /// Files declaring at least one metric.
    pub declaring_file_count: usize,
    /// Declarations checked.
    pub declaration_count: usize,
    /// Distinct Stories named by those declarations.
    pub story_count: usize,
    /// Distinct domains named, `REF` included.
    pub domain_count: usize,
}

/// True where `byte` can appear inside a Rust identifier.
fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Drops a line's `//` comment tail, respecting double-quoted strings.
///
/// **Comments are prose, and this gate reads code.** Without this, the very
/// first run failed on *this module's own doc comment*, which necessarily
/// writes `MetricLabel { .. }` to describe what it parses — the trap `09C`
/// records at two nesting depths: a source-level scan that matches its own
/// text finds the description, not the subject. `citations.rs` solved the
/// mirror image of this by scanning doc comments **only**; this one scans
/// everything but.
///
/// `/* */` block comments are not handled and are not used in this tree; a
/// block comment containing `Metric {` would produce a false finding, which
/// fails *loud* and is the acceptable direction.
fn without_comment_tail(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' if in_string => index += 1,
            b'"' => in_string = !in_string,
            b'/' if !in_string && index + 1 < bytes.len() && bytes[index + 1] == b'/' => {
                return &line[..index];
            }
            _ => {}
        }
        index += 1;
    }
    line
}

/// The code this gate reads: every `#[cfg(test)]` region blanked and every
/// comment tail dropped, with line numbers preserved.
///
/// A test region runs from the attribute to the first line that is exactly the
/// attribute's own indentation followed by `}`. Every `#[cfg(test)]` in this
/// tree is a module written that way, and a region that never closes simply
/// runs to end of file — which errs towards scanning *less*, so it can hide a
/// finding but never invent one.
fn scannable_code(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut closing: Option<String> = None;
    for line in source.lines() {
        match &closing {
            Some(terminator) => {
                let ended = line.trim_end() == terminator.as_str();
                out.push('\n');
                if ended {
                    closing = None;
                }
            }
            None => {
                if line.trim() == "#[cfg(test)]" {
                    let indent: String =
                        line.chars().take_while(|character| character.is_whitespace()).collect();
                    closing = Some(format!("{indent}}}"));
                    out.push('\n');
                } else {
                    out.push_str(without_comment_tail(line));
                    out.push('\n');
                }
            }
        }
    }
    out
}

/// True when the identifier at `start` is the name in a `struct` *definition*
/// rather than in a literal.
///
/// `pub struct MetricLabel {` and `MetricLabel { domain: .. }` are the same
/// three tokens apart, and the definition is not a declaration of anything
/// measured. The first version of this gate did not distinguish them and
/// reported `kernel::measure`'s own type as a `MetricLabel` with no `domain`.
fn is_struct_definition(source: &str, start: usize) -> bool {
    source[..start].trim_end().ends_with("struct")
}

/// Finds every occurrence of the identifier `token` immediately followed (past
/// whitespace) by `{`, as a byte offset into `source`. Definitions are not
/// literals, and are skipped.
fn struct_literal_offsets(source: &str, token: &str) -> Vec<usize> {
    let bytes = source.as_bytes();
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = source[from..].find(token) {
        let start = from + offset;
        from = start + token.len();
        // Identifier boundaries on both sides: `MetricLabel` must not read as
        // `Metric`, and `SomeMetric` must not either.
        if start > 0 && is_ident_byte(bytes[start - 1]) {
            continue;
        }
        if is_struct_definition(source, start) {
            continue;
        }
        let mut after = start + token.len();
        while after < bytes.len() && (bytes[after] as char).is_whitespace() {
            after += 1;
        }
        if after < bytes.len() && bytes[after] == b'{' {
            found.push(start);
        }
    }
    found
}

/// 1-based line number of `offset`.
fn line_of(source: &str, offset: usize) -> usize {
    source[..offset].bytes().filter(|byte| *byte == b'\n').count() + 1
}

/// Reads `field: "value"` out of the braced block starting at `open`.
fn field_in_block(block: &str, field: &str) -> Option<String> {
    let needle = format!("{field}:");
    let mut from = 0;
    while let Some(offset) = block[from..].find(&needle) {
        let start = from + offset;
        from = start + needle.len();
        if start > 0 && is_ident_byte(block.as_bytes()[start - 1]) {
            continue;
        }
        let rest = &block[start + needle.len()..];
        let quote = rest.find('"')?;
        let tail = &rest[quote + 1..];
        let end = tail.find('"')?;
        return Some(tail[..end].to_string());
    }
    None
}

/// The braced block beginning at the `{` that follows `start`, exclusive of
/// the braces, or `None` if it never closes.
fn block_after(source: &str, start: usize) -> Option<&str> {
    let open = start + source[start..].find('{')?;
    let mut depth = 0usize;
    for (index, byte) in source[open..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&source[open + 1..open + index]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Extracts every declaration, and every `Metric` construction no declaration
/// covers, from one file's non-test code.
///
/// # Errors
///
/// A `MetricLabel` block missing any of its three fields — the shape a
/// half-finished edit leaves behind — is an error rather than a skipped row.
/// Reporting on a prefix of the subject while looking like a full pass is
/// `LE-77`.
pub fn declarations_in(
    source: &str,
    file: &str,
) -> Result<(Vec<Declaration>, Vec<UnlabelledSite>), String> {
    let scanned = scannable_code(source);

    let mut declarations = Vec::new();
    for start in struct_literal_offsets(&scanned, "MetricLabel") {
        let line = line_of(&scanned, start);
        let block = block_after(&scanned, start)
            .ok_or_else(|| format!("{file}:{line}: `MetricLabel {{` is never closed"))?;
        let field = |name: &str| {
            field_in_block(block, name).ok_or_else(|| {
                format!(
                    "{file}:{line}: this `MetricLabel` declares no `{name}`. All three of \
                     `domain`, `story` and `name` are required — a declaration missing one is \
                     the shape a half-finished edit leaves, and skipping it would report a pass \
                     over a metric nothing checked"
                )
            })
        };
        declarations.push(Declaration {
            file: file.to_string(),
            line,
            domain: field("domain")?,
            story: field("story")?,
            name: field("name")?,
        });
    }

    let unlabelled = struct_literal_offsets(&scanned, "Metric")
        .into_iter()
        .map(|start| UnlabelledSite { file: file.to_string(), line: line_of(&scanned, start) })
        .collect();

    Ok((declarations, unlabelled))
}

/// One `guardrail-evidence.tsv` row, reduced to what rule 6 reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRow {
    /// `PERF-Dnn-Gnn`.
    pub guardrail: String,
    /// The row's own domain.
    pub domain: String,
    /// The `note` column, where the register names the metric it was read from.
    pub note: String,
}

/// Reads the guardrail evidence register.
///
/// **Columns are located by header name, not by index.** That register gains
/// columns — four condition columns landed on 2026-08-06 — and a positional
/// reader would either break loudly or, worse, quietly read the wrong field
/// and compare metric names against a column that never holds them.
fn evidence_rows(repo_root: &Path) -> Result<Vec<EvidenceRow>, String> {
    let path = repo_root.join("goals").join("assurance").join("guardrail-evidence.tsv");
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut lines = contents.lines();
    let header: Vec<&str> = lines
        .next()
        .ok_or_else(|| "guardrail-evidence.tsv is empty".to_string())?
        .trim_end_matches('\r')
        .split('\t')
        .collect();
    let column = |name: &str| {
        header.iter().position(|field| *field == name).ok_or_else(|| {
            format!("guardrail-evidence.tsv has no `{name}` column; its header changed shape")
        })
    };
    let (guardrail_at, domain_at, note_at) =
        (column("guardrail_id")?, column("domain")?, column("note")?);

    let mut rows = Vec::new();
    for raw_line in lines {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != header.len() {
            return Err(format!(
                "guardrail-evidence.tsv row `{}` has {} fields against a {}-column header; run \
                 `cargo run -p xtask -- check-spine-files`",
                fields.first().copied().unwrap_or_default(),
                fields.len(),
                header.len()
            ));
        }
        rows.push(EvidenceRow {
            guardrail: fields[guardrail_at].to_string(),
            domain: fields[domain_at].to_string(),
            note: fields[note_at].to_string(),
        });
    }
    Ok(rows)
}

/// True when `name` occurs in `haystack` as a whole identifier.
fn names_metric(haystack: &str, name: &str) -> bool {
    let bytes = haystack.as_bytes();
    let mut from = 0;
    while let Some(offset) = haystack[from..].find(name) {
        let start = from + offset;
        from = start + name.len();
        let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
        let after = start + name.len();
        // The trailing boundary is what keeps
        // `pool_u64x64_alloc_free_round_trip_per_op_of_8` from matching inside
        // its own `_spoored` twin — two different metrics whose names are one
        // a prefix of the other.
        let after_ok = after >= bytes.len() || !is_ident_byte(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// Maps each Story to the performance domains its contract row selects.
fn selected_domains(repo_root: &Path) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let path = repo_root.join("goals").join("assurance").join("story-contracts.tsv");
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut selections = BTreeMap::new();
    for raw_line in contents.lines().skip(1) {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 3 {
            return Err(format!(
                "story-contracts.tsv row `{}` has fewer than three fields; run \
                 `cargo run -p xtask -- check-spine-files`",
                fields.first().copied().unwrap_or_default()
            ));
        }
        selections.insert(
            fields[0].to_string(),
            fields[2].split(',').map(str::to_string).collect::<BTreeSet<String>>(),
        );
    }
    Ok(selections)
}

/// True for `D01`..`D25`.
fn is_domain_id(value: &str) -> bool {
    let Some(digits) = value.strip_prefix('D') else { return false };
    digits.len() == 2
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && matches!(digits.parse::<usize>(), Ok(number) if (1..=AXIS_SIZE).contains(&number))
}

/// Collects every `.rs` file under `root`, skipping build output.
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

/// Checks every declared metric label under `os/src` against
/// `goals/assurance/story-contracts.tsv`.
///
/// # Errors
///
/// Returns every finding together rather than the first: a gate that stops at
/// one failure reports on a prefix of its subject (`LE-77`).
pub fn check_metric_labels(repo_root: &Path) -> Result<MetricLabelSummary, String> {
    let selections = selected_domains(repo_root)?;
    let source_root = repo_root.join("os").join("src");
    let mut files = Vec::new();
    rust_sources(&source_root, &mut files)?;
    files.sort();

    let mut declarations = Vec::new();
    let mut unlabelled = Vec::new();
    let mut declaring_files = BTreeSet::new();
    for path in &files {
        let relative =
            path.strip_prefix(repo_root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        if SELF_DOCUMENTING.contains(&relative.as_str()) {
            continue;
        }
        let source = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {relative}: {error}"))?;
        let (found, mut sites) = declarations_in(&source, &relative)?;
        if !found.is_empty() {
            declaring_files.insert(relative.clone());
        }
        if let Some((_, sanctioned)) =
            SANCTIONED_CONSTRUCTION.iter().find(|(file, _)| *file == relative)
        {
            if sites.len() != *sanctioned {
                return Err(format!(
                    "{relative} holds {} `Metric` struct literal(s); exactly {sanctioned} is \
                     sanctioned (`Metric::labelled`'s own body). A literal beside it would be a \
                     metric built where nothing reads its domain, and a *missing* one means the \
                     sanctioned constructor moved and this exemption now covers something else",
                    sites.len()
                ));
            }
            sites.clear();
        }
        declarations.extend(found);
        unlabelled.extend(sites);
    }

    let evidence = evidence_rows(repo_root)?;
    check_against(&declarations, &unlabelled, &selections, &evidence)?;

    Ok(MetricLabelSummary {
        file_count: files.len(),
        declaring_file_count: declaring_files.len(),
        declaration_count: declarations.len(),
        story_count: declarations.iter().map(|d| d.story.clone()).collect::<BTreeSet<_>>().len(),
        domain_count: declarations.iter().map(|d| d.domain.clone()).collect::<BTreeSet<_>>().len(),
    })
}

/// The five rules, over already-parsed inputs.
///
/// Separated from [`check_metric_labels`] so each rule is exercised against a
/// constructed input rather than only against the committed tree: a gate whose
/// only test is "the tree passes" cannot distinguish a rule that works from a
/// rule that never fires.
///
/// # Errors
///
/// Returns every finding together rather than the first (`LE-77`).
fn check_against(
    declarations: &[Declaration],
    unlabelled: &[UnlabelledSite],
    selections: &BTreeMap<String, BTreeSet<String>>,
    evidence: &[EvidenceRow],
) -> Result<(), String> {
    let mut problems: Vec<String> = Vec::new();

    // Rule 4: names are unique within one fixture, because they must be unique
    // within one report and a fixture emits one report.
    let mut names_by_file: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for declaration in declarations {
        if !names_by_file.entry(&declaration.file).or_default().insert(&declaration.name) {
            problems.push(format!(
                "{}:{}: `{}` is declared twice in one fixture, so one report would carry two \
                 `METRIC` lines with the same name",
                declaration.file, declaration.line, declaration.name
            ));
        }
    }

    for declaration in declarations {
        let Declaration { file, line, domain, story, name } = declaration;

        let Some(selected) = selections.get(story) else {
            problems.push(format!(
                "{file}:{line}: `{name}` names `{story}`, which has no row in \
                 goals/assurance/story-contracts.tsv. A metric owned by a Story that does not \
                 exist is owned by nobody"
            ));
            continue;
        };

        if domain == REFERENCE_DOMAIN {
            // The reference denominator: not a catalogue domain, and it has no
            // target column, so there is nothing to read it against. Exempt by
            // name, and asserted so by a test.
            continue;
        }

        if !is_domain_id(domain) {
            problems.push(format!(
                "{file}:{line}: `{name}` declares domain `{domain}`, which is neither \
                 `{REFERENCE_DOMAIN}` nor a `D01`..`D{AXIS_SIZE}` id"
            ));
            continue;
        }

        if !selected.contains(domain) {
            let selected_list: Vec<&str> = selected.iter().map(String::as_str).collect();
            problems.push(format!(
                "{file}:{line}: `{name}` is labelled `{domain}` but `{story}`'s contract selects \
                 {}. A domain label decides which target column the number is read against, so \
                 this metric would be published under a gate nobody joins it to (`LE-91`).\n    \
                 fix: if `{domain}` names what is measured, extend `{story}`'s \
                 `performance_domains` and say why in its rationale. Re-labelling the metric to \
                 fit the contract is the defect this gate exists to catch — that is exactly how \
                 three spoor metrics carried `D07` for two days while missing `D11`'s median \
                 target by 1.9x.",
                selected_list.join(",")
            ));
        }
    }

    // Rule 5, the completeness half: a `Metric` built anywhere but from a
    // declaration means this gate covered a prefix of the fixtures while
    // looking like it covered all of them.
    for site in unlabelled {
        problems.push(format!(
            "{}:{}: a `Metric` is constructed here rather than built from a declared \
             `MetricLabel`. Build it with `Metric::labelled(&METRIC_LABELS[n], warmup, summary)` \
             and declare the label beside the fixture, or this metric's domain is chosen at an \
             emit site where nothing reads it (`LE-91`)",
            site.file, site.line
        ));
    }

    // Rule 6, and the one that would actually have caught `LE-91`'s own
    // demonstrated instance.
    //
    // Rules 1–3 hold a label against a *contract*, and in the demonstrated
    // defect the bent label was **consistent** with its contract — `D07` under
    // a Story that selected only `D07`. What was inconsistent was the label
    // against the *gate the number was filed as evidence for*:
    // `PERF-D11-G01`, `G02` and `G03` were all read from
    // `spoor_stamp_park_rung_per_op_of_8` while that metric said `D07`. So a
    // guardrail-evidence row that names a declared metric must agree with that
    // metric's declared domain.
    //
    // `REF` metrics are exempt: the reference denominator is quotable in any
    // domain's row by construction, because every ratio divides by it.
    let by_name: BTreeMap<&str, &Declaration> =
        declarations.iter().map(|declaration| (declaration.name.as_str(), declaration)).collect();
    for row in evidence {
        let named: Vec<&Declaration> = by_name
            .iter()
            .filter(|(name, declaration)| {
                declaration.domain != REFERENCE_DOMAIN && names_metric(&row.note, name)
            })
            .map(|(_, declaration)| *declaration)
            .collect();
        // **Some**, not every. A note legitimately cites another domain's
        // metric as an explanatory quantity — `PERF-D04-G23`'s does, quoting
        // the `D11` stamp cost to explain where its 110-cycle delta comes
        // from — and a rule demanding that *every* named metric match the row
        // would flag that correct row and get itself switched off. What is
        // never legitimate is a gate whose evidence names measured metrics and
        // **not one of its own domain**: that is a target column read against
        // somebody else's number, which is `LE-91` exactly.
        if named.is_empty() || named.iter().any(|declaration| declaration.domain == row.domain) {
            continue;
        }
        let cited: Vec<String> = named
            .iter()
            .map(|declaration| {
                format!(
                    "`{}` ({} at {}:{})",
                    declaration.name, declaration.domain, declaration.file, declaration.line
                )
            })
            .collect();
        problems.push(format!(
            "{} is a `{}` guardrail whose evidence names {} and no `{}` metric at all. A gate \
             read from a metric labelled for another domain is a number compared to a target \
             column that is not its own — exactly how `PERF-D11-G01` came to be filed from \
             `spoor_stamp_park_rung_per_op_of_8` while that metric still said `D07` (`LE-91`).\n \
             fix: correct whichever of the two is wrong. If the metric's label is right, the \
             evidence row is filed under the wrong gate; if the gate is right, the metric is \
             mislabelled and its Story's contract needs the real domain.",
            row.guardrail,
            row.domain,
            cited.join(", "),
            row.domain
        ));
    }

    if !problems.is_empty() {
        let mut message = format!("{} metric label problem(s):", problems.len());
        for problem in &problems {
            message.push_str("\n  ");
            message.push_str(problem);
        }
        return Err(message);
    }

    Ok(())
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

    #[test]
    fn a_single_line_declaration_yields_all_three_fields() {
        let source = concat!(
            "static METRIC_LABELS: [MetricLabel; 1] = [\n",
            "    MetricLabel { domain: \"D11\", story: \"STORY-P1-10-02\", name: \"stamp\" },\n",
            "];\n"
        );
        let (found, _) = declarations_in(source, "fixture.rs").expect("parses");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].domain, "D11");
        assert_eq!(found[0].story, "STORY-P1-10-02");
        assert_eq!(found[0].name, "stamp");
        assert_eq!(found[0].line, 2);
    }

    /// `rustfmt` breaks a long row across four lines, so a line-oriented
    /// parser would read the committed tree as having no declarations at all
    /// and pass — the vacuous green this project has shipped twice.
    #[test]
    fn a_declaration_rustfmt_split_across_lines_is_still_one_declaration() {
        let source = concat!(
            "    MetricLabel {\n",
            "        domain: \"D07\",\n",
            "        story: \"STORY-P1-07-06\",\n",
            "        name: \"pool_u64x64_alloc_free_round_trip_per_op_of_8_spoored\",\n",
            "    },\n"
        );
        let (found, _) = declarations_in(source, "fixture.rs").expect("parses");
        assert_eq!(found.len(), 1, "found {found:?}");
        assert_eq!(found[0].domain, "D07");
        assert_eq!(found[0].name, "pool_u64x64_alloc_free_round_trip_per_op_of_8_spoored");
    }

    /// A half-finished edit must fail loud. Skipping the row would report a
    /// pass over a metric nothing checked.
    #[test]
    fn a_declaration_missing_a_field_is_an_error_not_a_skip() {
        let source = "MetricLabel { domain: \"D07\", name: \"x\" };\n";
        let error = declarations_in(source, "fixture.rs").expect_err("must refuse");
        assert!(error.contains("story"), "{error}");
    }

    /// `MetricLabel` must not read as `Metric` — otherwise every declaration
    /// would also be reported as an unlabelled construction and the gate would
    /// cry wolf on its own subject.
    #[test]
    fn a_declaration_is_not_also_counted_as_an_unlabelled_metric() {
        let source = "MetricLabel { domain: \"D07\", story: \"STORY-P0-03-01\", name: \"x\" };\n";
        let (found, unlabelled) = declarations_in(source, "fixture.rs").expect("parses");
        assert_eq!(found.len(), 1);
        assert!(unlabelled.is_empty(), "found {unlabelled:?}");
    }

    /// Rule 5's subject, in the shape every fixture used before `LE-91`.
    #[test]
    fn a_metric_struct_literal_is_reported_as_unlabelled() {
        let source = concat!(
            "report.metric(&Metric {\n",
            "    domain: \"D07\",\n",
            "    name: \"whatever\",\n",
            "    warmup: 0,\n",
            "    summary,\n",
            "});\n"
        );
        let (_, unlabelled) = declarations_in(source, "fixture.rs").expect("parses");
        assert_eq!(unlabelled.len(), 1, "found {unlabelled:?}");
        assert_eq!(unlabelled[0].line, 1);
    }

    /// `Metric::labelled(..)` is the sanctioned construction and must not be
    /// mistaken for a struct literal.
    #[test]
    fn the_labelled_constructor_is_not_an_unlabelled_site() {
        let source = "report.metric(&Metric::labelled(&METRIC_LABELS[0], WARMUP, summary));\n";
        let (_, unlabelled) = declarations_in(source, "fixture.rs").expect("parses");
        assert!(unlabelled.is_empty(), "found {unlabelled:?}");
    }

    /// Trap: a source-level scan that reads its own file's tests finds the
    /// test rather than the code. Host tests build `Metric` with synthetic
    /// names on purpose, and flagging them would get this gate switched off.
    #[test]
    fn a_metric_inside_a_cfg_test_region_is_out_of_scope() {
        let source = concat!(
            "fn emit() {}\n",
            "\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    fn t() {\n",
            "        let m = Metric { domain: \"D04\", name: \"a\", warmup: 0, summary };\n",
            "    }\n",
            "}\n",
            "fn after() { let m = Metric { domain: \"D05\", name: \"b\" }; }\n"
        );
        let (_, unlabelled) = declarations_in(source, "measure.rs").expect("parses");
        assert_eq!(unlabelled.len(), 1, "only the post-test site counts: {unlabelled:?}");
        assert_eq!(unlabelled[0].line, 9, "the region must close at its own indentation");
    }

    /// Blanking a test region must not shift the line numbers reported for
    /// everything after it, or every finding past the first test module points
    /// a reader at the wrong line.
    #[test]
    fn blanking_a_test_region_preserves_line_numbers() {
        let source = "a\n#[cfg(test)]\nmod t {\n}\nz\n";
        let scanned = scannable_code(source);
        assert_eq!(scanned.lines().count(), source.lines().count());
        assert_eq!(scanned.lines().nth(4), Some("z"));
    }

    /// The trap this gate sprang on itself on its first run: a doc comment
    /// describing `MetricLabel { .. }` is prose about the subject, not the
    /// subject. A scan that matches its own description finds the description.
    #[test]
    fn a_metric_named_in_a_comment_is_prose_not_code() {
        let source = concat!(
            "/// One `MetricLabel { .. }` declaration, and a `Metric { .. }` beside it.\n",
            "// let m = Metric { domain: \"D07\" };\n",
            "pub struct Declaration {}\n"
        );
        let (found, unlabelled) = declarations_in(source, "metric_labels.rs").expect("parses");
        assert!(found.is_empty(), "found {found:?}");
        assert!(unlabelled.is_empty(), "found {unlabelled:?}");
    }

    /// A `//` inside a string literal does not start a comment, so the rest of
    /// the line must still be scanned.
    #[test]
    fn a_double_slash_inside_a_string_does_not_start_a_comment() {
        let line = "let url = \"http://x\"; let m = Metric { domain: \"D07\" };";
        assert_eq!(without_comment_tail(line), line);
    }

    /// A type *definition* is not a literal. The first run of this gate
    /// reported `kernel::measure`'s own `pub struct MetricLabel {` as a
    /// declaration with no `domain`.
    #[test]
    fn a_struct_definition_is_not_a_literal() {
        let source = "pub struct MetricLabel {\n    pub domain: &'static str,\n}\n";
        let (found, _) = declarations_in(source, "measure.rs").expect("parses");
        assert!(found.is_empty(), "found {found:?}");
    }

    /// The `LE-91` defect itself: `D11` under a contract that selects only
    /// `D07` is what the three spoor metrics were bent away from.
    #[test]
    fn a_domain_the_named_story_does_not_select_is_refused() {
        let mut selections = BTreeMap::new();
        selections.insert("STORY-P1-10-02".to_string(), BTreeSet::from(["D07".to_string()]));
        let error = check_against(
            &[Declaration {
                file: "fixture_measure_arm64.rs".into(),
                line: 8,
                domain: "D11".into(),
                story: "STORY-P1-10-02".into(),
                name: "spoor_stamp_park_rung_per_op_of_8".into(),
            }],
            &[],
            &selections,
            &[],
        )
        .expect_err("an unselected domain must be refused");
        assert!(error.contains("D11"), "{error}");
        assert!(error.contains("STORY-P1-10-02"), "{error}");
        assert!(
            error.contains("extend"),
            "the message must point at the contract, not at re-labelling: {error}"
        );
    }

    /// The other direction, and the one that decides whether this gate is
    /// worth having: extending the contract makes the same label legal.
    #[test]
    fn the_same_label_passes_once_the_contract_selects_the_domain() {
        let mut selections = BTreeMap::new();
        selections.insert(
            "STORY-P1-10-02".to_string(),
            BTreeSet::from(["D07".to_string(), "D11".to_string()]),
        );
        check_against(
            &[Declaration {
                file: "fixture_measure_arm64.rs".into(),
                line: 8,
                domain: "D11".into(),
                story: "STORY-P1-10-02".into(),
                name: "spoor_stamp_park_rung_per_op_of_8".into(),
            }],
            &[],
            &selections,
            &[],
        )
        .expect("D11 is selected now");
    }

    #[test]
    fn ref_is_exempt_by_name_and_needs_no_selection() {
        let mut selections = BTreeMap::new();
        selections.insert("STORY-P1-01-04".to_string(), BTreeSet::from(["D02".to_string()]));
        check_against(
            &[Declaration {
                file: "fixture_measure.rs".into(),
                line: 1,
                domain: REFERENCE_DOMAIN.into(),
                story: "STORY-P1-01-04".into(),
                name: "fixed_integer_loop".into(),
            }],
            &[],
            &selections,
            &[],
        )
        .expect("REF has no target column to be read against");
    }

    /// A domain-shaped typo must not fall through the `Dnn` check into the
    /// exemption `REF` occupies.
    #[test]
    fn a_domain_that_is_neither_ref_nor_dnn_is_refused() {
        let mut selections = BTreeMap::new();
        selections.insert("STORY-P1-01-01".to_string(), BTreeSet::from(["D07".to_string()]));
        let error = check_against(
            &[Declaration {
                file: "f.rs".into(),
                line: 1,
                domain: "D99".into(),
                story: "STORY-P1-01-01".into(),
                name: "x".into(),
            }],
            &[],
            &selections,
            &[],
        )
        .expect_err("D99 is off the axis");
        assert!(error.contains("D99"), "{error}");
    }

    #[test]
    fn a_story_with_no_contract_row_is_refused() {
        let error = check_against(
            &[Declaration {
                file: "f.rs".into(),
                line: 1,
                domain: "D07".into(),
                story: "STORY-P9-99-99".into(),
                name: "x".into(),
            }],
            &[],
            &BTreeMap::new(),
            &[],
        )
        .expect_err("an unfiled Story owns nothing");
        assert!(error.contains("STORY-P9-99-99"), "{error}");
    }

    #[test]
    fn two_metrics_with_one_name_in_one_fixture_are_refused() {
        let mut selections = BTreeMap::new();
        selections.insert("STORY-P0-03-01".to_string(), BTreeSet::from(["D07".to_string()]));
        let row = |line| Declaration {
            file: "fixture_pool_bench.rs".into(),
            line,
            domain: "D07".into(),
            story: "STORY-P0-03-01".into(),
            name: "pool_u64x64_tail".into(),
        };
        let error = check_against(&[row(1), row(2)], &[], &selections, &[])
            .expect_err("one report cannot carry two METRIC lines with one name");
        assert!(error.contains("declared twice"), "{error}");
    }

    #[test]
    fn an_unlabelled_construction_fails_the_gate() {
        let error = check_against(
            &[],
            &[UnlabelledSite { file: "new_fixture.rs".into(), line: 42 }],
            &BTreeMap::new(),
            &[],
        )
        .expect_err("a new fixture must not emit an unlabelled metric");
        assert!(error.contains("new_fixture.rs:42"), "{error}");
    }

    /// Rule 6, and the reconstruction of `LE-91`'s own instance: the register
    /// filed `PERF-D11-G01` from a metric that said `D07`, and rules 1–3 had
    /// nothing to object to because `STORY-P1-10-02` selected `D07`.
    #[test]
    fn a_gate_filed_from_a_metric_labelled_for_another_domain_is_refused() {
        let mut selections = BTreeMap::new();
        selections.insert("STORY-P1-10-02".to_string(), BTreeSet::from(["D07".to_string()]));
        let declaration = Declaration {
            file: "fixture_measure_arm64.rs".into(),
            line: 8,
            domain: "D07".into(),
            story: "STORY-P1-10-02".into(),
            name: "spoor_stamp_park_rung_per_op_of_8".into(),
        };
        let evidence = [EvidenceRow {
            guardrail: "PERF-D11-G01".into(),
            domain: "D11".into(),
            note: "Measured on silicon: spoor_stamp_park_rung_per_op_of_8 p50 is 1.9x over".into(),
        }];
        let error = check_against(&[declaration], &[], &selections, &evidence)
            .expect_err("a D11 gate read from a D07 metric must be refused");
        assert!(error.contains("PERF-D11-G01"), "{error}");
        assert!(error.contains("spoor_stamp_park_rung_per_op_of_8"), "{error}");
    }

    /// The row rule 6 must **not** flag, and the reason it is "some" rather
    /// than "every": `PERF-D04-G23`'s real note quotes the `D11` stamp cost to
    /// explain where its 110-cycle delta comes from, while measuring its own
    /// `D04` metric. A rule demanding that every named metric match would have
    /// flagged that correct row on the committed tree — it did, before this
    /// test existed — and a gate that cries wolf on correct work gets
    /// switched off.
    #[test]
    fn a_note_may_cite_another_domains_metric_as_long_as_it_names_its_own() {
        let mut selections = BTreeMap::new();
        selections.insert(
            "STORY-P1-07-06".to_string(),
            BTreeSet::from(["D04".to_string(), "D11".to_string()]),
        );
        let own = Declaration {
            file: "f.rs".into(),
            line: 1,
            domain: "D04".into(),
            story: "STORY-P1-07-06".into(),
            name: "context_switch_yield_roundtrip_2switches".into(),
        };
        let other = Declaration {
            file: "f.rs".into(),
            line: 2,
            domain: "D11".into(),
            story: "STORY-P1-07-06".into(),
            name: "spoor_stamp_park_rung_per_op_of_8".into(),
        };
        let evidence = [EvidenceRow {
            guardrail: "PERF-D04-G23".into(),
            domain: "D04".into(),
            note: "context_switch_yield_roundtrip_2switches p99 82 -> 192; the 110 is the same \
                   110 spoor_stamp_park_rung_per_op_of_8 costs everywhere"
                .into(),
        }];
        check_against(&[own, other], &[], &selections, &evidence)
            .expect("a cross-reference is not a mislabel when the row names its own metric");
    }

    /// The reference denominator divides every ratio, so it is quotable in any
    /// domain's evidence row and must not fire rule 6.
    #[test]
    fn a_ref_metric_named_in_any_gates_evidence_is_not_a_rule_six_finding() {
        let mut selections = BTreeMap::new();
        selections.insert("STORY-P1-01-04".to_string(), BTreeSet::from(["D02".to_string()]));
        let declaration = Declaration {
            file: "fixture_measure.rs".into(),
            line: 1,
            domain: REFERENCE_DOMAIN.into(),
            story: "STORY-P1-01-04".into(),
            name: "fixed_integer_loop".into(),
        };
        let evidence = [EvidenceRow {
            guardrail: "PERF-D05-G23".into(),
            domain: "D05".into(),
            note: "the ratio divides by fixed_integer_loop".into(),
        }];
        check_against(&[declaration], &[], &selections, &evidence)
            .expect("REF is every domain's denominator");
    }

    /// Two metrics where one name is a prefix of the other — the batched arm
    /// and its `_spoored` twin. A substring match would report the shorter one
    /// as named by a row that only mentions the longer.
    #[test]
    fn a_metric_name_that_is_a_prefix_of_another_does_not_match_inside_it() {
        let short = "pool_u64x64_alloc_free_round_trip_per_op_of_8";
        let long = "pool_u64x64_alloc_free_round_trip_per_op_of_8_spoored";
        let note = format!("both arms in one run, {long} against its twin");
        assert!(names_metric(&note, long));
        assert!(!names_metric(&note, short), "the shorter name is not named by this note");
    }

    /// The gate's real subject: the committed tree.
    ///
    /// The counts are asserted rather than merely reported, because the
    /// failure mode this gate is most likely to acquire is silence — a parser
    /// that finds nothing passes everything. A fixture that stops declaring,
    /// or a table that stops parsing, fails here.
    #[test]
    fn the_committed_tree_declares_every_metric_and_every_label_holds() {
        let summary = check_metric_labels(&repo_root()).expect("every metric label must hold");
        assert_eq!(
            summary.declaration_count, 40,
            "40 metrics were declared across six fixtures when this gate was written; a change \
             in this number is a change in the tree's measured surface and should be deliberate"
        );
        assert_eq!(
            summary.declaring_file_count, 6,
            "six fixtures emit measurement envelopes: the two `fixture_measure`s, `pool_bench`, \
             `actuation`, and `exec`'s dispatch and pe fixtures"
        );
        assert!(summary.file_count > 50, "os/src holds more than fifty Rust files");
        assert_eq!(
            summary.domain_count, 8,
            "seven domains — D02, D03, D04, D05, D07, D09, D11 — plus the REF denominator"
        );
        assert_eq!(
            summary.story_count, 9,
            "nine Stories own metrics — P0-01-06, P0-03-01, P1-01-01, P1-01-04, P1-02-01, \
             P1-03-03, P1-06-01, P1-07-06, P1-10-02 — across six fixtures, because a fixture is \
             not the unit that owns a metric"
        );
    }
}
