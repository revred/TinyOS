//! V1 — the Ti64 operator console (06A / work/UX-V1), windowed.
//!
//! Composition, and nothing else: the signed manifest (embedded at rest, verified before
//! anything runs), the `ConsoleAuthority` resolver installed over the fork's Stage C seam,
//! the verbs from `stage-e-console::commands`, and **one window, sibling webviews**:
//!
//! - `console` — the V1 chrome (`ui/index.html`, the work/UX-V1 reference build): every
//!   visible pixel except the system line. Holds the chrome verbs only.
//! - `reserved` — the host-owned system line, the LAST line of the window (SPEC §10.1).
//!   Its label is enumerated nowhere in the manifest, so it holds no verbs (e8/R6); it
//!   is repainted exclusively by [`reconcile`] on the Rust side, and no tab content can
//!   reach it.
//! - `tab-1`…`tab-6` — per-tab identity satellites, siblings created by the reconciler
//!   from registry state. Each holds the tab verbs under its own runtime label and
//!   relays snapshots to the chrome over the same-origin UI bus; no capability crosses
//!   that bus.
//!
//! `STAGE_E_SMOKE=1` runs the V1 acceptance sequence unattended, driving the chrome's
//! keyboard grammar via `window.smokeKey` (`STORY-UX-04`) — boot pre-run + audited
//! denial, Ctrl+T, typed isolation, `SAMPLE.TCB`, F8 parity, resolver denials — writing
//! PNG screenshots and the evidence JSON, then exits with the aggregate verdict.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use stage_e_console::authority::ConsoleAuthority;
use stage_e_console::commands::{self, ConsoleState};
use stage_e_console::manifest::SignedManifest;
use stage_e_console::tabs::TabKind;
use tauri::{LogicalPosition, LogicalSize, Manager, WebviewUrl};

/// The at-rest manifest and the key the console trusts, fixed at build time.
const MANIFEST_JSON: &str = include_str!("../../stage-e-console/manifest/console-manifest.json");
const PUBKEY_HEX: &str = include_str!("../../stage-e-console/manifest/console-pubkey.hex");

/// Window geometry — the V1 acceptance boot is 1440×860 (V1-STRATEGY Part H). Still
/// non-resizable: `STORY-UX-01` (V1.3) removes these constants and derives regions from
/// `WindowEvent::Resized`.
const WIN_W: f64 = 1440.0;
const WIN_H: f64 = 860.0;
/// The host-owned system line: the LAST line of the window (SPEC §10.1), 28 px tall,
/// overlaying the chrome's `.reserved` status-text segment — after the Ti64 pill
/// (2 + 92 + 6 margin + 14 gap = x 114) and clear of the budget readout and layout
/// picker on the right. Its label is enumerated in no grant table; `reconcile` is its
/// only writer.
const RESERVED_X: f64 = 114.0;
const RESERVED_Y: f64 = WIN_H - 28.0;
const RESERVED_W: f64 = 820.0;
const RESERVED_H: f64 = 28.0;

/// The TinyOS `os/` directory: `TINYOS_OS_DIR`, or found by walking up from this crate to
/// the repository root — the tree lives in-repo at `external/tauri/tinyos-poc/`, so the
/// first ancestor carrying `os/targets/x86_64-tinyos.json` is the TinyOS checkout.
fn os_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("TINYOS_OS_DIR") {
        return PathBuf::from(dir);
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .map(|ancestor| ancestor.join("os"))
        .find(|candidate| candidate.join("targets").join("x86_64-tinyos.json").exists())
        .expect("no TinyOS os/ directory above this crate and TINYOS_OS_DIR is unset")
}

/// One reconciler tick, on the main thread: create missing tab identity satellites as
/// siblings, and repaint the host-owned system line. The V1 chrome (`index.html`)
/// renders every visible pixel except that line; a tab webview exists per open tab
/// because only its runtime label may invoke the tab verbs — it is a 1×1 identity
/// satellite, not a visual region, until `STORY-UX-02` (V1.3) adds real suspension.
/// The only writer the reserved webview ever has is the `eval` below.
fn reconcile(app: &tauri::AppHandle) {
    let state = app.state::<ConsoleState>();
    let infos = state.tabs().lock().expect("tab registry").infos();
    if let Some(window) = app.get_window("host") {
        for info in &infos {
            if app.get_webview(&info.label).is_none() {
                let _ = window.add_child(
                    tauri::webview::WebviewBuilder::new(
                        info.label.clone(),
                        WebviewUrl::App("tab.html".into()),
                    ),
                    LogicalPosition::new(WIN_W - 1.0, 0.0),
                    LogicalSize::new(1.0, 1.0),
                );
            }
            // Always shown: a hidden webview throttles its timers, and the satellite
            // must keep polling read_tab under its own identity (Part F seam 3).
            if let Some(webview) = app.get_webview(&info.label) {
                let _ = webview.show();
            }
        }
    }
    if let Some(reserved) = app.get_webview("reserved") {
        let text = serde_json::to_string(&state.reserved_text()).expect("a string serializes");
        let _ = reserved.eval(format!("document.getElementById('t').textContent = {text}"));
    }
}

/// Capture the window's OWN rendered surface to `dir/<name>.png` via `PrintWindow`
/// with `PW_RENDERFULLCONTENT` (DWM renders the window regardless of z-order), so the
/// unattended run never needs the window on top, focused, or unobstructed — the
/// operator's desktop stays theirs while the smoke runs. Returns the path on success;
/// a failed capture is reported in the smoke JSON, never a panic.
fn screenshot(window: &tauri::Window<tauri::Wry>, dir: &Path, name: &str) -> Option<String> {
    let hwnd = window.hwnd().ok()?.0 as isize;
    let size = window.outer_size().ok()?;
    let path = dir.join(format!("{name}.png"));
    let script = format!(
        "Add-Type -AssemblyName System.Drawing; \
         $sig = '[System.Runtime.InteropServices.DllImport(\"user32.dll\")] public static extern bool PrintWindow(System.IntPtr hwnd, System.IntPtr hdc, uint flags);'; \
         Add-Type -MemberDefinition $sig -Name Native -Namespace Cap; \
         $b = New-Object System.Drawing.Bitmap({w}, {h}); \
         $g = [System.Drawing.Graphics]::FromImage($b); \
         $hdc = $g.GetHdc(); \
         $null = [Cap.Native]::PrintWindow([System.IntPtr]{hwnd}, $hdc, 2); \
         $g.ReleaseHdc($hdc); \
         $g.Dispose(); \
         $b.Save('{path}', [System.Drawing.Imaging.ImageFormat]::Png)",
        w = size.width,
        h = size.height,
        hwnd = hwnd,
        path = path.display(),
    );
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .ok()?;
    status.success().then(|| path.display().to_string())
}

/// Evaluate `script` in webview `label`, if it exists.
fn eval_in(app: &tauri::AppHandle, label: &str, script: &str) {
    if let Some(webview) = app.get_webview(label) {
        let _ = webview.eval(script);
    }
}

/// Poll `predicate` against the console state until it holds or `budget` elapses.
fn wait_for(
    app: &tauri::AppHandle,
    budget: Duration,
    predicate: impl Fn(&ConsoleState) -> bool,
) -> bool {
    let deadline = std::time::Instant::now() + budget;
    loop {
        if predicate(&app.state::<ConsoleState>()) {
            return true;
        }
        if std::time::Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// The unattended V1 acceptance sequence (`STORY-UX-04`): the run drives the chrome's
/// keyboard grammar through `window.smokeKey`, so every step travels the real UI path —
/// chrome keystrokes → UI bus → the tab's own identity satellite → `run_line`. Returns
/// the evidence JSON and the process exit code.
fn smoke_sequence(
    app: &tauri::AppHandle,
    window: &tauri::Window<tauri::Wry>,
    shots_dir: &Path,
) -> (serde_json::Value, i32) {
    let mut shots: Vec<Option<String>> = Vec::new();
    let mut shot = |name: &str| {
        // Give the reconciler and the pages two ticks to paint before capturing.
        // PrintWindow captures the occluded window fine; only a minimized one has no
        // surface to render, so restore it in that single case — never steal focus.
        if window.is_minimized().unwrap_or(false) {
            let _ = window.unminimize();
        }
        std::thread::sleep(Duration::from_millis(1200));
        shots.push(screenshot(window, shots_dir, name).map(|p| format!("{name}.png: {p}")));
    };
    let key = |name: &str| {
        eval_in(app, "console", &format!("window.smokeKey('{name}')"));
        std::thread::sleep(Duration::from_millis(300));
    };

    std::thread::sleep(Duration::from_secs(4)); // chrome boot: tx01 + pre-run sequence
    // No set_focus and no always-on-top anywhere in this run: every check reads host
    // state over IPC, smokeKey dispatches in-page events, and PrintWindow captures an
    // occluded window — the smoke must never fight the operator for the desktop.

    // Boot: the chrome opened tx01 (tab-1) and pre-ran VER · DIR · SET · TASKMGR ·
    // TASKKILL RT-CTRL · TYPE README.TXT — the transcript must show the real banner
    // AND the audited TASKKILL denial (deep red on screen, counted on the system line).
    let boot_ran = wait_for(app, Duration::from_secs(30), |state| {
        state.tabs().lock().expect("registry").get("tab-1").is_some_and(|t| {
            t.transcript().contains("TinyOS Version")
                && t.transcript().contains("Access denied")
                && t.transcript().contains("Directory of A:\\")
        })
    });
    shot("01-boot-cockpit");

    // Ctrl+T duplicates the focused kind: a second DOS session. The chrome names it
    // tx04 (tx02/tx03 are the rt/agent display tabs) and opens host slot 4 to match.
    key("Ctrl+T");
    let second_dos = wait_for(app, Duration::from_secs(20), |state| {
        let infos = state.tabs().lock().expect("registry").infos();
        infos.iter().filter(|i| i.kind == TabKind::Dos).count() >= 2
    });
    shot("02-second-dos-via-ctrl-t");

    // Isolation, typed at the prompt: SET in the new tab (tab-4), the same variable
    // undefined in tx01 (tab-1) — the per-tab World boundary, visibly.
    key("type-line:SET GREET=HELLO-17G");
    key("Enter");
    key("type-line:SET GREET");
    key("Enter");
    let set_ran = wait_for(app, Duration::from_secs(20), |state| {
        state
            .tabs()
            .lock()
            .expect("registry")
            .get("tab-4")
            .is_some_and(|t| t.transcript().contains("GREET=HELLO-17G"))
    });
    shot("03-set-in-new-tab");
    key("Ctrl+1");
    key("type-line:SET GREET");
    key("Enter");
    let isolated = wait_for(app, Duration::from_secs(20), |state| {
        state.tabs().lock().expect("registry").get("tab-1").is_some_and(|t| {
            t.transcript().contains("Environment variable GREET not defined")
        })
    });
    shot("04-isolation-in-tx01");

    // The sample batch, typed by name at tx01's prompt: the real .TCB runner.
    key("type-line:SAMPLE.TCB");
    key("Enter");
    let sample_ran = wait_for(app, Duration::from_secs(20), |state| {
        state
            .tabs()
            .lock()
            .expect("registry")
            .get("tab-1")
            .is_some_and(|t| t.transcript().contains("RUNNING batch complete"))
    });
    shot("05-sample-tcb");

    // F8 runs the whole three-signal parity suite: the chrome opens the parity tab
    // (chrome verb) and the intent crosses the bus to that tab's own identity, which
    // alone may invoke run_parity.
    key("F8");
    let parity_started = wait_for(app, Duration::from_secs(30), |state| {
        state.suite().lock().expect("suite").started
    });
    let parity_done = wait_for(app, Duration::from_secs(1800), |state| {
        state.suite().lock().expect("suite").overall.is_some()
    });
    shot("06-parity-wall");

    // Denials, visibly: an unlisted verb and a tab verb from the chrome identity —
    // refused at the resolver and painted into tx01's transcript in deny red.
    key("Ctrl+1");
    eval_in(app, "console", "window.smokeDenials()");
    let denials_visible = wait_for(app, Duration::from_secs(10), |state| {
        state.denials().lock().expect("denials").len() >= 2
    });
    shot("07-denials-visible");

    // Keyboard chrome, for the record: the master menu and the F1 key map.
    key("Ctrl+Esc");
    shot("08-master-menu");
    key("Escape");
    key("F1");
    shot("09-keyboard-map");
    key("Escape");

    let state = app.state::<ConsoleState>();
    let suite = state.suite().lock().expect("suite").clone();
    let tabs = state.tabs().lock().expect("registry").infos();
    let denials = state.denials().lock().expect("denials").clone();
    let parity_pass = matches!(
        suite.overall,
        Some(stage_e_console::harness::Verdict::Pass)
    );
    let all_good = boot_ran
        && second_dos
        && set_ran
        && isolated
        && sample_ran
        && parity_started
        && parity_done
        && parity_pass
        && denials_visible;

    let evidence = serde_json::json!({
        "smoke": if all_good { "pass" } else { "fail" },
        "tab_count": tabs.len(),
        "tabs": tabs,
        "checks": {
            "boot_prerun_and_taskkill_denial_in_tx01": boot_ran,
            "second_dos_via_ctrl_t": second_dos,
            "set_ran_in_new_tab": set_ran,
            "isolation_visible_in_tx01": isolated,
            "sample_tcb_ran_in_tx01": sample_ran,
            "parity_suite_started_via_f8": parity_started,
            "parity_suite_finished": parity_done,
            "parity_overall_pass": parity_pass,
            "denials_visible": denials_visible,
        },
        "parity": {
            "entries": suite.entries,
            "fixture_signal": suite.fixture_signal,
            "transcript_signal": suite.transcript_signal,
            "spoor_signal": suite.spoor_signal,
            "overall": suite.overall,
        },
        "denials": denials,
        "reserved_line": state.reserved_text(),
        "screenshots": shots,
        "ui_driven": "keyboard (window.smokeKey, STORY-UX-04)",
    });
    (evidence, if all_good { 0 } else { 1 })
}

fn main() {
    // Verify before anything runs; refuse to start otherwise. A console with an
    // unverifiable verb enumeration has no authority to offer.
    let manifest = SignedManifest::from_json(MANIFEST_JSON)
        .expect("embedded manifest must parse")
        .verify(PUBKEY_HEX)
        .unwrap_or_else(|error| {
            eprintln!("stage-e-console: manifest verification failed, refusing to start: {error}");
            std::process::exit(1);
        });

    let authority = ConsoleAuthority::new(manifest.clone());
    let state = ConsoleState::new(
        os_dir(),
        std::env::temp_dir().join("stage-e-console-captures"),
        authority.denial_log(),
        manifest.verbs().map(String::from).collect(),
        manifest.tab_verbs().map(String::from).collect(),
    );

    let smoke = std::env::var("STAGE_E_SMOKE").is_ok_and(|v| v == "1");

    tauri::Builder::default()
        .invoke_handler(commands::handler())
        .manage(state)
        .setup(move |app| {
            app.set_authority_resolver(authority);
            let window = tauri::window::WindowBuilder::new(app, "host")
                .title("Ti-OS \u{2014} operator console (single window, host-side V1)")
                .inner_size(WIN_W, WIN_H)
                .resizable(false)
                .build()?;
            // Smoke forensics: an unattended run that ends early must say who ended
            // it — a CloseRequested is an outside actor, a Destroyed with none is not.
            window.on_window_event(|event| match event {
                tauri::WindowEvent::CloseRequested { .. } => {
                    eprintln!("stage-e-console: host window close requested (outside actor)");
                }
                tauri::WindowEvent::Destroyed => {
                    eprintln!("stage-e-console: host window destroyed");
                }
                _ => {}
            });
            window.add_child(
                tauri::webview::WebviewBuilder::new(
                    "console",
                    WebviewUrl::App("index.html".into()),
                ),
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(WIN_W, WIN_H),
            )?;
            // Added after the chrome so it composes above it: the system line is the
            // last line of the window, under the prompt, unlabelled (SPEC §10.1).
            window.add_child(
                tauri::webview::WebviewBuilder::new(
                    "reserved",
                    WebviewUrl::App("reserved.html".into()),
                ),
                LogicalPosition::new(RESERVED_X, RESERVED_Y),
                LogicalSize::new(RESERVED_W, RESERVED_H),
            )?;

            // The reconciler: registry state → window composition, every 200 ms, always
            // on the main thread. The only writer the reserved region ever has.
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                let tick = handle.clone();
                let _ = handle.run_on_main_thread(move || reconcile(&tick));
                std::thread::sleep(Duration::from_millis(200));
            });

            if smoke {
                let out = std::env::var("STAGE_E_SMOKE_OUT")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| std::env::temp_dir().join("stage-e-smoke.json"));
                let shots_dir = std::env::var("STAGE_E_SMOKE_SHOTS")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| std::env::temp_dir().join("stage-e-smoke-shots"));
                std::fs::create_dir_all(&shots_dir).ok();
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let window = handle.get_window("host").expect("host window exists");
                    let (evidence, code) = smoke_sequence(&handle, &window, &shots_dir);
                    let _ = std::fs::write(&out, serde_json::to_vec_pretty(&evidence).unwrap());
                    handle.exit(code);
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("console app must run");
}
