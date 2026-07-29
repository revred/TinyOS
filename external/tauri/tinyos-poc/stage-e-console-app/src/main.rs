//! 08C Stage E / 13F Deliverable A — the windowed operator console.
//!
//! Composition, and nothing else: the signed manifest (embedded at rest, verified before
//! anything runs), the `ConsoleAuthority` resolver installed over the fork's Stage C seam,
//! the four verbs from `stage-e-console::commands`, and a WebView2 window whose page is
//! the console pane. Fail-closed: if the embedded manifest does not verify, the process
//! exits before a window ever exists.
//!
//! `STAGE_E_SMOKE=1` runs the acceptance sequence unattended: the page (not the Rust
//! side) launches the `measure` fixture, waits for the verdict, then invokes an unlisted
//! verb; a Rust observer waits for verdict + recorded denial in shared state, writes the
//! evidence JSON to `STAGE_E_SMOKE_OUT` (or the temp dir), and exits.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::time::Duration;

use stage_e_console::authority::ConsoleAuthority;
use stage_e_console::commands::{self, ConsoleState};
use stage_e_console::manifest::SignedManifest;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

/// The at-rest manifest and the key the console trusts, fixed at build time.
const MANIFEST_JSON: &str = include_str!("../../stage-e-console/manifest/console-manifest.json");
const PUBKEY_HEX: &str = include_str!("../../stage-e-console/manifest/console-pubkey.hex");

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
    let denial_log = authority.denial_log();
    let state = ConsoleState::new(
        os_dir(),
        std::env::temp_dir().join("stage-e-console-captures"),
        authority.denial_log(),
        manifest.verbs().map(String::from).collect(),
    );

    let smoke = std::env::var("STAGE_E_SMOKE").is_ok_and(|v| v == "1");

    tauri::Builder::default()
        .invoke_handler(commands::handler())
        .manage(state)
        .setup(move |app| {
            app.set_authority_resolver(authority);
            let window = WebviewWindowBuilder::new(
                app,
                "console", // must match the manifest's console identity
                WebviewUrl::App("index.html".into()),
            )
            .title("TinyOS operator console — Stage E")
            .inner_size(1100.0, 720.0)
            .build()?;

            if smoke {
                let out = std::env::var("STAGE_E_SMOKE_OUT")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| std::env::temp_dir().join("stage-e-smoke.json"));
                let handle = app.handle().clone();
                let denials = denial_log.clone();
                std::thread::spawn(move || {
                    // Let the page boot, then hand it the wheel.
                    std::thread::sleep(Duration::from_secs(3));
                    let _ = window.eval("window.smokeRun()");

                    let deadline = std::time::Instant::now() + Duration::from_secs(300);
                    let verdict = loop {
                        {
                            let state = handle.state::<ConsoleState>();
                            let snapshot = commands::read_stream(state);
                            let denied =
                                denials.lock().expect("denial lock").iter().any(|d| d.command == "format_disk");
                            if snapshot.verdict.is_some() && denied {
                                break Some(snapshot);
                            }
                        }
                        if std::time::Instant::now() > deadline {
                            break None;
                        }
                        std::thread::sleep(Duration::from_millis(500));
                    };

                    let (code, evidence) = match verdict {
                        Some(snapshot) => (
                            0,
                            serde_json::json!({
                                "smoke": "pass",
                                "fixture": snapshot.fixture,
                                "verdict": snapshot.verdict,
                                "serial_lines": snapshot.lines.len(),
                                "first_line": snapshot.lines.first(),
                                "envelope_seen": snapshot.lines.iter().any(|l| l.contains("TINYOS-MEAS/2")),
                                "denials": snapshot.denials,
                                "manifest_verbs": snapshot.manifest_verbs,
                                "ui_driven": true,
                            }),
                        ),
                        None => (1, serde_json::json!({"smoke": "fail: timeout waiting for verdict + denial"})),
                    };
                    let _ = std::fs::write(&out, serde_json::to_vec_pretty(&evidence).unwrap());
                    handle.exit(code);
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("console app must run");
}
