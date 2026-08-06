//! The loose-ends register — the project's machine-readable defect list — and
//! the citation resolver that keeps each row joined to the session document
//! that raised or closed it (`LE-51`).
//!
//! The register exists because `LE-*` ids were once carried only in handover
//! prose, which the session convention forbids editing once a newer dated
//! folder exists; the canonical list therefore fragmented across handovers and
//! could not be queried.

use super::*;

/// Validates the loose-ends register: the project's machine-readable defect list.
///
/// The register exists because `LE-*` ids were previously carried only in session
/// handover prose, which the session convention forbids editing once a newer dated
/// folder exists — so the canonical list fragmented across handovers and could not
/// be queried. Ids must be contiguous from `LE-01`, because a gap is the signature
/// of exactly that fragmentation.
pub(super) fn validate_loose_ends(contents: &str) -> Result<LooseEndIndex, String> {
    const OWNERSHIP: [&str; 3] = ["owned", "unowned", "deferred-with-trigger"];
    const STATES: [&str; 2] = ["open", "closed"];

    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "loose-ends register is empty".to_string())?
        .trim_end_matches('\r');
    if header != LOOSE_END_HEADER {
        return Err(format!("unexpected loose-ends header; expected exactly `{LOOSE_END_HEADER}`"));
    }

    let mut ids = BTreeSet::new();
    let mut open_count = 0;
    let mut citations: Vec<(String, &'static str, String)> = Vec::new();
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let fields =
            non_empty_tsv_fields(raw_line, line_number, LOOSE_END_FIELD_COUNT, "loose-ends")?;

        let id = fields[0];
        // Two digits, or three since the register passed `LE-99` on
        // 2026-08-06. Zero-padding to a fixed width was considered and
        // rejected: it would renumber every existing row and every citation of
        // one, across the whole tree, to buy nothing but alignment.
        let Some(number) = id.strip_prefix("LE-").filter(|suffix| {
            is_loose_end_number_width(suffix.len()) && suffix.bytes().all(|b| b.is_ascii_digit())
        }) else {
            return Err(format!(
                "loose-ends line {line_number}: `{id}` is not a valid id (expected `LE-NN` or \
                 `LE-NNN`)"
            ));
        };
        if !ids.insert(id.to_string()) {
            return Err(format!("loose-ends line {line_number}: duplicate id `{id}`"));
        }
        if number.parse::<usize>().unwrap_or(0) != ids.len() {
            return Err(format!(
                "loose-ends line {line_number}: `{id}` is out of order or leaves a gap; ids must \
                 run contiguously from `LE-01`"
            ));
        }

        let ownership = fields[4];
        if !OWNERSHIP.contains(&ownership) {
            return Err(format!(
                "loose-ends line {line_number}: unknown ownership `{ownership}` (expected one of {})",
                OWNERSHIP.join(", ")
            ));
        }

        let state = fields[5];
        if !STATES.contains(&state) {
            return Err(format!(
                "loose-ends line {line_number}: unknown state `{state}` (expected one of {})",
                STATES.join(", ")
            ));
        }

        // A closed loose end must say where it closed, and an open one must not
        // claim to have closed anywhere. Without this the register can report a
        // defect as resolved with no evidence behind the claim.
        let closed_in = fields[7];
        match (state, closed_in == LOOSE_END_UNSET) {
            ("closed", true) => {
                return Err(format!(
                    "loose-ends line {line_number}: `{id}` is closed but records no `closed_in`"
                ));
            }
            ("open", false) => {
                return Err(format!(
                    "loose-ends line {line_number}: `{id}` is open but records `closed_in` \
                     `{closed_in}`"
                ));
            }
            _ => {}
        }
        if state == "open" {
            open_count += 1;
        }

        // `LE-51`: collected here, resolved against `session/` by the caller —
        // this function is pure so its own tests stay filesystem-free.
        citations.push((id.to_string(), "raised_in", fields[6].to_string()));
        if closed_in != LOOSE_END_UNSET {
            citations.push((id.to_string(), "closed_in", closed_in.to_string()));
        }
    }

    if ids.is_empty() {
        return Err("loose-ends register has no entries".to_string());
    }
    Ok(LooseEndIndex { ids, open_count, citations })
}

/// Fails if any `LE-*` token in the live documents has no row in the register.
///
/// Only `goals/` and `docs/` are scanned. `session/` is deliberately excluded: those
/// dated folders are an immutable historical record that the session convention says
/// is never edited, so a token frozen in an old handover must not gate the register.
pub(super) fn validate_loose_end_references(
    repo_root: &Path,
    ids: &BTreeSet<String>,
) -> Result<usize, String> {
    let mut markdown = Vec::new();
    for directory in ["goals", "docs"] {
        collect_markdown(&repo_root.join(directory), &mut markdown)?;
    }

    let mut reference_count = 0;
    for path in markdown {
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        for (zero_based_index, line) in contents.lines().enumerate() {
            for token in loose_end_tokens(line) {
                reference_count += 1;
                if !ids.contains(&token) {
                    return Err(format!(
                        "{}:{}: `{token}` has no row in goals/assurance/loose-ends.tsv",
                        path.display(),
                        zero_based_index + 1
                    ));
                }
            }
        }
    }
    Ok(reference_count)
}

/// A parsed loose-end citation: the dated session folder and the slot prefix
/// within it (`LE-51`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Citation {
    pub(super) folder: String,
    pub(super) slot: String,
}

/// Parses a `raised_in`/`closed_in` field into the document it cites.
///
/// The citation is the first whitespace-delimited token; a few historical rows
/// carry trailing prose (`hand-2026-07-30/05A session, owner feedback`) and
/// that prose is deliberately preserved rather than rewritten — the citation
/// still has to resolve, which is the whole point of `LE-51`.
///
/// The slot is the `NN`/`NN<Letter>` prefix, or a whole document stem when the
/// short form would be ambiguous. Keeping the letter *in* the slot is what lets
/// the check tell `41A` from `41B`; allowing the long form is what lets a row
/// cite one of two documents that genuinely share a slot, which
/// `hand-2026-07-30/03A` does. Renaming a committed handover to suit the gate
/// would edit the historical record, so the citation gets more specific instead.
pub(super) fn parse_citation(field: &str) -> Result<Citation, String> {
    let token = field.split_whitespace().next().unwrap_or("");
    let Some((folder, slot)) = token.split_once('/') else {
        return Err(format!("`{token}` is not a `hand-<date>/<slot>` citation"));
    };
    if !folder.starts_with("hand-") {
        return Err(format!("`{token}` does not name a `hand-<date>` session folder"));
    }
    let slot = slot.trim_end_matches(',');
    if slot.len() < 2 || !slot.as_bytes()[..2].iter().all(u8::is_ascii_digit) {
        return Err(format!("`{token}` has no `NN` slot number"));
    }
    if !slot.as_bytes()[2..].iter().all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-') {
        return Err(format!("`{token}`'s slot must be `NN`, `NN<Letter>` or a document stem"));
    }
    Ok(Citation { folder: folder.to_string(), slot: slot.to_string() })
}

/// Picks the one document in a session folder that a slot names.
///
/// Zero matches is a dangling citation — the register pointing at a handover
/// nobody wrote. Two or more is the `LE-51` ambiguity itself. The prefix
/// includes the separating `-` so slot `07` cannot swallow `07A-...`.
pub(super) fn resolve_slot(file_names: &[String], slot: &str) -> Result<String, String> {
    let prefix = format!("{slot}-");
    let matches: Vec<&String> = file_names
        .iter()
        .filter(|name| {
            // A short slot matches by prefix; a full stem matches exactly.
            let stem = name.rsplit_once('.').map_or(name.as_str(), |(stem, _)| stem);
            name.starts_with(&prefix) || stem == slot
        })
        .collect();
    match matches.as_slice() {
        [] => Err(format!("no document in that folder begins with `{prefix}`")),
        [only] => Ok((*only).clone()),
        many => Err(format!(
            "{} documents begin with `{prefix}` ({}); a citation must name exactly one",
            many.len(),
            many.iter().map(|name| name.as_str()).collect::<Vec<_>>().join(", ")
        )),
    }
}

/// Resolves every `raised_in` and `closed_in` against `session/` (`LE-51`).
///
/// Before this gate the register checked only that a closed row carried *some*
/// `closed_in` string. It never asked whether the string named a document that
/// exists, so a defect could be recorded as resolved by a handover nobody
/// wrote, and two rows could cite one slot while meaning two different files.
/// Returns the number of citations resolved.
pub(super) fn validate_loose_end_citations(
    repo_root: &Path,
    citations: &[(String, &'static str, String)],
) -> Result<usize, String> {
    let mut listings: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, field, value) in citations {
        let citation = parse_citation(value)
            .map_err(|reason| format!("loose-ends `{id}` {field}: {reason}"))?;

        let folder = &citation.folder;
        if !listings.contains_key(folder) {
            let directory = repo_root.join("session").join(folder);
            let mut names = Vec::new();
            match fs::read_dir(&directory) {
                Ok(entries) => {
                    for entry in entries {
                        let entry = entry.map_err(|error| {
                            format!("failed to read {}: {error}", directory.display())
                        })?;
                        names.push(entry.file_name().to_string_lossy().into_owned());
                    }
                }
                Err(_) => {
                    return Err(format!(
                        "loose-ends `{id}` {field}: `{folder}` is not a folder under session/"
                    ));
                }
            }
            names.sort();
            listings.insert(folder.clone(), names);
        }

        let names = &listings[folder];
        resolve_slot(names, &citation.slot)
            .map_err(|reason| format!("loose-ends `{id}` {field} `{value}`: {reason}"))?;
    }
    Ok(citations.len())
}

/// Extracts every `LE-NN` or `LE-NNN` token from one line.
///
/// **The whole digit run, not the first two.** Until 2026-08-06 this took
/// `digits[..2]`, which was correct while the register held fewer than a
/// hundred rows and became a silent misreading the moment it did not:
/// `LE-100` in prose resolved to a citation of `LE-10`, a real row about
/// something else. A decoder that confidently names the wrong record is
/// `LE-80`'s family, and it is worse than one that refuses — so a run of four
/// or more digits is refused here rather than truncated to three.
pub(super) fn loose_end_tokens(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut tokens = Vec::new();
    for (index, _) in line.match_indices("LE-") {
        // `SAMPLE-01` and similar must not match: require a non-alphanumeric before
        // the `L`, so only a standalone token counts.
        if index > 0 {
            let previous = bytes[index - 1];
            if previous.is_ascii_alphanumeric() || previous == b'-' || previous == b'_' {
                continue;
            }
        }
        let digits = &line[index + 3..];
        let run = digits.bytes().take_while(u8::is_ascii_digit).count();
        if is_loose_end_number_width(run) {
            tokens.push(format!("LE-{}", &digits[..run]));
        }
    }
    tokens
}

/// The widths a loose-end number may have: two, or three since `LE-100`.
///
/// One place, so the extractor and the id validator cannot disagree about
/// what an id is — which is exactly how a register ends up accepting a row
/// that nothing can then cite.
pub(super) fn is_loose_end_number_width(digits: usize) -> bool {
    digits == 2 || digits == 3
}
