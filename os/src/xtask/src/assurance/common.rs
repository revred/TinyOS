//! Parsing and formatting helpers shared by every validator in this module.
//!
//! Nothing here knows what a contract, a guardrail or a loose end *is*. These
//! are the tab-separated-value, filesystem and id-formatting primitives the
//! subject-matter modules are written in terms of, kept in one place so a
//! change to the TSV shape is one edit rather than nine.

use super::*;

/// Collects `.rs` files under a directory, recursively.
pub(super) fn collect_rust_sources(
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read a directory entry: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(&path, output)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            output.push(path);
        }
    }
    Ok(())
}
pub(super) fn collect_markdown(directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry
            .map_err(|error| format!("failed to enumerate {}: {error}", directory.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_markdown(&path, output)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("md") {
            output.push(path);
        }
    }
    Ok(())
}
pub(super) fn non_empty_tsv_fields<'a>(
    raw_line: &'a str,
    line_number: usize,
    expected_count: usize,
    kind: &str,
) -> Result<Vec<&'a str>, String> {
    let line = raw_line.trim_end_matches('\r');
    if line.is_empty() {
        return Err(format!("{kind} line {line_number}: blank rows are not allowed"));
    }
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() != expected_count {
        return Err(format!(
            "{kind} line {line_number}: expected {expected_count} fields, found {}",
            fields.len()
        ));
    }
    if fields.iter().any(|field| field.trim().is_empty()) {
        return Err(format!("{kind} line {line_number}: every field must be non-empty"));
    }
    Ok(fields)
}

pub(super) fn is_phase_id(value: &str) -> bool {
    value.strip_prefix('P').is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}

pub(super) fn is_two_digits(value: &str) -> bool {
    value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_digit())
}

pub(super) fn markdown_ids(directory: &Path, prefix: &str) -> Result<BTreeSet<String>, String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
    let mut ids = BTreeSet::new();
    for entry in entries {
        let entry = entry
            .map_err(|error| format!("failed to enumerate {}: {error}", directory.display()))?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if stem.starts_with(prefix) {
            ids.insert(stem.to_string());
        }
    }
    Ok(ids)
}

pub(super) fn compare_exact_coverage(
    kind: &str,
    files: &BTreeSet<String>,
    contracts: &BTreeSet<String>,
) -> Result<(), String> {
    let missing: Vec<&String> = files.difference(contracts).collect();
    if !missing.is_empty() {
        return Err(format!("{kind} files missing assurance contracts: {}", join_ids(&missing)));
    }
    let stale: Vec<&String> = contracts.difference(files).collect();
    if !stale.is_empty() {
        return Err(format!(
            "assurance contracts reference nonexistent {kind} files: {}",
            join_ids(&stale)
        ));
    }
    Ok(())
}
pub(super) fn join_ids(ids: &[&String]) -> String {
    ids.iter().map(|id| id.as_str()).collect::<Vec<_>>().join(", ")
}

pub(super) fn join_owned_ids(ids: &BTreeSet<String>) -> String {
    ids.iter().map(String::as_str).collect::<Vec<_>>().join(", ")
}
