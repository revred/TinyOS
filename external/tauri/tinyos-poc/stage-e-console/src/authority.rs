//! The console's authority resolver: the Stage C seam, fed by the signed manifest.
//!
//! Deny-by-default in every direction: a verb the manifest does not name is denied, a
//! webview label the manifest does not name is denied, and a remote origin is denied
//! unconditionally — table or no table (the Charter's rule, enforced by the engine rather
//! than by manifest hygiene, exactly as Stage C's `MockAci` proved).
//!
//! Every denial is recorded with its identity tuple so the UI can *show* the refusal
//! (13F acceptance 2: "the denial is visible in the UI"), not merely reject a promise.

use std::sync::{Arc, Mutex};

use tauri::ipc::{AuthorityResolver, Origin};
use tauri_utils::acl::resolved::ResolvedCommand;

use crate::manifest::VerifiedManifest;

/// One recorded denial: the runtime-derived identity tuple and the reason.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Denial {
    /// The command that was refused.
    pub command: String,
    /// The window label the invoke arrived through.
    pub window: String,
    /// The webview label the invoke arrived through.
    pub webview: String,
    /// The origin, rendered (`local` or the remote URL).
    pub origin: String,
    /// Why it was refused.
    pub reason: String,
}

/// Shared, cloneable view of the denial log — the resolver writes it, the UI reads it.
pub type DenialLog = Arc<Mutex<Vec<Denial>>>;

/// The resolver. Constructed only from a [`VerifiedManifest`] — verification already
/// happened or this value cannot exist.
pub struct ConsoleAuthority {
    manifest: VerifiedManifest,
    denials: DenialLog,
}

impl ConsoleAuthority {
    /// Build the resolver around an already-verified manifest.
    pub fn new(manifest: VerifiedManifest) -> Self {
        Self { manifest, denials: Arc::new(Mutex::new(Vec::new())) }
    }

    /// The shared denial log, for handing to the UI state.
    pub fn denial_log(&self) -> DenialLog {
        Arc::clone(&self.denials)
    }

    fn deny(&self, command: &str, window: &str, webview: &str, origin: &Origin, reason: &str) {
        self.denials.lock().expect("denial log lock").push(Denial {
            command: command.into(),
            window: window.into(),
            webview: webview.into(),
            origin: origin.to_string(),
            reason: reason.into(),
        });
    }
}

impl AuthorityResolver for ConsoleAuthority {
    fn resolve_access(
        &self,
        command: &str,
        window: &str,
        webview: &str,
        origin: &Origin,
    ) -> Option<Vec<ResolvedCommand>> {
        if !matches!(origin, Origin::Local) {
            self.deny(command, window, webview, origin, "remote origins are refused unconditionally");
            return None;
        }
        if webview != self.manifest.console() {
            self.deny(command, window, webview, origin, "identity is not the manifest's console webview");
            return None;
        }
        if !self.manifest.allows(command) {
            self.deny(command, window, webview, origin, "verb is not enumerated in the signed manifest");
            return None;
        }
        Some(vec![ResolvedCommand::default()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ManifestPayload, SignedManifest};

    const TEST_SECRET: &str = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";

    fn authority() -> ConsoleAuthority {
        let payload = ManifestPayload {
            console: "console".into(),
            verbs: vec!["launch_fixture".into(), "read_stream".into()],
        };
        let signed = SignedManifest::sign(payload, TEST_SECRET).unwrap();
        let public = crate::manifest::public_key_hex(TEST_SECRET).unwrap();
        ConsoleAuthority::new(signed.verify(&public).unwrap())
    }

    /// R1 — a manifest-listed verb from the console webview, local origin: allowed.
    #[test]
    fn r1_listed_verb_local_console_allowed() {
        let auth = authority();
        let verdict = auth.resolve_access("launch_fixture", "main", "console", &Origin::Local);
        assert!(verdict.is_some(), "an enumerated verb from the console identity must resolve");
        assert!(auth.denial_log().lock().unwrap().is_empty());
    }

    /// R2 — an unlisted verb is denied and the denial is recorded with its identity tuple —
    /// the record the UI renders.
    #[test]
    fn r2_unlisted_verb_denied_and_recorded() {
        let auth = authority();
        let verdict = auth.resolve_access("format_disk", "main", "console", &Origin::Local);
        assert!(verdict.is_none(), "deny-by-default: an unlisted verb must be refused");
        let log = auth.denial_log();
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].command, "format_disk");
        assert_eq!(log[0].webview, "console");
        assert!(log[0].reason.contains("not enumerated"));
    }

    /// R3 — a remote origin is refused even for a listed verb: possession of a grant never
    /// crosses the origin boundary (Stage C's C4, preserved).
    #[test]
    fn r3_remote_origin_refused_even_when_listed() {
        let auth = authority();
        let remote = Origin::Remote { url: "https://evil.example.com/".parse().unwrap() };
        let verdict = auth.resolve_access("launch_fixture", "main", "console", &remote);
        assert!(verdict.is_none());
        let log = auth.denial_log();
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert!(log[0].origin.contains("evil.example.com"));
    }

    /// R4 — a different webview label is denied the same listed verb: the grant follows the
    /// runtime-derived identity, not the command (Stage C's C5, preserved).
    #[test]
    fn r4_other_webview_identity_denied() {
        let auth = authority();
        let verdict = auth.resolve_access("launch_fixture", "main", "some-tab", &Origin::Local);
        assert!(verdict.is_none());
        let log = auth.denial_log();
        let log = log.lock().unwrap();
        assert_eq!(log[0].reason, "identity is not the manifest's console webview");
    }
}
