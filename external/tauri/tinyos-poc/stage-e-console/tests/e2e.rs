//! Stage E end-to-end, headless: the real console commands, the real signed manifest
//! artifacts (the ones the windowed app embeds), the real `ConsoleAuthority`, driven
//! through the fork's IPC path on `MockRuntime` — everything but the window.
//!
//! Written red-first against a `commands` module and manifest artifacts that did not yet
//! exist; see the Stage E report section.

use std::time::Duration;

use stage_a::{ipc_response_with_timeout, local_request};
use tauri::ipc::InvokeBody;
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{App, Manager, WebviewWindowBuilder};

use stage_e_console::commands::{self, ConsoleState, StreamSnapshot};
use stage_e_console::manifest::SignedManifest;

/// The at-rest artifacts, exactly as the app embeds them.
const MANIFEST_JSON: &str = include_str!("../manifest/console-manifest.json");
const PUBKEY_HEX: &str = include_str!("../manifest/console-pubkey.hex");

/// Boot the console core headless: real manifest, real resolver, real commands.
fn console_app(os_dir: &str) -> App<MockRuntime> {
    let manifest = SignedManifest::from_json(MANIFEST_JSON)
        .expect("committed manifest must parse")
        .verify(PUBKEY_HEX)
        .expect("committed manifest must verify against the committed public key");

    let authority = stage_e_console::authority::ConsoleAuthority::new(manifest.clone());
    let state = ConsoleState::new(
        os_dir.into(),
        std::env::temp_dir().join("stage-e-e2e-captures"),
        authority.denial_log(),
        manifest.verbs().map(String::from).collect(),
        manifest.tab_verbs().map(String::from).collect(),
    );

    let app = mock_builder()
        .invoke_handler(commands::handler())
        .manage(state)
        .build(mock_context(noop_assets()))
        .expect("console core must boot on MockRuntime");
    app.set_authority_resolver(authority);
    app
}

fn read_stream_snapshot(webview: &tauri::WebviewWindow<MockRuntime>) -> StreamSnapshot {
    let value =
        ipc_response_with_timeout(webview, local_request("read_stream", InvokeBody::default()))
            .expect("read_stream must be answered")
            .expect("read_stream is enumerated and must succeed");
    serde_json::from_value(value).expect("snapshot must deserialize")
}

/// E1 — enumerated verbs resolve end to end; an unlisted verb is denied at the authority
/// seam and the denial then *appears in the stream snapshot* — the record the UI renders.
#[test]
fn e1_manifest_governs_the_ipc_surface() {
    let app = console_app("C:/nonexistent-os-dir");
    let webview = WebviewWindowBuilder::new(&app, "console", Default::default()).build().unwrap();

    // Enumerated: read_stream answers.
    let snapshot = read_stream_snapshot(&webview);
    assert!(snapshot.denials.is_empty());
    assert!(snapshot.manifest_verbs.contains(&"launch_fixture".to_string()));

    // Unlisted verb — a command that does not even exist: denied, not "not found".
    let refused = ipc_response_with_timeout(
        &webview,
        local_request("format_disk", InvokeBody::default()),
    )
    .expect("unlisted verb must be answered with a rejection");
    assert!(refused.is_err(), "deny-by-default: unlisted verb must be refused");

    // The denial is now visible through the stream the UI polls.
    let snapshot = read_stream_snapshot(&webview);
    assert_eq!(snapshot.denials.len(), 1);
    assert_eq!(snapshot.denials[0].command, "format_disk");
    assert_eq!(snapshot.denials[0].webview, "console");
}

/// E2 — a webview whose label is not the manifest's console identity is denied the same
/// enumerated verb (identity travels with the runtime label, not the command name).
#[test]
fn e2_wrong_identity_is_denied_enumerated_verbs() {
    let app = console_app("C:/nonexistent-os-dir");
    let imposter =
        WebviewWindowBuilder::new(&app, "imposter", Default::default()).build().unwrap();

    let refused =
        ipc_response_with_timeout(&imposter, local_request("read_stream", InvokeBody::default()))
            .expect("must be answered with a rejection");
    assert!(refused.is_err(), "an unlisted identity must not use the console's verbs");
}

/// E3 — `send_line` is enumerated, resolves, and then answers the honest transport error:
/// the target has no serial RX path. The refusal is a *transport* fact, not an authority
/// denial — the denial log stays empty.
#[test]
fn e3_send_line_reports_the_one_way_transport() {
    let app = console_app("C:/nonexistent-os-dir");
    let webview = WebviewWindowBuilder::new(&app, "console", Default::default()).build().unwrap();

    let answer = ipc_response_with_timeout(
        &webview,
        local_request("send_line", InvokeBody::Json(serde_json::json!({"line": "DIR"}))),
    )
    .expect("send_line must be answered");
    let error = answer.expect_err("send_line must fail while the transport is one-way");
    assert!(error.to_string().contains("TX-only"));

    let snapshot = read_stream_snapshot(&webview);
    assert!(snapshot.denials.is_empty(), "a transport error is not an authority denial");
}

/// E4 — `launch_fixture` against a directory that is not a TinyOS tree fails as an error
/// result, never a hang and never a false PASS.
#[test]
fn e4_launch_fixture_fails_closed_on_a_bad_tree() {
    let app = console_app("C:/nonexistent-os-dir");
    let webview = WebviewWindowBuilder::new(&app, "console", Default::default()).build().unwrap();

    let answer = ipc_response_with_timeout(
        &webview,
        local_request("launch_fixture", InvokeBody::Json(serde_json::json!({"fixture": "measure"}))),
    )
    .expect("launch_fixture must be answered");

    match answer {
        // Spawn refused outright (cwd does not exist): the honest immediate error.
        Err(_) => {}
        // Or the spawn succeeded and the harness reports the failure as a verdict.
        Ok(_) => {
            let deadline = std::time::Instant::now() + Duration::from_secs(60);
            loop {
                let snapshot = read_stream_snapshot(&webview);
                if let Some(verdict) = snapshot.verdict {
                    assert_ne!(
                        serde_json::to_value(&verdict).unwrap()["kind"],
                        "Pass",
                        "a bad tree must never produce a PASS"
                    );
                    break;
                }
                assert!(std::time::Instant::now() < deadline, "no verdict for the bad tree");
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
}

/// E6 — the 17G tab flow through the real IPC path: the chrome opens two DOS tabs, each
/// tab identity runs lines in its own session, and the isolation is *visible* — tab 2
/// answers "not defined" for tab 1's variable, through `read_tab`, on MockRuntime.
#[test]
fn e6_tabs_open_and_sessions_isolate_through_ipc() {
    let app = console_app("C:/nonexistent-os-dir");
    let console = WebviewWindowBuilder::new(&app, "console", Default::default()).build().unwrap();

    for _ in 0..2 {
        ipc_response_with_timeout(
            &console,
            local_request("open_tab", InvokeBody::Json(serde_json::json!({"kind": "dos"}))),
        )
        .expect("open_tab must be answered")
        .expect("the chrome may open tabs");
    }

    let tab1 = WebviewWindowBuilder::new(&app, "tab-1", Default::default()).build().unwrap();
    let tab2 = WebviewWindowBuilder::new(&app, "tab-2", Default::default()).build().unwrap();

    for line in ["SET GREET=HELLO-17G", "SET GREET"] {
        ipc_response_with_timeout(
            &tab1,
            local_request("run_line", InvokeBody::Json(serde_json::json!({"line": line}))),
        )
        .expect("run_line must be answered")
        .expect("an enumerated tab identity may run lines");
    }
    ipc_response_with_timeout(
        &tab2,
        local_request("run_line", InvokeBody::Json(serde_json::json!({"line": "SET GREET"}))),
    )
    .expect("run_line must be answered")
    .expect("tab-2 may run lines too");

    let snap = |webview: &tauri::WebviewWindow<MockRuntime>| {
        ipc_response_with_timeout(webview, local_request("read_tab", InvokeBody::default()))
            .expect("read_tab must be answered")
            .expect("read_tab is a tab verb")
    };
    let one = snap(&tab1);
    let two = snap(&tab2);
    assert!(one["transcript"].as_str().unwrap().contains("GREET=HELLO-17G"));
    assert!(
        two["transcript"].as_str().unwrap().contains("Environment variable GREET not defined"),
        "tab 2 must not see tab 1's environment"
    );
    assert_eq!(one["info"]["session"], "TAB-1");
    assert_eq!(two["info"]["session"], "TAB-2");
}

/// E7 — the grant tables are disjoint through the real resolver: a tab cannot open
/// tabs, the chrome cannot run lines, and both refusals surface in the snapshot the
/// tab bar polls.
#[test]
fn e7_tab_and_chrome_grants_are_disjoint_through_ipc() {
    let app = console_app("C:/nonexistent-os-dir");
    let console = WebviewWindowBuilder::new(&app, "console", Default::default()).build().unwrap();
    ipc_response_with_timeout(
        &console,
        local_request("open_tab", InvokeBody::Json(serde_json::json!({"kind": "dos"}))),
    )
    .expect("answered")
    .expect("chrome opens tab-1");
    let tab1 = WebviewWindowBuilder::new(&app, "tab-1", Default::default()).build().unwrap();

    let refused = ipc_response_with_timeout(
        &tab1,
        local_request("open_tab", InvokeBody::Json(serde_json::json!({"kind": "dos"}))),
    )
    .expect("answered");
    assert!(refused.is_err(), "a tab identity must not open tabs");
    let refused = ipc_response_with_timeout(
        &console,
        local_request("run_line", InvokeBody::Json(serde_json::json!({"line": "DIR"}))),
    )
    .expect("answered");
    assert!(refused.is_err(), "the chrome identity must not run lines");

    let snapshot = ipc_response_with_timeout(
        &console,
        local_request("read_console", InvokeBody::default()),
    )
    .expect("answered")
    .expect("read_console is a chrome verb");
    let denials = snapshot["denials"].as_array().unwrap();
    assert_eq!(denials.len(), 2, "both refusals are visible: {denials:?}");
    assert_eq!(denials[0]["command"], "open_tab");
    assert_eq!(denials[0]["webview"], "tab-1");
    assert_eq!(denials[1]["command"], "run_line");
    assert_eq!(denials[1]["webview"], "console");
}

/// E8 — the reserved region's identity is enumerated nowhere: every verb it might try
/// is refused at the authority seam, so "repainted only by the Rust side" is a property
/// of the grant table, not of page discipline.
#[test]
fn e8_reserved_region_cannot_invoke() {
    let app = console_app("C:/nonexistent-os-dir");
    let reserved =
        WebviewWindowBuilder::new(&app, "reserved", Default::default()).build().unwrap();
    for verb in ["read_tab", "run_line", "open_tab", "read_console", "launch_fixture"] {
        let refused =
            ipc_response_with_timeout(&reserved, local_request(verb, InvokeBody::default()))
                .expect("answered");
        assert!(refused.is_err(), "reserved must not resolve {verb}");
    }
}

/// E5 (live, `--ignored`) — the full Stage E path headless: launch the real `measure`
/// fixture in the tree named by `TINYOS_OS_DIR`, poll `read_stream` through IPC until the
/// verdict lands, and assert the serial envelope arrived and the verdict is PASS — the
/// same verdict `xtask` computes.
#[test]
#[ignore = "needs TINYOS_OS_DIR, the x86_64 toolchain and QEMU; run explicitly for the Stage E evidence"]
fn e5_live_fixture_through_the_ipc_path() {
    let os_dir = std::env::var("TINYOS_OS_DIR").expect("set TINYOS_OS_DIR");
    let app = console_app(&os_dir);
    let webview = WebviewWindowBuilder::new(&app, "console", Default::default()).build().unwrap();

    let launched = ipc_response_with_timeout(
        &webview,
        local_request("launch_fixture", InvokeBody::Json(serde_json::json!({"fixture": "measure"}))),
    )
    .expect("launch_fixture must be answered");
    launched.expect("launch against the real tree must start");

    let deadline = std::time::Instant::now() + Duration::from_secs(300);
    let final_snapshot = loop {
        let snapshot = read_stream_snapshot(&webview);
        if snapshot.verdict.is_some() {
            break snapshot;
        }
        assert!(std::time::Instant::now() < deadline, "no verdict within the time budget");
        std::thread::sleep(Duration::from_millis(500));
    };

    assert_eq!(serde_json::to_value(final_snapshot.verdict.unwrap()).unwrap()["kind"], "Pass");
    assert!(
        final_snapshot.lines.iter().any(|l| l.contains("TINYOS-MEAS/2")),
        "live serial must contain the measurement envelope"
    );
}
