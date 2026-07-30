//! The console's verbs, as Tauri commands — exactly the enumeration the signed manifest
//! carries, nothing else. Generic over the runtime so the same handlers run on
//! `MockRuntime` (the e2e tests) and on wry (the windowed app).
//!
//! Two grant tables since 17G (both signed): the chrome identity manages the target and
//! the tab set (`launch_fixture`, `read_stream`, `send_line`, `terminate`, `open_tab`,
//! `focus_tab`, `read_console`); tab identities own their session (`run_line`,
//! `read_tab`, `run_parity`). Session identity for a tab verb is the invoking webview's
//! *runtime-derived label*, never a request argument — a tab cannot name another tab's
//! session because there is nowhere to write one.
//!
//! Commands mutate registry state only. Window composition (creating sibling webviews,
//! geometry, the reserved-region repaint) is the windowed shell's reconciler, kept out
//! of the verb surface entirely.

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::State;

use crate::authority::{Denial, DenialLog};
use crate::harness::{spawn_fixture, FixtureRun, Verdict, ONE_WAY_TRANSPORT};
use crate::parity_suite::{self, SharedSuiteState, SuiteEntry};
use crate::tabs::{TabInfo, TabKind, TabRegistry};

/// Managed state behind the commands.
pub struct ConsoleState {
    os_dir: PathBuf,
    capture_dir: PathBuf,
    denials: DenialLog,
    manifest_verbs: Vec<String>,
    manifest_tab_verbs: Vec<String>,
    run: Mutex<Option<FixtureRun>>,
    tabs: Mutex<TabRegistry>,
    suite: SharedSuiteState,
}

impl ConsoleState {
    /// `os_dir` is the TinyOS `os/` directory the harness runs `xtask` in; `capture_dir`
    /// receives serial capture files; `denials` is the resolver's shared log so the UI can
    /// render refusals; the verb lists are the verified enumerations, for display.
    pub fn new(
        os_dir: PathBuf,
        capture_dir: PathBuf,
        denials: DenialLog,
        manifest_verbs: Vec<String>,
        manifest_tab_verbs: Vec<String>,
    ) -> Self {
        Self {
            os_dir,
            capture_dir,
            denials,
            manifest_verbs,
            manifest_tab_verbs,
            run: Mutex::new(None),
            tabs: Mutex::new(TabRegistry::new()),
            suite: SharedSuiteState::default(),
        }
    }

    /// The tab registry — the windowed shell's reconciler reads it to compose the
    /// window (webviews, focus, the reserved region), outside the verb surface.
    pub fn tabs(&self) -> &Mutex<TabRegistry> {
        &self.tabs
    }

    /// The parity-suite state, for the reconciler's reserved-region verdict.
    pub fn suite(&self) -> &SharedSuiteState {
        &self.suite
    }

    /// The denial log, for smoke evidence.
    pub fn denials(&self) -> &DenialLog {
        &self.denials
    }

    /// Compose the reserved-region line: focused-tab identity and flavour from the
    /// registry, the parity verdict, and the denial count. Painted only by the Rust
    /// side; no tab content string feeds it.
    pub fn reserved_text(&self) -> String {
        let base = self.tabs.lock().expect("tab registry").reserved_line();
        let suite = self.suite.lock().expect("suite state");
        let parity = match (&suite.overall, suite.started) {
            (Some(Verdict::Pass), _) => "PASS",
            (Some(_), _) => "FAIL",
            (None, true) => "running\u{2026}",
            (None, false) => "not run",
        };
        let denials = self.denials.lock().expect("denial log").len();
        format!("{base} \u{2014} parity suite: {parity} \u{2014} authority denials: {denials}")
    }
}

/// What `read_stream` answers: the whole picture the chrome's target pane renders.
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
    /// The verbs the signed manifest enumerates for the chrome, for display.
    pub manifest_verbs: Vec<String>,
}

/// What `read_console` answers: the tab bar's world.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConsoleSnapshot {
    /// Every open tab, in open order.
    pub tabs: Vec<TabInfo>,
    /// The focused tab's label, if any.
    pub focused: Option<String>,
    /// The reserved-region line the host paints (for the chrome's own reference —
    /// the reserved webview itself is repainted Rust-side).
    pub reserved: String,
    /// Every authority denial recorded so far.
    pub denials: Vec<Denial>,
    /// The chrome verbs the signed manifest enumerates.
    pub manifest_verbs: Vec<String>,
    /// The tab verbs the signed manifest enumerates.
    pub manifest_tab_verbs: Vec<String>,
}

/// What `read_tab` answers: one tab's whole picture.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TabSnapshot {
    /// The tab's identity card.
    pub info: TabInfo,
    /// The session transcript (DOS tabs).
    pub transcript: String,
    /// The parity suite state (parity tab only).
    pub suite: Option<SuiteSnapshot>,
}

/// The parity tab's renderable state.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SuiteSnapshot {
    /// Whether a run has been started.
    pub started: bool,
    /// Per-test PASS/FAIL rows, in arrival order.
    pub entries: Vec<SuiteEntry>,
    /// The tail of the raw log.
    pub lines: Vec<String>,
    /// Signal 1: fixture in-guest verdict.
    pub fixture_signal: Option<bool>,
    /// Signal 2: transcript-vs-golden comparison.
    pub transcript_signal: Option<bool>,
    /// The aggregate verdict, once both children finished.
    pub overall: Option<Verdict>,
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

/// Snapshot the target stream: serial lines, verdict, denials, manifest. The UI polls
/// this — so every repaint of the console pane is itself an authority-resolved action.
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

/// Open a tab (`"dos"` or `"parity"`). Chrome verb: tabs are opened by the tab bar,
/// never by other tabs. The refusal beyond the enumerated capacity is typed and visible.
#[tauri::command]
pub fn open_tab(state: State<'_, ConsoleState>, kind: String) -> Result<TabInfo, String> {
    let kind = match kind.as_str() {
        "dos" => TabKind::Dos,
        "parity" => TabKind::Parity,
        other => return Err(format!("unknown tab kind '{other}': expected 'dos' or 'parity'")),
    };
    state.tabs.lock().expect("tab registry").open(kind).map_err(|e| e.to_string())
}

/// Give a tab the focus (chrome verb).
#[tauri::command]
pub fn focus_tab(state: State<'_, ConsoleState>, label: String) -> Result<(), String> {
    state.tabs.lock().expect("tab registry").focus(&label).map_err(|e| e.to_string())
}

/// Snapshot the tab bar's world (chrome verb).
#[tauri::command]
pub fn read_console(state: State<'_, ConsoleState>) -> ConsoleSnapshot {
    let (tabs, focused) = {
        let tabs = state.tabs.lock().expect("tab registry");
        (tabs.infos(), tabs.focused().map(|t| t.label().to_string()))
    };
    ConsoleSnapshot {
        tabs,
        focused,
        reserved: state.reserved_text(),
        denials: state.denials.lock().expect("denial lock").clone(),
        manifest_verbs: state.manifest_verbs.clone(),
        manifest_tab_verbs: state.manifest_tab_verbs.clone(),
    }
}

/// Run one DOS line in the invoking tab's own session. The session is the webview's
/// runtime-derived label — there is no argument through which another tab can be named.
#[tauri::command]
pub fn run_line<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    state: State<'_, ConsoleState>,
    line: String,
) -> Result<(), String> {
    state
        .tabs
        .lock()
        .expect("tab registry")
        .run_line(webview.label(), &line)
        .map_err(|e| e.to_string())
}

/// Snapshot the invoking tab: transcript for DOS tabs, suite state for the parity tab.
#[tauri::command]
pub fn read_tab<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    state: State<'_, ConsoleState>,
) -> Result<TabSnapshot, String> {
    let tabs = state.tabs.lock().expect("tab registry");
    let tab = tabs.get(webview.label()).ok_or("no tab carries this label")?;
    let suite = match tab.kind() {
        TabKind::Parity => {
            let suite = state.suite.lock().expect("suite state");
            Some(SuiteSnapshot {
                started: suite.started,
                entries: suite.entries.clone(),
                lines: suite.lines.iter().rev().take(40).rev().cloned().collect(),
                fixture_signal: suite.fixture_signal,
                transcript_signal: suite.transcript_signal,
                overall: suite.overall.clone(),
            })
        }
        TabKind::Dos => None,
    };
    Ok(TabSnapshot { info: tab.info(), transcript: tab.transcript().into(), suite })
}

/// Run the whole MS-DOS parity suite — host `shell` tests, then `check-shell-parity`
/// (the `shell-batch` fixture under QEMU plus the golden comparison). Only the parity
/// tab's own identity resolves this verb; one run at a time.
#[tauri::command]
pub fn run_parity<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    state: State<'_, ConsoleState>,
) -> Result<String, String> {
    {
        let tabs = state.tabs.lock().expect("tab registry");
        let tab = tabs.get(webview.label()).ok_or("no tab carries this label")?;
        if tab.kind() != TabKind::Parity {
            return Err("the verb does not match this tab's session kind".into());
        }
    }
    {
        let mut suite = state.suite.lock().expect("suite state");
        if suite.started && suite.overall.is_none() {
            return Err("the parity suite is already running".into());
        }
        *suite = Default::default();
        suite.started = true;
    }
    parity_suite::spawn_suite(state.os_dir.clone(), state.suite.clone());
    Ok("parity suite started: cargo test -p shell --lib, then check-shell-parity".into())
}

/// The invoke handler with exactly the manifest's verbs registered.
pub fn handler<R: tauri::Runtime>(
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        launch_fixture,
        read_stream,
        send_line,
        terminate,
        open_tab,
        focus_tab,
        read_console,
        run_line,
        read_tab,
        run_parity
    ]
}
