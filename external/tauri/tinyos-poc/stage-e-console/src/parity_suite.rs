//! The parity-suite runner (17G §1.3): one action runs *all* the MS-DOS parity tests —
//! the host `shell` tests, then `check-shell-parity`, which itself boots the
//! `shell-batch` QEMU fixture and byte-compares the transcript against the committed
//! golden. Both children are the *same command surfaces CI uses* (`cargo test -p shell
//! --lib`, `cargo run -p xtask -- check-shell-parity`), so nothing here re-decides what
//! passing means — this module only streams, parses and aggregates.
//!
//! The two-signal rule (17G acceptance 3) is enforced in [`overall_verdict`]: the
//! fixture's in-guest exit verdict AND the transcript comparison must both be
//! affirmatively green; a missing signal is never a pass.

use std::io::BufRead;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::harness::Verdict;

/// One row of the PASS wall.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SuiteEntry {
    /// Test name as its own harness spells it.
    pub name: String,
    /// Did it pass?
    pub pass: bool,
}

/// Everything the parity tab renders, updated live by the runner thread.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct SuiteState {
    /// Per-test verdicts, in arrival order.
    pub entries: Vec<SuiteEntry>,
    /// Raw log lines (child stdout/stderr), for the tab's stream pane.
    pub lines: Vec<String>,
    /// Signal 1: the `shell-batch` fixture's in-guest `isa-debug-exit` verdict.
    pub fixture_signal: Option<bool>,
    /// Signal 2: the byte comparison against the committed golden transcript.
    pub transcript_signal: Option<bool>,
    /// The aggregate — set once, at the end. `None` while running.
    pub overall: Option<Verdict>,
    /// Whether a run has been started (refuses double starts).
    pub started: bool,
}

/// Shared view: the runner thread writes, `read_tab` reads.
pub type SharedSuiteState = Arc<Mutex<SuiteState>>;

/// Parse one line of `cargo test` output into a suite entry.
/// `test dos::tests::d1_bindings_dispatch ... ok` → pass;
/// `test x ... FAILED` → fail; `... ignored` and everything else → not an entry.
pub fn parse_cargo_test_line(line: &str) -> Option<SuiteEntry> {
    let rest = line.strip_prefix("test ")?;
    if rest.starts_with("result:") {
        return None;
    }
    let (name, outcome) = rest.rsplit_once(" ... ")?;
    match outcome.trim() {
        "ok" => Some(SuiteEntry { name: name.into(), pass: true }),
        "FAILED" => Some(SuiteEntry { name: name.into(), pass: false }),
        _ => None, // ignored / bench / has-output continuations
    }
}

/// Map `check-shell-parity`'s exit and output to the two signals.
///
/// Exit 0 prints one success line naming both facts → `(true, true)`. On failure the
/// error message distinguishes them: an in-guest assertion failure means the fixture
/// signal is red and the comparison never ran; a divergence message means the fixture
/// was green and the comparison is red. Anything else is a harness failure: both
/// signals stay unknown — and unknown never passes.
pub fn parse_parity_signals(exit_zero: bool, output: &str) -> (Option<bool>, Option<bool>) {
    if exit_zero && output.contains("transcript matches golden") {
        return (Some(true), Some(true));
    }
    if output.contains("in-guest assertion failure") {
        return (Some(false), None);
    }
    if output.contains("diverges from golden")
        || output.contains("extra content")
        || output.contains("ends early")
        || output.contains("trailing bytes")
    {
        return (Some(true), Some(false));
    }
    (None, None)
}

/// The aggregate verdict: PASS only if there is at least one host test, every entry
/// passed, and *both* parity signals are affirmatively true. Anything less is FAIL —
/// a missing signal, an empty wall and a red row all fail the same way.
pub fn overall_verdict(state: &SuiteState) -> Verdict {
    let host_green = !state.entries.is_empty() && state.entries.iter().all(|e| e.pass);
    let two_signals =
        state.fixture_signal == Some(true) && state.transcript_signal == Some(true);
    if host_green && two_signals {
        Verdict::Pass
    } else {
        Verdict::Fail
    }
}

/// Scrub the toolchain pins a cargo-spawned child inherits (same reasoning as
/// `harness::spawn_fixture`): the TinyOS tree pins its own nightly.
fn scrubbed(mut command: Command) -> Command {
    for var in
        ["RUSTUP_TOOLCHAIN", "CARGO", "RUSTC", "RUSTDOC", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"]
    {
        command.env_remove(var);
    }
    command
}

/// Run one child in `os_dir`, streaming every stdout/stderr line through `on_line`.
/// Returns whether the child exited 0, or an error line if it could not run at all.
fn run_streaming(
    os_dir: &Path,
    args: &[&str],
    state: &SharedSuiteState,
    mut on_line: impl FnMut(&str, &mut SuiteState) + Send,
) -> Result<bool, String> {
    let mut command = scrubbed(Command::new("cargo"));
    command
        .current_dir(os_dir)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| format!("cannot spawn cargo: {e}"))?;

    // Drain stderr on its own thread (build noise, xtask diagnostics) while this
    // thread parses stdout; both feed the shared log.
    let stderr_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let stderr_thread = child.stderr.take().map(|stderr| {
        let sink = Arc::clone(&stderr_lines);
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
                sink.lock().expect("stderr sink").push(line);
            }
        })
    });
    if let Some(stdout) = child.stdout.take() {
        for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
            let mut state = state.lock().expect("suite state");
            state.lines.push(line.clone());
            on_line(&line, &mut state);
        }
    }
    if let Some(thread) = stderr_thread {
        let _ = thread.join();
    }
    let status = child.wait().map_err(|e| format!("cannot wait for cargo: {e}"))?;
    {
        let mut state = state.lock().expect("suite state");
        for line in stderr_lines.lock().expect("stderr sink").drain(..) {
            state.lines.push(format!("[stderr] {line}"));
        }
    }
    Ok(status.success())
}

/// Run the whole suite on a background thread: host `shell` tests, then
/// `check-shell-parity` (fixture + golden comparison). Updates `state` live and sets
/// `overall` last. Never panics the app: every failure lands as a red verdict.
pub fn spawn_suite(os_dir: std::path::PathBuf, state: SharedSuiteState) {
    std::thread::spawn(move || {
        state.lock().expect("suite state").lines.push(
            "== 1/2: host parity tests \u{2014} cargo test -p shell --lib ==".into(),
        );
        let host_green = run_streaming(
            &os_dir,
            &["test", "-p", "shell", "--lib"],
            &state,
            |line, state| {
                if let Some(entry) = parse_cargo_test_line(line) {
                    state.entries.push(entry);
                }
            },
        );

        let phase2_start = {
            let mut state = state.lock().expect("suite state");
            state.lines.push(
                "== 2/2: target parity \u{2014} cargo run -p xtask -- check-shell-parity ==".into(),
            );
            state.lines.len()
        };
        let mut parity_output = String::new();
        let parity_green = run_streaming(
            &os_dir,
            &["run", "-p", "xtask", "--", "check-shell-parity"],
            &state,
            |line, _| {
                parity_output.push_str(line);
                parity_output.push('\n');
            },
        );

        let mut state = state.lock().expect("suite state");
        // Stderr carries check-shell-parity's failure detail; parse phase-2 lines only,
        // so a host-test failure message can never masquerade as a target signal.
        let logged: String = state.lines[phase2_start..].join("\n");
        let exit_zero = matches!(parity_green, Ok(true));
        let (fixture, transcript) =
            parse_parity_signals(exit_zero, &format!("{parity_output}\n{logged}"));
        state.fixture_signal = fixture;
        state.transcript_signal = transcript;
        state.entries.push(SuiteEntry {
            name: "shell-batch fixture: in-guest assertions (isa-debug-exit)".into(),
            pass: fixture == Some(true),
        });
        state.entries.push(SuiteEntry {
            name: "transcript vs committed golden (check-shell-parity)".into(),
            pass: transcript == Some(true),
        });
        if let Err(error) = host_green {
            state.lines.push(format!("[suite] host tests could not run: {error}"));
        }
        if let Err(error) = parity_green {
            state.lines.push(format!("[suite] check-shell-parity could not run: {error}"));
        }
        state.overall = Some(overall_verdict(&state));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// S1 — cargo-test lines parse to entries; non-entry lines do not.
    #[test]
    fn s1_cargo_test_lines_parse() {
        assert_eq!(
            parse_cargo_test_line("test dos::tests::d1_bindings_dispatch ... ok"),
            Some(SuiteEntry { name: "dos::tests::d1_bindings_dispatch".into(), pass: true })
        );
        assert_eq!(
            parse_cargo_test_line("test parity::tests::p1_transcript_matches_golden ... FAILED"),
            Some(SuiteEntry {
                name: "parity::tests::p1_transcript_matches_golden".into(),
                pass: false
            })
        );
        assert_eq!(parse_cargo_test_line("test parity::tests::regenerate_golden ... ignored"), None);
        assert_eq!(parse_cargo_test_line("test result: ok. 22 passed; 0 failed"), None);
        assert_eq!(parse_cargo_test_line("running 22 tests"), None);
    }

    /// S2 — the two signals map exactly as `check-shell-parity` speaks: success names
    /// both; an assertion failure reds the fixture and leaves the comparison unknown; a
    /// divergence greens the fixture and reds the comparison; noise stays unknown.
    #[test]
    fn s2_parity_signals_map() {
        let ok = "shell-parity: transcript matches golden (61 lines) and the fixture's in-guest assertions passed";
        assert_eq!(parse_parity_signals(true, ok), (Some(true), Some(true)));
        assert_eq!(
            parse_parity_signals(
                false,
                "xtask: shell parity failed: shell-batch fixture reported in-guest assertion failure (exit 1)"
            ),
            (Some(false), None)
        );
        assert_eq!(
            parse_parity_signals(
                false,
                "xtask: shell parity failed: transcript diverges from golden at line 7:"
            ),
            (Some(true), Some(false))
        );
        assert_eq!(parse_parity_signals(false, "error: could not compile"), (None, None));
    }

    /// S3 — the two-signal rule: a missing signal is never a pass, and neither is an
    /// empty wall or one red host test.
    #[test]
    fn s3_two_signal_rule_holds() {
        let green = |name: &str| SuiteEntry { name: name.into(), pass: true };
        let mut state = SuiteState {
            entries: vec![green("a"), green("b")],
            fixture_signal: Some(true),
            transcript_signal: Some(true),
            ..Default::default()
        };
        assert_eq!(overall_verdict(&state), Verdict::Pass);

        state.transcript_signal = None; // fixture green, comparison missing
        assert_eq!(overall_verdict(&state), Verdict::Fail);
        state.transcript_signal = Some(true);
        state.fixture_signal = None;
        assert_eq!(overall_verdict(&state), Verdict::Fail);
        state.fixture_signal = Some(true);
        state.entries.push(SuiteEntry { name: "red".into(), pass: false });
        assert_eq!(overall_verdict(&state), Verdict::Fail);
        let empty = SuiteState {
            fixture_signal: Some(true),
            transcript_signal: Some(true),
            ..Default::default()
        };
        assert_eq!(overall_verdict(&empty), Verdict::Fail);
    }
}
