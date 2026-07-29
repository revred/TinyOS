//! 08C Stage C — the resolver seam: `RuntimeAuthority`'s verdict externalised behind a trait,
//! implemented twice.
//!
//! 1. [`UpstreamWrap`] wraps an unmodified upstream `RuntimeAuthority` — proving no regression:
//!    the trait can express exactly what the `BTreeMap` lookup expressed.
//! 2. [`MockAci`] defers to an external deny-by-default table keyed on (webview label, command)
//!    — the ACI shape: the framework asks, the engine answers, and *nothing* is granted that
//!    the table does not name. Remote origins are refused unconditionally, table or no table.
//!
//! Kill criterion watched: authority decisions scattered beyond `resolve_access`. One instance
//! was found and is recorded in the report: `on_message` consults `has_app_manifest()` to decide
//! whether a `None` verdict *rejects* a local app command at all. The seam therefore defines
//! "resolver installed" as "authority is fully governed" — `has_app_manifest()` answers true —
//! so an external engine can never be silently bypassed for unlisted local commands.

use std::collections::BTreeSet;

use stage_a::{ipc_response_with_timeout, local_request, poc_app};
use tauri::ipc::{AuthorityResolver, InvokeBody, Origin, RuntimeAuthority};
use tauri::Manager;
use tauri_utils::acl::resolved::ResolvedCommand;

/// Implementation 1 — wrap upstream's own lookup. No behaviour of its own at all.
pub struct UpstreamWrap(pub RuntimeAuthority);

impl AuthorityResolver for UpstreamWrap {
    fn resolve_access(
        &self,
        command: &str,
        window: &str,
        webview: &str,
        origin: &Origin,
    ) -> Option<Vec<ResolvedCommand>> {
        self.0.resolve_access(command, window, webview, origin)
    }
}

/// Implementation 2 — the ACI shape: deny-by-default from an external table.
///
/// The table names (webview label, command) pairs. Anything unnamed is denied. Remote origins
/// are denied unconditionally — the Charter's rule, enforced by the engine rather than by
/// manifest hygiene.
#[derive(Default)]
pub struct MockAci {
    table: BTreeSet<(String, String)>,
}

impl MockAci {
    pub fn grant(mut self, webview: &str, command: &str) -> Self {
        self.table.insert((webview.into(), command.into()));
        self
    }
}

impl AuthorityResolver for MockAci {
    fn resolve_access(
        &self,
        command: &str,
        _window: &str,
        webview: &str,
        origin: &Origin,
    ) -> Option<Vec<ResolvedCommand>> {
        if !matches!(origin, Origin::Local) {
            return None;
        }
        if self.table.contains(&(webview.into(), command.into())) {
            Some(vec![ResolvedCommand::default()])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// C1 — no resolver installed: behaviour is upstream's, byte for byte. The control.
    #[test]
    fn c1_no_resolver_is_upstream_behaviour() {
        let app = poc_app();
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let ok = ipc_response_with_timeout(&webview, local_request("ping", InvokeBody::default()))
            .expect("local ping must be answered");
        assert_eq!(ok.unwrap(), serde_json::json!("pong"));
    }

    /// C2 — the upstream-wrap resolver installed: still no regression. An empty upstream
    /// authority wrapped in the trait denies (a resolver being installed means authority is
    /// fully governed), and a remote origin is denied exactly as before.
    #[test]
    fn c2_upstream_wrap_governs_and_denies_empty() {
        let app = poc_app();
        app.set_authority_resolver(UpstreamWrap(RuntimeAuthority::new(
            Default::default(),
            Default::default(),
        )));
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let refused =
            ipc_response_with_timeout(&webview, local_request("ping", InvokeBody::default()))
                .expect("must be answered with a rejection");
        assert!(
            refused.is_err(),
            "an installed resolver with an empty authority must deny"
        );
    }

    /// C3 — the mock ACI: exactly the table, nothing but the table.
    #[test]
    fn c3_mock_aci_answers_deny_by_default() {
        let app = poc_app();
        app.set_authority_resolver(MockAci::default().grant("main", "ping"));
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        // Named in the table: allowed, end to end.
        let ok = ipc_response_with_timeout(&webview, local_request("ping", InvokeBody::default()))
            .expect("granted command must be answered");
        assert_eq!(ok.unwrap(), serde_json::json!("pong"));

        // Not named: denied — including a command that EXISTS and would run upstream.
        let refused =
            ipc_response_with_timeout(&webview, local_request("add", InvokeBody::Json(
                serde_json::json!({"a": 1, "b": 2}),
            )))
            .expect("ungranted command must be answered with a rejection");
        assert!(refused.is_err(), "deny-by-default: unlisted command must be refused");
    }

    /// C4 — the engine outranks the table: a remote origin is refused even for a granted
    /// command. Authority never follows possession of a grant across the origin boundary.
    #[test]
    fn c4_mock_aci_refuses_remote_even_when_granted() {
        let app = poc_app();
        app.set_authority_resolver(MockAci::default().grant("main", "ping"));
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let mut remote = local_request("ping", InvokeBody::default());
        remote.url = "https://evil.example.com/".parse().unwrap();
        let refused = ipc_response_with_timeout(&webview, remote)
            .expect("remote invoke must be answered with a rejection");
        assert!(refused.is_err(), "a granted command must still refuse a remote origin");
    }

    /// C5 — identity comes from the Rust side: the same command granted to a *different*
    /// webview label is denied for this one (PD-02 preserved through the seam).
    #[test]
    fn c5_grant_is_per_identity_not_per_command() {
        let app = poc_app();
        app.set_authority_resolver(MockAci::default().grant("other-webview", "ping"));
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();

        let refused =
            ipc_response_with_timeout(&webview, local_request("ping", InvokeBody::default()))
                .expect("must be answered with a rejection");
        assert!(
            refused.is_err(),
            "a grant to another identity must not authorise this one"
        );
    }
}
