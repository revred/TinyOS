//! The QEMU fixture harness: the console's Rust side owns the target.
//!
//! Launch goes through the *same command surface* CI uses — `cargo run -p xtask --
//! qemu-x86_64 --fixture=<name> --serial-capture=<path>` in the TinyOS `os/` directory — so
//! fixture validation, the QEMU invocation, the boot-time budget and the
//! `isa-debug-exit` → exit-code mapping are all `xtask`'s own, not a reimplementation that
//! could drift. The console adds exactly two things: a live tail of the serial capture, and
//! a UI-visible verdict computed from `xtask`'s exit code by [`verdict_from_exit`] — the
//! same PASS/FAIL CI would report.
//!
//! One deliberate absence: there is no input path. The target kernel's serial driver is
//! TX-only (`hal-x86_64/src/serial.rs` has no receive path and no fixture reads one), so a
//! `send_line` reaching this harness fails with [`ONE_WAY_TRANSPORT`] rather than
//! pretending. The verb still exists — it is enumerated, resolved and transported the day
//! the target grows an RX path; the gap analysis records the asymmetry.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The verdict the UI shows — the same bit `xtask` computes from `isa-debug-exit`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "detail")]
pub enum Verdict {
    /// `xtask` exit 0: the fixture reached its success exit.
    Pass,
    /// `xtask` exit 1: the fixture reached its failure exit.
    Fail,
    /// The operator terminated the run before a verdict.
    Terminated,
    /// Anything else — `xtask` harness error, unexpected code, or death by signal.
    Harness(String),
}

/// Map an `xtask qemu-x86_64` exit code to the verdict. Mirrors `XtaskExit`:
/// 0 = kernel boot succeeded, 1 = kernel boot failed, 2 = harness error.
pub fn verdict_from_exit(code: Option<i32>) -> Verdict {
    match code {
        Some(0) => Verdict::Pass,
        Some(1) => Verdict::Fail,
        Some(other) => Verdict::Harness(format!("xtask exited with unexpected code {other}")),
        None => Verdict::Harness("xtask terminated without an exit code".into()),
    }
}

/// The error `send_line` answers until the target has a serial RX path.
pub const ONE_WAY_TRANSPORT: &str = "transport is one-way: the target kernel's serial is \
     TX-only (no UART RX path exists in hal-x86_64); the verb is enumerated and resolved, \
     but there is nothing on the other end to receive — see docs/terminal-gap-analysis.md";

/// Everything the UI polls: the fixture name, the serial lines so far, and the verdict once
/// there is one.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RunState {
    /// The fixture launched (empty string = the default boot fixture).
    pub fixture: String,
    /// Serial lines tailed from the capture file, in arrival order.
    pub lines: Vec<String>,
    /// `None` while the fixture is still running.
    pub verdict: Option<Verdict>,
}

/// Shared view of a run: the harness thread writes it, `read_stream` reads it.
pub type SharedRunState = Arc<Mutex<RunState>>;

/// A launched fixture: shared state plus the handle needed to terminate it.
pub struct FixtureRun {
    state: SharedRunState,
    child: Arc<Mutex<Option<Child>>>,
}

impl FixtureRun {
    /// The shared state for polling.
    pub fn state(&self) -> SharedRunState {
        Arc::clone(&self.state)
    }

    /// Terminate the run: kill the `cargo`/`xtask`/QEMU process tree and record the
    /// verdict as [`Verdict::Terminated`] (unless a real verdict already landed).
    pub fn terminate(&self) {
        let mut slot = self.child.lock().expect("child lock");
        if let Some(child) = slot.as_mut() {
            // `cargo run` is a process tree (cargo → xtask → QEMU); `Child::kill` reaps
            // only the root, so use taskkill /T to take the tree down with it.
            let _ = Command::new("taskkill")
                .args(["/PID", &child.id().to_string(), "/T", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = child.wait();
            *slot = None;
            let mut state = self.state.lock().expect("state lock");
            if state.verdict.is_none() {
                state.verdict = Some(Verdict::Terminated);
            }
        }
    }
}

/// Read whatever `path` holds beyond `offset`. Returns the new bytes (may split a line;
/// the caller keeps the remainder) and the new offset. A file that does not exist yet is
/// simply "nothing new" — QEMU creates it when it opens the serial backend.
pub fn drain_new_bytes(path: &Path, offset: u64) -> (Vec<u8>, u64) {
    let Ok(mut file) = std::fs::File::open(path) else {
        return (Vec::new(), offset);
    };
    if file.seek(SeekFrom::Start(offset)).is_err() {
        return (Vec::new(), offset);
    }
    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return (Vec::new(), offset);
    }
    let new_offset = offset + buf.len() as u64;
    (buf, new_offset)
}

/// Split `pending` on newlines, pushing complete lines into `lines` and leaving the
/// unterminated tail in `pending`.
pub fn take_complete_lines(pending: &mut Vec<u8>, lines: &mut Vec<String>) {
    while let Some(pos) = pending.iter().position(|&b| b == b'\n') {
        let line: Vec<u8> = pending.drain(..=pos).collect();
        let text = String::from_utf8_lossy(&line);
        lines.push(text.trim_end_matches(['\n', '\r']).to_string());
    }
}

/// Launch `fixture` through `cargo run -p xtask -- qemu-x86_64` in `os_dir`, tailing the
/// serial capture into the shared state and recording the verdict when `xtask` exits.
///
/// `fixture` empty selects the default boot fixture (no `--fixture=` argument), exactly as
/// the CLI does. Unknown fixture names are *not* pre-validated here: `xtask` rejects them
/// itself with a harness error, and that refusal surfaces as the run's verdict — reusing
/// the fail-closed validation rather than duplicating it.
pub fn spawn_fixture(
    os_dir: &Path,
    fixture: &str,
    capture_path: &Path,
) -> std::io::Result<FixtureRun> {
    // A stale capture from a previous run must not replay into this one's stream.
    let _ = std::fs::remove_file(capture_path);

    let mut command = Command::new("cargo");
    // The console itself is built by cargo, and a cargo-spawned process inherits the
    // parent's toolchain pins (`RUSTUP_TOOLCHAIN`, `CARGO`, `RUSTC`, rustflags). The
    // TinyOS tree pins its own nightly in `rust-toolchain.toml`; a leaked stable pin
    // makes its `-Z build-std` build fail with a harness error. Scrub, don't inherit.
    for var in
        ["RUSTUP_TOOLCHAIN", "CARGO", "RUSTC", "RUSTDOC", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"]
    {
        command.env_remove(var);
    }
    command
        .current_dir(os_dir)
        .args(["run", "-p", "xtask", "--", "qemu-x86_64"])
        .arg(format!("--serial-capture={}", capture_path.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if !fixture.is_empty() {
        command.arg(format!("--fixture={fixture}"));
    }
    let mut child = command.spawn()?;

    let stderr = child.stderr.take();
    let state: SharedRunState =
        Arc::new(Mutex::new(RunState { fixture: fixture.to_string(), ..Default::default() }));
    let child = Arc::new(Mutex::new(Some(child)));
    let run = FixtureRun { state: Arc::clone(&state), child: Arc::clone(&child) };

    // `xtask`'s own diagnostics (bad fixture name, build failure) belong in the stream too.
    if let Some(stderr) = stderr {
        let state = Arc::clone(&state);
        std::thread::spawn(move || {
            use std::io::BufRead;
            for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
                state.lock().expect("state lock").lines.push(format!("[xtask] {line}"));
            }
        });
    }

    let capture: PathBuf = capture_path.to_path_buf();
    std::thread::spawn(move || {
        let mut offset = 0u64;
        let mut pending: Vec<u8> = Vec::new();
        loop {
            let (bytes, new_offset) = drain_new_bytes(&capture, offset);
            offset = new_offset;
            if !bytes.is_empty() {
                pending.extend_from_slice(&bytes);
                let mut state = state.lock().expect("state lock");
                take_complete_lines(&mut pending, &mut state.lines);
            }
            let exited = {
                let mut slot = child.lock().expect("child lock");
                match slot.as_mut() {
                    None => break, // terminated by the operator; verdict already set
                    Some(process) => process.try_wait().ok().flatten(),
                }
            };
            if let Some(status) = exited {
                // Final drain: QEMU may have flushed between our last poll and its exit.
                let (bytes, _) = drain_new_bytes(&capture, offset);
                pending.extend_from_slice(&bytes);
                let mut state = state.lock().expect("state lock");
                take_complete_lines(&mut pending, &mut state.lines);
                if !pending.is_empty() {
                    state.lines.push(String::from_utf8_lossy(&pending).into_owned());
                }
                state.verdict = Some(verdict_from_exit(status.code()));
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });

    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// H1 — the exit-code mapping is exactly `xtask`'s: 0 PASS, 1 FAIL, everything else a
    /// harness error, never a pass.
    #[test]
    fn h1_verdict_mapping_matches_xtask() {
        assert_eq!(verdict_from_exit(Some(0)), Verdict::Pass);
        assert_eq!(verdict_from_exit(Some(1)), Verdict::Fail);
        assert!(matches!(verdict_from_exit(Some(2)), Verdict::Harness(_)));
        assert!(matches!(verdict_from_exit(Some(33)), Verdict::Harness(_)));
        assert!(matches!(verdict_from_exit(None), Verdict::Harness(_)));
    }

    /// H2 — tailing streams progressively: bytes appear as written, lines complete only at
    /// newlines, a missing file is "nothing yet" rather than an error.
    #[test]
    fn h2_tail_streams_progressively() {
        let dir = std::env::temp_dir().join("stage-e-h2");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("capture.txt");
        let _ = std::fs::remove_file(&path);

        // Not created yet: no bytes, offset unchanged.
        assert_eq!(drain_new_bytes(&path, 0), (Vec::new(), 0));

        std::fs::write(&path, b"first line\nsecond ").unwrap();
        let (bytes, offset) = drain_new_bytes(&path, 0);
        let mut pending = bytes;
        let mut lines = Vec::new();
        take_complete_lines(&mut pending, &mut lines);
        assert_eq!(lines, vec!["first line".to_string()]);
        assert_eq!(pending, b"second ");

        // The rest of the second line arrives.
        let mut file = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        use std::io::Write;
        file.write_all(b"half\r\n").unwrap();
        drop(file);
        let (bytes, _) = drain_new_bytes(&path, offset);
        pending.extend_from_slice(&bytes);
        take_complete_lines(&mut pending, &mut lines);
        assert_eq!(lines, vec!["first line".to_string(), "second half".to_string()]);
        assert!(pending.is_empty());
    }

    /// H3 (live, `--ignored`) — the real thing end to end: launch the `measure` fixture
    /// through the real `xtask` in the tree named by `TINYOS_OS_DIR`, watch real serial
    /// lines arrive, and get the PASS verdict `xtask` computed from `isa-debug-exit`.
    #[test]
    #[ignore = "needs TINYOS_OS_DIR, the x86_64 toolchain and QEMU; run explicitly for the Stage E evidence"]
    fn h3_live_measure_fixture_passes() {
        let os_dir = PathBuf::from(
            std::env::var("TINYOS_OS_DIR").expect("set TINYOS_OS_DIR to the TinyOS os/ directory"),
        );
        let capture = std::env::temp_dir().join("stage-e-h3-capture.txt");
        let run = spawn_fixture(&os_dir, "measure", &capture).expect("spawn must succeed");

        let deadline = std::time::Instant::now() + Duration::from_secs(300);
        let verdict = loop {
            if let Some(verdict) = run.state().lock().unwrap().verdict.clone() {
                break verdict;
            }
            assert!(std::time::Instant::now() < deadline, "no verdict within the time budget");
            std::thread::sleep(Duration::from_millis(200));
        };

        assert_eq!(verdict, Verdict::Pass, "the measure fixture must reach its success exit");
        let state = run.state();
        let state = state.lock().unwrap();
        assert!(
            state.lines.iter().any(|line| line.contains("TINYOS-MEAS/2")),
            "the live serial stream must contain the measurement envelope; got {} lines",
            state.lines.len()
        );
    }

    /// H4 — `send_line` is honest about the one-way transport: the constant exists and
    /// names the reason (the UI surfaces it verbatim).
    #[test]
    fn h4_send_line_names_the_one_way_transport() {
        assert!(ONE_WAY_TRANSPORT.contains("TX-only"));
        assert!(ONE_WAY_TRANSPORT.contains("terminal-gap-analysis"));
    }
}
