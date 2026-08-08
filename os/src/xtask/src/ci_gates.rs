//! `check-ci-gates` — the gates this repository claims are enforced must be
//! enforced *on the runner* (`LE-100`).
//!
//! For the whole life of this project CI ran **no tests at all**.
//! `.github/workflows/ci.yml` had four jobs; the governance one ran `fmt`,
//! `clippy --all-targets`, three `xtask` checks and `cargo doc`, and the other
//! three booted QEMU fixtures. There was no `cargo test` step anywhere in the
//! file.
//!
//! It survived because it looks covered at a glance: `clippy --all-targets`
//! **compiles** the test harnesses, so a *broken* test fails the build. A
//! **failing** test does not. Roughly 1,217 host tests were passing locally
//! when this was found, and among them was every source-level guard this
//! project has ever filed as the closure of a loose end — `LE-99`'s
//! stamp-density guard, `LE-97`'s cadence guard, the `G23` pair equivalence,
//! `check-citations`' own `the_committed_tree_has_no_unresolved_citation`.
//! Each was written as *the mechanism that stops this recurring*. Each was
//! invisible to the runner.
//!
//! That is `LE-72` and `LE-92` a third time, and the general shape is worth
//! stating once: **a gate is only as strong as the weakest place it is
//! actually executed.** `LE-72` was the discovery for AArch64 compilation,
//! `LE-92` for x86_64 compilation, and this is the same discovery for the test
//! runner.
//!
//! # Why this is a subcommand and not a `#[test]`
//!
//! Because the obvious version of this guard is circular and therefore
//! worthless. A `#[test]` asserting *"CI runs the host tests"* is run by the
//! very job it asserts the existence of: delete the job, and the test that
//! would have objected is no longer executed either. The guard has to run
//! somewhere that survives the deletion of what it guards, so it runs as a
//! step in the fast governance job — the same reasoning that put
//! `check-metric-labels` there rather than leaving it a unit test (`LE-91`).
//!
//! Rule 4 closes the recursion one turn further: this gate asserts that *it
//! is itself* wired into the workflow. A guard nobody runs is the defect it
//! was written about.
//!
//! # What is checked
//!
//! 1. Some job runs the workspace host test suite.
//! 2. That job is **not** the fast governance job. The deterministic gates
//!    exist to fail in seconds and must not queue behind a full suite; keeping
//!    them separate was a deliberate decision and this is what stops it being
//!    undone by a one-line edit that looks tidier.
//! 3. The suite's failure is **blocking** — no `continue-on-error`, no `||
//!    true`. A non-blocking test job is the state this loose end describes
//!    wearing a green tick.
//! 4. Every subcommand in [`CI_ENFORCED`] appears in the workflow, this one
//!    included.
//!
//! # What it cannot see
//!
//! It is a text scan over one file, with the same limit `LE-99`'s guard
//! records about itself. It reads indentation to attribute a `run:` line to a
//! job and does not parse YAML, so a workflow that reformatted its jobs into
//! flow mappings would defeat the attribution — loudly, by failing, which is
//! the acceptable direction. It says nothing about whether the runner *passed*
//! the suite, only that the workflow asks it to; and it cannot tell that a
//! test is meaningful, only that the harness is invoked. Coverage of the
//! *tests themselves* is not a thing this file can hold.

use std::fs;
use std::path::Path;

/// The workflow this gate reads, relative to the repository root.
pub const WORKFLOW: &str = ".github/workflows/ci.yml";

/// The command that runs every host test in the workspace.
///
/// Matched as a prefix so `--locked`, `--release` or a `-- --nocapture` tail
/// still count; narrowing the suite to a subset of packages does not, because
/// that is precisely how this hole would reopen quietly.
const HOST_SUITE: &str = "cargo test --workspace";

/// The fast deterministic job. Named rather than inferred, because rule 2 is
/// about *this* job specifically: it is the one whose whole value is failing
/// in seconds.
const GOVERNANCE_JOB: &str = "governance-gates";

/// Every `xtask` subcommand this repository files as a CI-enforced mechanism.
///
/// **A closed list, pinned by a test.** Each entry is a check some loose end
/// was closed on, and the closure means nothing unless the runner executes it.
/// Adding a row here is the deliberate act of promising that; removing one
/// should be equally deliberate, and is now equally visible.
///
/// `check-ci-gates` is in its own list on purpose — see rule 4.
pub const CI_ENFORCED: [&str; 9] = [
    "check-assurance-spine",
    "check-performance-catalogue",
    "check-crate-sizes",
    "check-metric-labels",
    "check-boot-images",
    "check-ci-gates",
    // The feasibility report is the page an outside reader is most likely to
    // be shown, so a stale one misinforms further than a stale register does.
    // Listed here rather than trusted to a habit: `LE-106` records a gate this
    // project already owned, wired into nothing, which a session then
    // re-derived by hand — a mechanism nobody is required to run is a mechanism
    // that does not run.
    "check-feasibility",
    // The bench-instrument suites (`LE-114`). This row is the half `LE-106`
    // shows is load-bearing: a job someone adds can be a job someone later
    // drops, and dropping this one must fail the build rather than quietly
    // returning work/tools/ to the ungated state it sat in for its whole life.
    "check-tool-tests",
    // `goals/state.html` — the coherence page: every session, Epic, Feature,
    // open row, and the join saying what blocks what. It is the page somebody
    // reaches for to decide what to work on, which is exactly why a stale copy
    // is worse than none: it would send a session at a Feature whose blockers
    // were cleared a week ago, or hide one whose blockers arrived yesterday.
    "check-state",
];

/// What the gate found, for the operator line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiGateSummary {
    /// The job id running the host suite.
    pub host_test_job: String,
    /// Jobs declared in the workflow.
    pub job_count: usize,
    /// Entries of [`CI_ENFORCED`] located in the workflow — all of them, or
    /// the check failed.
    pub enforced_count: usize,
}

/// One line of a job, and whether it is something the runner **executes**.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RunLine {
    job: String,
    text: String,
    /// True for a `run:` value and for every line inside a `run: |` block.
    ///
    /// **Load-bearing, and it was learned the hard way.** The first version of
    /// this module scanned every line, so a step whose `name:` merely *quoted*
    /// the command satisfied the gate. Verified against the committed
    /// workflow: narrowing `run: cargo test --workspace` to `run: cargo test
    /// -p kernel` while leaving `- name: cargo test --workspace` above it left
    /// this gate **green** — the mutation that made it stay silent, run
    /// against the exact case it was written for. A display name is prose.
    command: bool,
}

/// Attributes every line in the workflow to its job, marking the executable
/// ones.
///
/// Deliberately crude: a job id is a two-space-indented key under `jobs:`, and
/// every subsequent line belongs to it until the next one. Block scalars are
/// tracked by indentation — a `run: |` opens one, and it closes at the first
/// line indented no deeper than the `run:` key itself.
fn run_lines(workflow: &str) -> Vec<RunLine> {
    let mut lines = Vec::new();
    let mut job = String::new();
    let mut in_jobs = false;
    let mut block: Option<usize> = None;
    for raw in workflow.lines() {
        let line = raw.trim_end();
        if line == "jobs:" {
            in_jobs = true;
            continue;
        }
        if !in_jobs {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();

        // A block scalar's body ends at the first line no deeper than its key.
        if let Some(opened_at) = block {
            if trimmed.is_empty() {
                continue;
            }
            if indent > opened_at {
                lines.push(RunLine { job: job.clone(), text: trimmed.to_string(), command: true });
                continue;
            }
            block = None;
        }

        if indent == 2 && trimmed.ends_with(':') && !trimmed.starts_with('-') {
            job = trimmed.trim_end_matches(':').to_string();
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let text = trimmed.trim_start_matches("- ");
        let body = text.strip_prefix("run:").map(str::trim);
        if matches!(body, Some("|" | ">" | "|-" | ">-")) {
            block = Some(indent);
            continue;
        }
        lines.push(RunLine { job: job.clone(), text: text.to_string(), command: body.is_some() });
    }
    lines
}

/// The body of a job, as its own lines. Used for the blocking check, which is
/// a property of the job rather than of one command.
fn job_lines<'a>(lines: &'a [RunLine], job: &'a str) -> impl Iterator<Item = &'a RunLine> {
    lines.iter().filter(move |line| line.job == job)
}

/// Checks the committed workflow. See the module docs for the four rules.
///
/// # Errors
///
/// Returns the first rule violated, naming the loose end it belongs to — a
/// message a reader can act on without opening this file.
pub fn check_ci_gates(repo_root: &Path) -> Result<CiGateSummary, String> {
    let path = repo_root.join(WORKFLOW);
    let workflow = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    check_workflow(&workflow)
}

/// The rules, over the workflow's text. Split from [`check_ci_gates`] so the
/// tests can state a workflow rather than write one to disk.
fn check_workflow(workflow: &str) -> Result<CiGateSummary, String> {
    let lines = run_lines(workflow);
    let mut jobs: Vec<&str> = lines.iter().map(|line| line.job.as_str()).collect();
    jobs.sort_unstable();
    jobs.dedup();

    // Rule 1. The subject of `LE-100`.
    let host_test_job = lines
        .iter()
        .find(|line| line.command && line.text.contains(HOST_SUITE))
        .map(|line| line.job.clone())
        .ok_or_else(|| {
            format!(
                "{WORKFLOW} runs no host tests: no step invokes `{HOST_SUITE}` (`LE-100`). \
                 `clippy --all-targets` compiles the harnesses, so a broken test fails the \
                 build and a FAILING test does not -- every #[test] in this workspace would \
                 gate a developer's machine and nothing on the runner"
            )
        })?;

    // Rule 2. The fast gates must not queue behind the suite.
    if host_test_job == GOVERNANCE_JOB {
        return Err(format!(
            "{WORKFLOW} runs `{HOST_SUITE}` inside `{GOVERNANCE_JOB}`, whose steps exist to \
             fail in seconds. The suite belongs in its own job beside the QEMU ones (`LE-100`)"
        ));
    }

    // Rule 3. A non-blocking test job is this loose end wearing a green tick.
    for line in job_lines(&lines, &host_test_job) {
        let text = &line.text;
        if text.starts_with("continue-on-error:") && !text.ends_with("false") {
            return Err(format!(
                "{WORKFLOW} job `{host_test_job}` runs the host suite with `{text}`, so a \
                 failing test does not fail the build -- which is the state `LE-100` \
                 describes, with a green tick over it"
            ));
        }
        if line.command
            && text.contains(HOST_SUITE)
            && (text.contains("|| true") || text.contains("|| exit 0"))
        {
            return Err(format!(
                "{WORKFLOW} job `{host_test_job}` swallows the suite's exit code: `{text}`"
            ));
        }
    }

    // Rule 4. Including this gate, which is what stops the recursion.
    for subcommand in CI_ENFORCED {
        let needle = format!("xtask -- {subcommand}");
        if !lines.iter().any(|line| line.command && line.text.contains(&needle)) {
            return Err(format!(
                "{WORKFLOW} never runs `cargo run -p xtask -- {subcommand}`, which this \
                 repository files as a CI-enforced mechanism. A gate is only as strong as the \
                 weakest place it is actually executed (`LE-72`, `LE-92`, `LE-100`)"
            ));
        }
    }

    Ok(CiGateSummary { host_test_job, job_count: jobs.len(), enforced_count: CI_ENFORCED.len() })
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

    /// A workflow with every rule satisfied, which each test below then breaks
    /// in exactly one place. Written as a fixture rather than derived from the
    /// committed file so a real regression cannot make these tests vacuous.
    fn sound_workflow() -> String {
        let mut yaml = String::from("name: CI\n\njobs:\n  governance-gates:\n    steps:\n");
        for subcommand in CI_ENFORCED {
            yaml.push_str(&format!("      - run: cargo run -p xtask -- {subcommand}\n"));
        }
        yaml.push_str("  host-tests:\n    steps:\n      - run: cargo test --workspace\n");
        yaml
    }

    /// `LE-100` itself: the committed workflow must run the suite. This is the
    /// assertion that was red when the loose end was filed.
    #[test]
    fn the_committed_workflow_runs_every_gate_this_repository_claims_to_enforce() {
        let summary = check_ci_gates(&repo_root()).expect("the committed workflow must pass");
        assert_ne!(summary.host_test_job, GOVERNANCE_JOB);
        assert_eq!(summary.enforced_count, CI_ENFORCED.len());
    }

    #[test]
    fn a_workflow_with_no_test_step_is_refused() {
        let yaml = sound_workflow().replace("cargo test --workspace", "cargo build --workspace");
        let error = check_workflow(&yaml).expect_err("no suite must fail");
        assert!(error.contains("LE-100"), "{error}");
        assert!(error.contains("runs no host tests"), "{error}");
    }

    /// The narrowing that would reopen the hole quietly: the step is still
    /// spelled `cargo test`, and covers one package.
    #[test]
    fn a_suite_narrowed_to_one_package_does_not_count_as_the_suite() {
        let yaml = sound_workflow().replace("cargo test --workspace", "cargo test -p kernel");
        check_workflow(&yaml).expect_err("a subset of packages is not the workspace suite");
    }

    /// **The mutation that made the first version of this gate stay silent.**
    ///
    /// It was found the only way it could be — by mutating the *committed*
    /// workflow rather than a fixture. Narrowing `run: cargo test --workspace`
    /// to `run: cargo test -p kernel`, with the step's own `- name:` still
    /// quoting the full command above it, left the gate green: the scan
    /// matched the display name. A `name:` is prose. Only what the runner
    /// executes counts, and the parser now says so.
    #[test]
    fn a_step_name_that_merely_quotes_the_suite_does_not_satisfy_the_gate() {
        let yaml = sound_workflow().replace(
            "      - run: cargo test --workspace\n",
            "      - name: cargo test --workspace\n        run: cargo test -p kernel\n",
        );
        let error = check_workflow(&yaml).expect_err("a display name is not a test run");
        assert!(error.contains("runs no host tests"), "{error}");
    }

    /// The same rule the other way, so the fix is not simply "reject `name:`":
    /// a command inside a `run: |` block scalar — the shape half this
    /// workflow's steps use — still counts.
    #[test]
    fn a_command_inside_a_block_scalar_counts_as_a_command() {
        let yaml = sound_workflow().replace(
            "      - run: cargo test --workspace\n",
            "      - name: the suite\n        run: |\n          cargo test --workspace\n",
        );
        check_workflow(&yaml).expect("a block scalar's body is executed");
    }

    /// Rule 2, and it must fail *only* for the placement — the same workflow
    /// with the job renamed passes, or this test is asserting something else.
    #[test]
    fn the_suite_may_not_live_in_the_fast_governance_job() {
        let yaml = sound_workflow().replace("  host-tests:\n", "  extra:\n").replace(
            "  governance-gates:\n    steps:\n",
            "  governance-gates:\n    steps:\n      - run: cargo test --workspace\n",
        );
        let error = check_workflow(&yaml).expect_err("the suite must not sit in the fast job");
        assert!(error.contains(GOVERNANCE_JOB), "{error}");
        check_workflow(&sound_workflow()).expect("the same rules pass when it has its own job");
    }

    #[test]
    fn a_non_blocking_test_job_is_refused() {
        let yaml = sound_workflow().replace(
            "  host-tests:\n    steps:\n",
            "  host-tests:\n    continue-on-error: true\n    steps:\n",
        );
        let error = check_workflow(&yaml).expect_err("a non-blocking suite must fail");
        assert!(error.contains("green tick"), "{error}");
    }

    #[test]
    fn an_explicitly_blocking_test_job_is_accepted() {
        let yaml = sound_workflow().replace(
            "  host-tests:\n    steps:\n",
            "  host-tests:\n    continue-on-error: false\n    steps:\n",
        );
        check_workflow(&yaml).expect("`continue-on-error: false` is the blocking spelling");
    }

    #[test]
    fn a_swallowed_exit_code_is_refused() {
        let yaml = sound_workflow()
            .replace("cargo test --workspace\n", "cargo test --workspace || true\n");
        let error = check_workflow(&yaml).expect_err("`|| true` must fail");
        assert!(error.contains("swallows"), "{error}");
    }

    /// Rule 4, one entry at a time. A list checked only in aggregate passes
    /// while covering whichever entry someone quietly dropped.
    #[test]
    fn every_enforced_subcommand_is_checked_individually() {
        for subcommand in CI_ENFORCED {
            let yaml = sound_workflow()
                .replace(&format!("xtask -- {subcommand}\n"), "xtask -- something-else\n");
            let error = check_workflow(&yaml)
                .expect_err(&format!("dropping `{subcommand}` from the workflow must fail"));
            assert!(error.contains(subcommand), "the error must name what is missing: {error}");
        }
    }

    /// The recursion rule 4 closes: this gate must be in its own list, or
    /// deleting its CI step is invisible to it.
    #[test]
    fn this_gate_asserts_that_this_gate_is_run() {
        assert!(CI_ENFORCED.contains(&"check-ci-gates"));
    }
}
