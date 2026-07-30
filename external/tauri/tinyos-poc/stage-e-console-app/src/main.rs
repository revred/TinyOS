//! 17G — the windowed multi-tab operator console.
//!
//! Composition, and nothing else: the signed manifest (embedded at rest, verified before
//! anything runs), the `ConsoleAuthority` resolver installed over the fork's Stage C seam,
//! the verbs from `stage-e-console::commands`, and **one window, sibling webviews** — the
//! `a3` shape made visible:
//!
//! - `reserved` — the host-owned reserved region (top strip). Its label is enumerated
//!   nowhere in the manifest, so it holds no verbs (e8/R6); it is repainted exclusively
//!   by [`reconcile`] on the Rust side, and no tab content can reach it.
//! - `console` — the tab-bar chrome. Holds the chrome verbs only.
//! - `tab-1`…`tab-6` — tab content webviews, siblings under the same window, created by
//!   the reconciler from registry state. Each holds the tab verbs under its own label.
//!
//! `STAGE_E_SMOKE=1` runs the 17G acceptance sequence unattended — tabs opened, `DIR`
//! run, isolation shown, `SAMPLE.TCB` run, the parity suite run from its tab — writing
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

/// Window geometry — fixed for the PoC (the window is non-resizable; a resize-following
/// layout is presentation work the demo does not need).
const WIN_W: f64 = 1280.0;
const WIN_H: f64 = 800.0;
const RESERVED_H: f64 = 44.0;
const CHROME_H: f64 = 96.0;

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

/// One reconciler tick, on the main thread: create missing tab webviews as siblings,
/// show the focused tab and hide the rest, and repaint the host-owned reserved region.
/// The only writer the reserved webview ever has is the `eval` below.
fn reconcile(app: &tauri::AppHandle) {
    let state = app.state::<ConsoleState>();
    let (infos, focused) = {
        let tabs = state.tabs().lock().expect("tab registry");
        (tabs.infos(), tabs.focused().map(|t| t.label().to_string()))
    };
    if let Some(window) = app.get_window("host") {
        for info in &infos {
            if app.get_webview(&info.label).is_none() {
                let _ = window.add_child(
                    tauri::webview::WebviewBuilder::new(
                        info.label.clone(),
                        WebviewUrl::App("tab.html".into()),
                    ),
                    LogicalPosition::new(0.0, RESERVED_H + CHROME_H),
                    LogicalSize::new(WIN_W, WIN_H - RESERVED_H - CHROME_H),
                );
            }
            if let Some(webview) = app.get_webview(&info.label) {
                let _ = if focused.as_deref() == Some(info.label.as_str()) {
                    webview.show()
                } else {
                    webview.hide()
                };
            }
        }
    }
    if let Some(reserved) = app.get_webview("reserved") {
        let text = serde_json::to_string(&state.reserved_text()).expect("a string serializes");
        let _ = reserved.eval(format!("document.getElementById('t').textContent = {text}"));
    }
}

/// Capture the window's on-screen rectangle to `dir/<name>.png` via the host screenshot
/// API (GDI `CopyFromScreen` — 17G acceptance 1 allows exactly this). Returns the path
/// on success; a failed capture is reported in the smoke JSON, never a panic.
fn screenshot(window: &tauri::Window<tauri::Wry>, dir: &Path, name: &str) -> Option<String> {
    let position = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    let path = dir.join(format!("{name}.png"));
    let script = format!(
        "Add-Type -AssemblyName System.Drawing; \
         $b = New-Object System.Drawing.Bitmap({w}, {h}); \
         $g = [System.Drawing.Graphics]::FromImage($b); \
         $g.CopyFromScreen({x}, {y}, 0, 0, $b.Size); \
         $b.Save('{path}', [System.Drawing.Imaging.ImageFormat]::Png)",
        w = size.width,
        h = size.height,
        x = position.x,
        y = position.y,
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

/// The unattended 17G acceptance sequence. Every step is driven through a page (`eval`
/// of a smoke hook), so each action travels the real UI path from the correct identity.
/// Returns the evidence JSON and the process exit code.
fn smoke_sequence(
    app: &tauri::AppHandle,
    window: &tauri::Window<tauri::Wry>,
    shots_dir: &Path,
) -> (serde_json::Value, i32) {
    let mut shots: Vec<Option<String>> = Vec::new();
    let mut shot = |name: &str| {
        // Give the reconciler and the pages two ticks to paint before capturing.
        std::thread::sleep(Duration::from_millis(1200));
        shots.push(screenshot(window, shots_dir, name).map(|p| format!("{name}.png: {p}")));
    };

    std::thread::sleep(Duration::from_secs(3)); // let the pages boot
    let _ = window.set_focus();
    shot("01-boot");

    // Tabs: two DOS sessions and the parity tab, opened by the chrome. The sequence is
    // label-independent from here: it reads the registry for the first two DOS tabs and
    // the parity tab, so a stray extra tab (this runs on a live desktop; an errant
    // click on "+ DOS tab" is possible) degrades nothing — the mandate says ≥3 tabs.
    eval_in(app, "console", "window.smokeOpenTabs()");
    let three_tabs = wait_for(app, Duration::from_secs(20), |state| {
        let infos = state.tabs().lock().expect("registry").infos();
        infos.iter().filter(|i| i.kind == TabKind::Dos).count() >= 2
            && infos.iter().any(|i| i.kind == TabKind::Parity)
    });
    shot("02-three-tabs");

    let (dos_a, dos_b, parity_tab) = {
        let state = app.state::<ConsoleState>();
        let infos = state.tabs().lock().expect("registry").infos();
        let mut dos = infos.iter().filter(|i| i.kind == TabKind::Dos).map(|i| i.label.clone());
        (
            dos.next().unwrap_or_else(|| "tab-1".into()),
            dos.next().unwrap_or_else(|| "tab-2".into()),
            infos
                .iter()
                .find(|i| i.kind == TabKind::Parity)
                .map(|i| i.label.clone())
                .unwrap_or_else(|| "tab-3".into()),
        )
    };

    // A live DIR in the first DOS tab.
    eval_in(app, "console", &format!("window.smokeFocus('{dos_a}')"));
    eval_in(app, &dos_a, "window.smokeRunLine('DIR')");
    let dir_ran = wait_for(app, Duration::from_secs(20), |state| {
        state
            .tabs()
            .lock()
            .expect("registry")
            .get(&dos_a)
            .is_some_and(|t| t.transcript().contains("Directory of A:\\"))
    });
    shot("03-dir-in-first-dos-tab");

    // Isolation, visibly: SET in the first DOS tab, the same variable undefined in the
    // second.
    eval_in(app, &dos_a, "window.smokeRunLine('SET GREET=HELLO-17G')");
    eval_in(app, &dos_a, "window.smokeRunLine('SET GREET')");
    std::thread::sleep(Duration::from_millis(700));
    eval_in(app, &dos_b, "window.smokeRunLine('SET GREET')");
    let isolated = wait_for(app, Duration::from_secs(20), |state| {
        let tabs = state.tabs().lock().expect("registry");
        tabs.get(&dos_a).is_some_and(|t| t.transcript().contains("GREET=HELLO-17G"))
            && tabs.get(&dos_b).is_some_and(|t| {
                t.transcript().contains("Environment variable GREET not defined")
            })
    });
    eval_in(app, "console", &format!("window.smokeFocus('{dos_b}')"));
    shot("04-isolation-second-dos-tab");

    // The sample batch: SAMPLE.TCB typed at the second DOS tab's prompt runs the real
    // .TCB runner.
    eval_in(app, &dos_b, "window.smokeRunLine('SAMPLE.TCB')");
    let sample_ran = wait_for(app, Duration::from_secs(20), |state| {
        state
            .tabs()
            .lock()
            .expect("registry")
            .get(&dos_b)
            .is_some_and(|t| t.transcript().contains("RUNNING batch complete"))
    });
    shot("05-sample-tcb");

    // The whole parity suite, from the parity tab's own identity.
    eval_in(app, "console", &format!("window.smokeFocus('{parity_tab}')"));
    eval_in(app, &parity_tab, "window.smokeRunParity()");
    let parity_done = wait_for(app, Duration::from_secs(1800), |state| {
        state.suite().lock().expect("suite").overall.is_some()
    });
    shot("06-parity-wall");

    // Denials, visibly: an unlisted verb and a tab verb from the chrome identity.
    eval_in(app, "console", "window.smokeDenials()");
    let denials_visible = wait_for(app, Duration::from_secs(10), |state| {
        state.denials().lock().expect("denials").len() >= 2
    });
    eval_in(app, "console", &format!("window.smokeFocus('{dos_a}')"));
    shot("07-denials-visible");

    let state = app.state::<ConsoleState>();
    let suite = state.suite().lock().expect("suite").clone();
    let tabs = state.tabs().lock().expect("registry").infos();
    let denials = state.denials().lock().expect("denials").clone();
    let parity_pass = matches!(
        suite.overall,
        Some(stage_e_console::harness::Verdict::Pass)
    );
    let all_good = three_tabs
        && dir_ran
        && isolated
        && sample_ran
        && parity_done
        && parity_pass
        && denials_visible;

    let evidence = serde_json::json!({
        "smoke": if all_good { "pass" } else { "fail" },
        "tab_count": tabs.len(),
        "tabs": tabs,
        "roles": { "dos_a": dos_a, "dos_b": dos_b, "parity": parity_tab },
        "checks": {
            "three_tabs_open": three_tabs,
            "dir_ran_in_tab_1": dir_ran,
            "isolation_visible": isolated,
            "sample_tcb_ran_in_tab_2": sample_ran,
            "parity_suite_finished": parity_done,
            "parity_overall_pass": parity_pass,
            "denials_visible": denials_visible,
        },
        "parity": {
            "entries": suite.entries,
            "fixture_signal": suite.fixture_signal,
            "transcript_signal": suite.transcript_signal,
            "overall": suite.overall,
        },
        "denials": denials,
        "reserved_line": state.reserved_text(),
        "screenshots": shots,
        "ui_driven": true,
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
                .title("TinyOS \u{2014} single window, multi-tab OS UX (host-side, 17G)")
                .inner_size(WIN_W, WIN_H)
                .resizable(false)
                .build()?;
            window.add_child(
                tauri::webview::WebviewBuilder::new(
                    "reserved",
                    WebviewUrl::App("reserved.html".into()),
                ),
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(WIN_W, RESERVED_H),
            )?;
            window.add_child(
                tauri::webview::WebviewBuilder::new(
                    "console",
                    WebviewUrl::App("index.html".into()),
                ),
                LogicalPosition::new(0.0, RESERVED_H),
                LogicalSize::new(WIN_W, CHROME_H),
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
                    let _ = window.set_always_on_top(true);
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
