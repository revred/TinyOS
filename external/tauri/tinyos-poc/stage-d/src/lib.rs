//! 08C Stage D — revocation on navigation (`PD-13`, review §2.3).
//!
//! The edge the review named: a handler already executing when the webview navigates to a
//! remote origin retains its authority for the rest of its run, and a `Channel` opened before
//! navigation keeps streaming after it. The fork's obligation: **cancel and close, not drain.**
//!
//! Kill criterion watched: cancellation requiring a restructuring of dispatch rather than a
//! token per invoke. The implementation under test is exactly a token per invoke — a per-webview
//! origin generation captured when work is authorised and checked when its results try to cross
//! back — so if these tests pass without dispatch changes, the criterion is not tripped.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use stage_a::{ipc_response_with_timeout, local_request};
use tauri::ipc::{Channel, InvokeBody, InvokeResponseBody};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{App, Manager, WebviewWindowBuilder};

/// Latest channel handed to the `stream` command, retrievable by the test.
#[derive(Default, Clone)]
pub struct StreamState(pub Arc<Mutex<Option<Channel<InvokeResponseBody>>>>);

#[tauri::command]
async fn slow() -> &'static str {
    tokio::time::sleep(Duration::from_millis(300)).await;
    "done"
}

#[tauri::command]
fn stream(state: tauri::State<'_, StreamState>, on_event: Channel<InvokeResponseBody>) {
    state.0.lock().unwrap().replace(on_event);
}

pub fn poc_app() -> App<MockRuntime> {
    mock_builder()
        .invoke_handler(tauri::generate_handler![slow, stream])
        .manage(StreamState::default())
        .build(mock_context(noop_assets()))
        .expect("core must boot on MockRuntime")
}

pub const REMOTE: &str = "https://evil.example.com/";
pub const OTHER_LOCAL: &str = "http://tauri.localhost/other-page";

#[cfg(test)]
mod tests {
    use super::*;

    /// D1 — the slow handler: invoked from a local origin, origin flipped to remote while it
    /// runs. Its result must be **cancelled**, never delivered across the revocation.
    #[test]
    fn d1_in_flight_result_is_cancelled_on_remote_navigation() {
        let app = poc_app();
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        webview.as_ref().clone().on_message(
            local_request("slow", InvokeBody::default()),
            Box::new(move |_wv, _cmd, response, _cb, _err| {
                let _ = tx.send(response);
            }),
        );

        // The handler is now sleeping. Flip the origin out from under it.
        webview.navigate(REMOTE.parse().unwrap()).unwrap();

        match rx.recv_timeout(Duration::from_secs(2)) {
            Err(_) => {} // nothing crossed back: cancelled, the required outcome
            Ok(response) => panic!(
                "a result computed under revoked authority was delivered: {response:?}"
            ),
        }
    }

    /// D2 — the open channel: opened from a local origin, origin flipped to remote. The
    /// channel must **close** — subsequent sends fail — rather than drain to the remote page.
    #[test]
    fn d2_open_channel_closes_on_remote_navigation() {
        let app = poc_app();
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        // Open the channel through real IPC: the argument arrives as a JS channel id string.
        let open = ipc_response_with_timeout(
            &webview,
            local_request(
                "stream",
                InvokeBody::Json(serde_json::json!({"onEvent": "__CHANNEL__:7"})),
            ),
        )
        .expect("stream command must be answered");
        assert!(open.is_ok(), "stream must accept the channel: {open:?}");

        let channel = app
            .state::<StreamState>()
            .0
            .lock()
            .unwrap()
            .clone()
            .expect("command must have stored the channel");

        // While local: the channel streams.
        channel
            .send(InvokeResponseBody::Json("1".into()))
            .expect("send before navigation must succeed");

        webview.navigate(REMOTE.parse().unwrap()).unwrap();

        // After the flip: closed, not drained.
        let result = channel.send(InvokeResponseBody::Json("2".into()));
        assert!(
            result.is_err(),
            "a channel opened under local authority must close when the origin turns remote"
        );
    }

    /// D3 — the control: a local→local navigation revokes nothing. The slow handler's result
    /// arrives and the channel keeps streaming; revocation keys on the origin transition, not
    /// on navigation as such.
    #[test]
    fn d3_local_navigation_revokes_nothing() {
        let app = poc_app();
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        webview.as_ref().clone().on_message(
            local_request("slow", InvokeBody::default()),
            Box::new(move |_wv, _cmd, response, _cb, _err| {
                let _ = tx.send(response);
            }),
        );

        webview.navigate(OTHER_LOCAL.parse().unwrap()).unwrap();

        rx.recv_timeout(Duration::from_secs(2))
            .expect("a local navigation must not cancel in-flight work");

        let open = ipc_response_with_timeout(
            &webview,
            local_request(
                "stream",
                InvokeBody::Json(serde_json::json!({"onEvent": "__CHANNEL__:9"})),
            ),
        )
        .expect("stream command must be answered");
        assert!(open.is_ok());
        let channel = app.state::<StreamState>().0.lock().unwrap().clone().unwrap();
        webview.navigate("http://tauri.localhost/third".parse().unwrap()).unwrap();
        channel
            .send(InvokeResponseBody::Json("still-local".into()))
            .expect("a local navigation must not close open channels");
    }
}
