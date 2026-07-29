//! 08C Stage A — the headless `Runtime` seam, driven through upstream's own `MockRuntime`.
//!
//! Proves review §7.3.1: the `tauri-runtime` trait seam is real and holds with zero patches to
//! `tao`/`wry` — neither crate is in this workspace's dependency graph at all (asserted by the
//! `cargo tree` check recorded in the Stage A report section, not compilable code here).
//!
//! Kill criterion watched throughout: a trait method that leaks `tao`/`wry` types or platform
//! assumptions through the seam.

use std::time::Duration;

use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponse};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, Manager, Webview, WebviewWindowBuilder};

#[tauri::command]
fn ping() -> &'static str {
    "pong"
}

#[tauri::command]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[tauri::command]
fn refuse() -> Result<(), String> {
    Err("refused on purpose".into())
}

/// Boot the core headless with the PoC commands registered.
pub fn poc_app() -> App<MockRuntime> {
    mock_builder()
        .invoke_handler(tauri::generate_handler![ping, add, refuse])
        .build(mock_context(noop_assets()))
        .expect("core must boot on MockRuntime")
}

/// A local-origin invoke request for `cmd` with the correct invoke key.
pub fn local_request(cmd: &str, body: InvokeBody) -> InvokeRequest {
    InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "http://tauri.localhost".parse().unwrap(),
        body,
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}

/// Like `tauri::test::get_ipc_response`, but with a timeout instead of a hang, so a *dropped*
/// request (the invoke-key gate returns without responding) is observable as `None`.
pub fn ipc_response_with_timeout<W: AsRef<Webview<MockRuntime>>>(
    webview: &W,
    request: InvokeRequest,
) -> Option<Result<serde_json::Value, serde_json::Value>> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    webview.as_ref().clone().on_message(
        request,
        Box::new(move |_webview, _cmd, response, _callback, _error| {
            let _ = tx.send(response);
        }),
    );
    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(InvokeResponse::Ok(body)) => Some(Ok(body.deserialize().unwrap())),
        Ok(InvokeResponse::Err(e)) => Some(Err(e.0)),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A1 — boot core, register commands, drive IPC invokes end-to-end, typed both ways.
    #[test]
    fn a1_ipc_invoke_end_to_end() {
        let app = poc_app();
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("webview must build on MockRuntime");

        let res = ipc_response_with_timeout(&webview, local_request("ping", InvokeBody::default()))
            .expect("ping must be answered");
        assert_eq!(res.unwrap(), serde_json::json!("pong"));

        let res = ipc_response_with_timeout(
            &webview,
            local_request("add", InvokeBody::Json(serde_json::json!({"a": 19, "b": 23}))),
        )
        .expect("add must be answered");
        assert_eq!(res.unwrap(), serde_json::json!(42));

        let res =
            ipc_response_with_timeout(&webview, local_request("refuse", InvokeBody::default()))
                .expect("refuse must be answered");
        assert_eq!(res.unwrap_err(), serde_json::json!("refused on purpose"));
    }

    /// A2 — webview create and teardown are clean through the seam. Close is an event-loop
    /// message on the mock (exactly as on a real runtime), so teardown is asserted after the
    /// loop has drained and exited, through a surviving `AppHandle`.
    #[test]
    fn a2_webview_create_teardown() {
        let app = poc_app();

        let wv = WebviewWindowBuilder::new(&app, "transient", Default::default())
            .build()
            .unwrap();
        assert!(app.get_webview_window("transient").is_some());

        let handle = app.handle().clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            wv.close().expect("close must succeed");
        });

        // The loop processes CloseWindow, removes the window, and exits when none remain.
        let exit_code = app.run_return(|_app, _event| {});
        assert_eq!(exit_code, 0, "event loop must exit cleanly after last close");
        assert!(
            handle.get_webview_window("transient").is_none(),
            "closed webview must be gone from the manager"
        );
    }

    /// A3 — the `unstable` multiwebview gate: one window, sibling webviews with distinct labels.
    /// This is the shape 07A §6.2 corrected LE-53 with: a host-owned reserved region as a
    /// *separate webview* beside tab content.
    #[test]
    fn a3_multiwebview_reserved_region_shape() {
        let app = poc_app();

        let window = tauri::window::WindowBuilder::new(&app, "host")
            .build()
            .expect("plain window must build under the unstable feature");

        let reserved = window
            .add_child(
                tauri::webview::WebviewBuilder::new("reserved-region", Default::default()),
                tauri::LogicalPosition::new(0., 0.),
                tauri::LogicalSize::new(800., 40.),
            )
            .expect("host-owned reserved webview must attach");
        let tab = window
            .add_child(
                tauri::webview::WebviewBuilder::new("tab-1", Default::default()),
                tauri::LogicalPosition::new(0., 40.),
                tauri::LogicalSize::new(800., 560.),
            )
            .expect("sibling tab webview must attach");

        let labels: Vec<String> = window.webviews().iter().map(|w| w.label().into()).collect();
        assert!(labels.contains(&"reserved-region".to_string()));
        assert!(labels.contains(&"tab-1".to_string()));
        assert_eq!(labels.len(), 2, "exactly the two siblings, no implicit main");

        // Tab teardown must not take the reserved region with it.
        tab.close().unwrap();
        assert!(app.get_webview("tab-1").is_none());
        assert!(app.get_webview("reserved-region").is_some());
        drop(reserved);
    }

    /// A4 — the invoke-key gate drops a mismatched request without a response (review §2.5:
    /// a caller-supplied bearer secret; the request dies silently rather than erroring).
    #[test]
    fn a4_invoke_key_mismatch_is_dropped() {
        let app = poc_app();
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let mut req = local_request("ping", InvokeBody::default());
        req.invoke_key = "not-the-key".into();
        assert!(
            ipc_response_with_timeout(&webview, req).is_none(),
            "a wrong invoke key must yield no response at all"
        );
    }

    /// A5 — origin is honoured per call: the same command that answers a local origin refuses a
    /// remote one (review §2.3 / EPIC-H2 §2.4). With no ACL resolved, remote origins have no
    /// authority — upstream's own deny path.
    #[test]
    fn a5_remote_origin_is_refused_where_local_is_answered() {
        let app = poc_app();
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let ok = ipc_response_with_timeout(&webview, local_request("ping", InvokeBody::default()))
            .expect("local origin must be answered");
        assert!(ok.is_ok());

        let mut remote = local_request("ping", InvokeBody::default());
        remote.url = "https://evil.example.com/".parse().unwrap();
        let refused = ipc_response_with_timeout(&webview, remote)
            .expect("remote origin must be answered with a rejection, not ignored");
        assert!(
            refused.is_err(),
            "remote origin must not reach an app command without an explicit remote grant"
        );
    }
}
