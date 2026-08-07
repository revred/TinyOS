//! `check-tool-tests` — the bench-instrument suites actually gate something
//! (`LE-114`).
//!
//! `work/tools/` holds every C# bench instrument this project owns — the
//! netboot server, the power relay, `ti64dink` — plus their test projects, and
//! until this gate existed **nothing ran them**: `ci.yml` contained no
//! `dotnet`, the pre-commit hook ran none of them, and `check-crate-sizes`
//! cannot see the directory at all. That is `LE-100` for the other language,
//! and it is worse in one specific way: the two most expensive instrument
//! defects this project has recorded (`LE-80`: a live rung decoded as an
//! absence; `LE-87`: a stale UDP-69 server silently winning the bind) are both
//! in these tools. The tools whose failures cost the most bench time were the
//! tools whose tests gated nothing.
//!
//! # Why discovery is a function and not a workflow glob
//!
//! The same reason [`crate::guest_images`] derives its plan from the fixture
//! register: the defect class is **coverage**, and coverage is a property of
//! the list. A hard-coded list of test projects in a YAML file is a second
//! hand-kept mirror of the directory — `LE-80`'s shape — so the list is
//! derived from the directory itself, refuses to be empty (a suite that
//! matches nothing gates nothing, wearing a green tick), and refuses a
//! `*.tests` directory whose project file is missing rather than silently
//! skipping it.
//!
//! # What this gate cannot see
//!
//! It proves the suites are *executed and passing*, not that they are
//! sufficient. It also inherits `LE-64`'s family: these tools do NIC
//! enumeration and raw capture, so a test passing on a Windows bench may fail
//! on the Linux runner — which is a finding, not a flake, and is the reason
//! `LE-114` asks for this gate to land alone.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Discovers every test project under the given tools directory.
///
/// A test project is a directory named `<name>.tests` containing
/// `<name>.tests.csproj`. The result is sorted by name so output and
/// execution order are deterministic across hosts.
///
/// # Errors
///
/// - a `*.tests` directory with no matching `.csproj` — a half-created or
///   half-deleted project must fail the gate, not shrink its coverage;
/// - an empty result — the gate exists to run suites, and a discovery that
///   finds none is a broken premise, never a pass.
pub fn discover(tools_root: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(tools_root)
        .map_err(|error| format!("cannot read {}: {error}", tools_root.display()))?;

    let mut projects = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read directory entry: {error}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.ends_with(".tests") {
            continue;
        }
        let csproj = path.join(format!("{name}.csproj"));
        if !csproj.is_file() {
            return Err(format!(
                "{} is named like a test project but holds no {name}.csproj — a \
                 half-created project must fail this gate, not shrink its coverage",
                path.display()
            ));
        }
        projects.push(csproj);
    }

    if projects.is_empty() {
        return Err(format!(
            "no *.tests project found under {} — a discovery that matches nothing \
             gates nothing, and that is `LE-114` wearing a green tick",
            tools_root.display()
        ));
    }
    projects.sort();
    Ok(projects)
}

/// Runs `dotnet test` for every discovered project, continuing past failures
/// so one suite's failure cannot hide the next suite's — the same reasoning
/// as `check-lints`.
///
/// # Errors
///
/// Discovery errors propagate; a missing `dotnet` SDK is reported as its own
/// failure; and any failing suite fails the gate, with every failing project
/// named rather than only the first.
pub fn check_tool_tests(repo_root: &Path) -> Result<usize, String> {
    let tools_root = repo_root.join("work").join("tools");
    let projects = discover(&tools_root)?;

    let mut failures: Vec<String> = Vec::new();
    for csproj in &projects {
        let label =
            csproj.file_stem().and_then(|stem| stem.to_str()).unwrap_or("(unnamed)").to_string();
        println!("tool-tests: dotnet test {label}");
        // `BaseOutputPath` is isolated per gate run, and it is load-bearing on
        // a live bench: these are the programs that serve the board, and
        // `tos64-netboot` may deliberately be left running for days. A default
        // build would try to overwrite the running server's own exe, fail on
        // the file lock, and report the suite as failed — training operators
        // to stop the server (or skip the gate) to make a test pass. Building
        // into a separate directory keeps the gate honest about the tests and
        // silent about the bench. Relative, so it lands inside each project's
        // own `bin/`.
        let status = Command::new("dotnet")
            .arg("test")
            .arg(csproj)
            .arg("--nologo")
            .arg("-p:BaseOutputPath=bin/xtask-tool-tests/")
            .status()
            .map_err(|error| {
                format!(
                    "failed to invoke `dotnet`: {error} — this gate needs the .NET SDK the \
                     tools target; on the runner it is installed by the workflow (`LE-114`)"
                )
            })?;
        if !status.success() {
            failures.push(label);
        }
    }

    if failures.is_empty() {
        Ok(projects.len())
    } else {
        Err(format!(
            "{} of {} bench-instrument suite(s) failed: {} — a bench instrument with a \
             failing test is an instrument that will lie at the cost of a power cycle",
            failures.len(),
            projects.len(),
            failures.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory that cleans up after itself, unique per test.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("tinyos-tool-tests-{name}"));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("scratch dir");
            Scratch(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn repo_tools_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("xtask manifest lives at os/src/xtask")
            .join("work")
            .join("tools")
    }

    /// The committed tree's own suites are discovered. A **floor**, never a
    /// total: a count of how much work exists grows with every new tool, and a
    /// test pinning it exactly is the defect `08A` §1 recorded.
    #[test]
    fn every_known_bench_test_project_is_discovered() {
        let projects = discover(&repo_tools_root()).expect("the committed tree has suites");
        for known in ["netboot.tests", "power.tests"] {
            assert!(
                projects
                    .iter()
                    .any(|path| { path.file_stem().and_then(|stem| stem.to_str()) == Some(known) }),
                "{known} is committed and must be discovered — found {projects:?}"
            );
        }
    }

    /// The refusal that keeps coverage from shrinking silently: a directory
    /// named like a test project with no project file inside is a broken
    /// state, not a skippable one.
    #[test]
    fn a_tests_directory_without_its_csproj_is_refused() {
        let scratch = Scratch::new("missing-csproj");
        std::fs::create_dir(scratch.0.join("stray.tests")).expect("stray dir");
        let error = discover(&scratch.0).expect_err("a projectless *.tests dir must refuse");
        assert!(error.contains("stray.tests"), "the error must name the directory: {error}");
    }

    /// The refusal that keeps the gate from passing vacuously: no discovered
    /// suite is a broken premise, never a green tick.
    #[test]
    fn an_empty_tools_directory_is_refused() {
        let scratch = Scratch::new("empty");
        let error = discover(&scratch.0).expect_err("an empty discovery must refuse");
        assert!(error.contains("matches nothing"), "{error}");
    }

    /// Deterministic order, so two hosts and two runs report the same thing
    /// in the same sequence.
    #[test]
    fn discovery_is_sorted_by_project_name() {
        let scratch = Scratch::new("sorted");
        for name in ["b.tests", "a.tests"] {
            let dir = scratch.0.join(name);
            std::fs::create_dir(&dir).expect("dir");
            std::fs::write(dir.join(format!("{name}.csproj")), "<Project/>").expect("csproj");
        }
        let projects = discover(&scratch.0).expect("two well-formed projects");
        let names: Vec<_> = projects
            .iter()
            .map(|path| path.file_stem().and_then(|stem| stem.to_str()).unwrap().to_string())
            .collect();
        assert_eq!(names, ["a.tests", "b.tests"]);
    }
}
