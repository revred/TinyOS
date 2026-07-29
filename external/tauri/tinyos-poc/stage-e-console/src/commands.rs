//! The console's four verbs, as Tauri commands — exactly the enumeration the signed
//! manifest carries, nothing else. Generic over the runtime so the same handler runs on
//! `MockRuntime` (the e2e tests) and on wry (the windowed app).

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::State;

use crate::authority::{Denial, DenialLog};
use crate::harness::{spawn_fixture, FixtureRun, Verdict, ONE_WAY_TRANSPORT};

/// Managed state behind the commands.
pub struct ConsoleState {
    os_dir: PathBuf,
    capture_dir: PathBuf,
    denials: DenialLog,
    manifest_verbs: Vec<String>,
    run: Mutex<Option<FixtureRun>>,
}

impl ConsoleState {
    /// `os_dir` is the TinyOS `os/` directory the harness runs `xtask` in; `capture_dir`
    /// receives serial capture files; `denials` is the resolver's shared log so the UI can
    /// render refusals; `manifest_verbs` is the verified enumeration, for display.
    pub fn new(
        os_dir: PathBuf,
        capture_dir: PathBuf,
        denials: DenialLog,
        manifest_verbs: Vec<String>,
    ) -> Self {
        Self { os_dir, capture_dir, denials, manifest_verbs, run: Mutex::new(None) }
    }
}

/// What `read_stream` answers: the whole picture the UI renders each poll.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StreamSnapshot {
    /// The running (or last) fixture name; empty when none has been launched.
    pub fixture: String,
    /// Serial lines so far.
    pub lines: Vec<String>,
    /// The verdict, once `xtask` has exited.
    pub verdict: Option<Verdict>,
    /// Every authority denial recorded since launch — the visible refusals.
    pub denials: Vec<Denial>,
    /// The verbs the signed manifest enumerates, for display.
    pub manifest_verbs: Vec<String>,
}

/// Launch a fixture through the `xtask` command surface. One run at a time: a second
/// launch while a fixture is still running is refused (terminate first).
#[tauri::command]
pub fn launch_fixture(state: State<'_, ConsoleState>, fixture: String) -> Result<String, String> {
    let mut slot = state.run.lock().expect("run lock");
    if let Some(run) = slot.as_ref() {
        if run.state().lock().expect("state lock").verdict.is_none() {
            return Err("a fixture is already running; terminate it first".into());
        }
    }
    std::fs::create_dir_all(&state.capture_dir)
        .map_err(|e| format!("cannot create capture directory: {e}"))?;
    let file_stem = if fixture.is_empty() { "default" } else { fixture.as_str() };
    let capture = state.capture_dir.join(format!("serial-{file_stem}.txt"));
    let run = spawn_fixture(&state.os_dir, &fixture, &capture)
        .map_err(|e| format!("failed to launch xtask in {}: {e}", state.os_dir.display()))?;
    *slot = Some(run);
    Ok(format!("launched fixture '{file_stem}' via cargo run -p xtask -- qemu-x86_64"))
}

/// Snapshot the stream: serial lines, verdict, denials, manifest. The UI polls this — so
/// every repaint of the console pane is itself an authority-resolved action.
#[tauri::command]
pub fn read_stream(state: State<'_, ConsoleState>) -> StreamSnapshot {
    let (fixture, lines, verdict) = match state.run.lock().expect("run lock").as_ref() {
        Some(run) => {
            let run_state = run.state();
            let run_state = run_state.lock().expect("state lock");
            (run_state.fixture.clone(), run_state.lines.clone(), run_state.verdict.clone())
        }
        None => (String::new(), Vec::new(), None),
    };
    StreamSnapshot {
        fixture,
        lines,
        verdict,
        denials: state.denials.lock().expect("denial lock").clone(),
        manifest_verbs: state.manifest_verbs.clone(),
    }
}

/// Send a line to the target. Enumerated and resolved — and then honest: the target
/// kernel's serial is TX-only today, so there is no receive path to deliver into.
#[tauri::command]
pub fn send_line(line: String) -> Result<(), String> {
    let _ = line;
    Err(ONE_WAY_TRANSPORT.into())
}

/// Terminate the running fixture (kills the cargo → xtask → QEMU tree).
#[tauri::command]
pub fn terminate(state: State<'_, ConsoleState>) -> Result<String, String> {
    let slot = state.run.lock().expect("run lock");
    match slot.as_ref() {
        Some(run) => {
            run.terminate();
            Ok("terminated".into())
        }
        None => Err("no fixture is running".into()),
    }
}

/// The invoke handler with exactly the four verbs registered.
pub fn handler<R: tauri::Runtime>(
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![launch_fixture, read_stream, send_line, terminate]
}
