//! `check-external-isolation` — the gate ADR 0008 folds into the spine.
//!
//! The internals review named the failure mode: a reference tree in a language
//! we build in is one `path =` dependency away from silently becoming a fork.
//! `external/` holds three such trees, one of them Rust, and
//! [`agent.md`] rule 7 — nothing compiled lives outside `os/src/` — was until
//! now a rule a reviewer had to remember. This check makes it a gate: every
//! `Cargo.toml` under `os/` is parsed, and the spine fails on any `path =`
//! dependency that resolves outside `os/`, or any workspace member that does.
//!
//! The parse is deliberately line-lexical, like every other xtask validator:
//! no TOML crate, comments stripped, `path` keys matched where a dependency
//! table can put them. A false positive here is a loud one-line fix; a parser
//! dependency is a supply-chain surface in the tool that gates the spine.

use std::fs;
use std::path::Path;

/// What the isolation check examined, so a caller can print evidence of
/// coverage rather than a bare "ok".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalIsolationSummary {
    /// `Cargo.toml` files parsed under `os/`.
    pub manifest_count: usize,
    /// `path =` dependencies found and proven to stay inside `os/`.
    pub path_dependency_count: usize,
}

/// Validates that no manifest under `os/` reaches outside `os/`.
pub fn check_external_isolation(repo_root: &Path) -> Result<ExternalIsolationSummary, String> {
    let os_root = repo_root.join("os");
    let mut manifests = Vec::new();
    collect_manifests(&os_root, &mut manifests)
        .map_err(|error| format!("walking {}: {error}", os_root.display()))?;
    if manifests.is_empty() {
        return Err(format!("no Cargo.toml found under {}", os_root.display()));
    }

    let mut path_dependency_count = 0;
    for manifest_path in &manifests {
        let contents = fs::read_to_string(manifest_path)
            .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
        let base_dir = manifest_path
            .parent()
            .and_then(|dir| dir.strip_prefix(&os_root).ok())
            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            .ok_or_else(|| format!("{} is not under os/", manifest_path.display()))?;

        for dependency in path_dependencies(&contents) {
            if resolves_outside_os(&base_dir, &dependency) {
                return Err(format!(
                    "{}: `path = \"{dependency}\"` resolves outside os/. Nothing under \
                     external/ (or anywhere else outside the workspace) may be built upon — \
                     ADR 0008, agent.md rule 7",
                    manifest_path.display()
                ));
            }
            path_dependency_count += 1;
        }
        for member in workspace_members(&contents) {
            if resolves_outside_os(&base_dir, &member) {
                return Err(format!(
                    "{}: workspace member `{member}` resolves outside os/ — ADR 0008, \
                     agent.md rule 7",
                    manifest_path.display()
                ));
            }
        }
    }
    Ok(ExternalIsolationSummary { manifest_count: manifests.len(), path_dependency_count })
}

/// Every `Cargo.toml` under `root`, skipping build output and VCS metadata.
fn collect_manifests(root: &Path, found: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name == "target" || name.starts_with('.') {
                continue;
            }
            collect_manifests(&path, found)?;
        } else if name == "Cargo.toml" {
            found.push(path);
        }
    }
    Ok(())
}

/// Every `path` key value in dependency position, comments stripped.
///
/// Matches both placements Cargo accepts: the inline table
/// (`dep = { path = "…" }`) and the bare key of a `[dependencies.dep]`
/// table. A key is a `path` key only when the character before it is a
/// word boundary, so `datapath = "…"` never matches.
fn path_dependencies(manifest: &str) -> Vec<String> {
    let mut found = Vec::new();
    for raw_line in manifest.lines() {
        let line = raw_line.split('#').next().unwrap_or("");
        let mut search_from = 0;
        while let Some(offset) = line[search_from..].find("path") {
            let key_start = search_from + offset;
            search_from = key_start + "path".len();
            let boundary_before = line[..key_start]
                .chars()
                .next_back()
                .is_none_or(|c| c.is_whitespace() || c == '{' || c == ',');
            if !boundary_before {
                continue;
            }
            let rest = line[search_from..].trim_start();
            let Some(rest) = rest.strip_prefix('=') else { continue };
            let rest = rest.trim_start();
            let Some(rest) = rest.strip_prefix('"') else { continue };
            let Some(end) = rest.find('"') else { continue };
            found.push(rest[..end].to_string());
        }
    }
    found
}

/// Every quoted entry of the `[workspace] members` array, multi-line aware.
fn workspace_members(manifest: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut in_members = false;
    for raw_line in manifest.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        let mut rest = if in_members {
            line
        } else if let Some(after) = line.strip_prefix("members").map(str::trim_start) {
            let Some(after) = after.strip_prefix('=') else { continue };
            let Some(after) = after.trim_start().strip_prefix('[') else { continue };
            in_members = true;
            after
        } else {
            continue;
        };
        loop {
            if let Some(close) = rest.find(']') {
                let (inside, _) = rest.split_at(close);
                rest = inside;
                in_members = false;
            }
            let Some(open) = rest.find('"') else { break };
            let after_open = &rest[open + 1..];
            let Some(close) = after_open.find('"') else { break };
            found.push(after_open[..close].to_string());
            rest = &after_open[close + 1..];
        }
    }
    found
}

/// Whether `relative` (a `path =` value or member entry), joined onto
/// `base_dir` (the manifest's directory stated relative to `os/`), escapes
/// `os/` by lexical normalisation. The target need not exist: this is asked of
/// paths whose whole defect is that they point at something outside the tree
/// the workspace may build.
fn resolves_outside_os(base_dir: &str, relative: &str) -> bool {
    let mut depth: isize =
        base_dir.split('/').filter(|c| !c.is_empty() && *c != ".").count() as isize;
    for component in relative.replace('\\', "/").split('/') {
        match component {
            "" | "." => {}
            ".." => {
                depth -= 1;
                if depth < 0 {
                    return true;
                }
            }
            _ => depth += 1,
        }
    }
    false
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

    // The failure mode ADR 0008 exists to close: an os/ crate quietly building
    // on the vendored fork.
    #[test]
    fn a_path_dependency_escaping_os_is_detected() {
        let manifest =
            "[dependencies]\ntauri = { path = \"../../../external/tauri/crates/tauri\" }\n";
        let deps = path_dependencies(manifest);
        assert_eq!(deps, vec!["../../../external/tauri/crates/tauri"]);
        assert!(resolves_outside_os("src/kernel", &deps[0]));
    }

    #[test]
    fn a_path_dependency_inside_os_is_allowed() {
        let manifest = "[dependencies]\nshared = { path = \"../shared\", version = \"0.1\" }\n";
        let deps = path_dependencies(manifest);
        assert_eq!(deps, vec!["../shared"]);
        assert!(!resolves_outside_os("src/kernel", &deps[0]));
    }

    #[test]
    fn a_table_style_path_dependency_is_found() {
        let manifest = "[dependencies.helper]\npath = \"../../work/helper\"\n";
        assert_eq!(path_dependencies(manifest), vec!["../../work/helper"]);
        assert!(resolves_outside_os("src", "../../work/helper"));
    }

    #[test]
    fn a_commented_out_path_is_ignored() {
        let manifest = "[dependencies]\n# tauri = { path = \"../../external/tauri\" }\n";
        assert!(path_dependencies(manifest).is_empty());
    }

    #[test]
    fn a_workspace_member_outside_os_is_detected() {
        let manifest = "[workspace]\nmembers = [\n    \"src/kernel\",\n    \"../external/tauri/crates/tauri\",\n]\n";
        let members = workspace_members(manifest);
        assert_eq!(members.len(), 2);
        assert!(!resolves_outside_os("", &members[0]));
        assert!(resolves_outside_os("", &members[1]));
    }

    // `path` as a substring of another key must not match: this is the false
    // positive a naive `contains("path")` would produce.
    #[test]
    fn a_key_merely_containing_path_is_not_a_path_dependency() {
        let manifest =
            "[package.metadata]\nhookspath = \"../outside\"\ndatapath = \"../outside\"\n";
        assert!(path_dependencies(manifest).is_empty());
    }

    #[test]
    fn the_committed_tree_passes() {
        let summary =
            check_external_isolation(&repo_root()).expect("the committed tree stays inside os/");
        // A floor, never a total: the workspace grows with every crate split.
        assert!(summary.manifest_count >= 2, "os/ holds at least the workspace root and xtask");
    }
}
