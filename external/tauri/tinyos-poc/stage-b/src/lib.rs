//! 08C Stage B — EPIC-H2 §2.2's boundary tests, prototyped early against the fork.
//!
//! These tests are written to graduate into `H2-02`'s real boundary-test set when `EPIC-H2`
//! decomposes: they drive only public `tauri-utils` ACL API, and they feed *hostile* inputs —
//! a capability that **carries** a `remote` grant, not one that omits it (08C trap 4: a carried
//! grant looks intentional; the test must not assume it was).
//!
//! Posture under test (`tinyos-acl` feature = the fork patch):
//! - `PD-03` empty authority first: `Capability.local` defaults to **false**.
//! - The Charter's webview rule: **no `remote` execution context survives resolution.**
//! - An *explicit* `local: true` is an intentional, enumerable grant and is honoured — the
//!   inversion is of the default, not a ban on local authority.

use std::collections::BTreeMap;

use tauri_utils::acl::capability::{Capability, CapabilityFile};
use tauri_utils::acl::manifest::Manifest;
use tauri_utils::acl::resolved::Resolved;
use tauri_utils::acl::ExecutionContext;
use tauri_utils::platform::Target;

/// A one-permission manifest for an `aci` plugin whose permission allows `do-thing`.
pub fn manifest_allowing_do_thing() -> BTreeMap<String, Manifest> {
    let permission: tauri_utils::acl::Permission = serde_json::from_value(serde_json::json!({
        "identifier": "allow-do-thing",
        "commands": { "allow": ["do-thing"] }
    }))
    .unwrap();
    let mut permissions = BTreeMap::new();
    permissions.insert("allow-do-thing".to_string(), permission);
    let manifest: Manifest = serde_json::from_value(serde_json::json!({
        "permissions": permissions,
        "permission_sets": {}
    }))
    .unwrap();
    let mut acl = BTreeMap::new();
    acl.insert("aci".to_string(), manifest);
    acl
}

/// Parse a capability from JSON exactly as a ported manifest would arrive.
pub fn capability(json: serde_json::Value) -> Capability {
    match serde_json::from_value::<CapabilityFile>(json).unwrap() {
        CapabilityFile::Capability(c) => c,
        _ => panic!("expected a single capability"),
    }
}

pub fn resolve(cap: Capability) -> Resolved {
    let mut capabilities = BTreeMap::new();
    capabilities.insert(cap.identifier.clone(), cap);
    Resolved::resolve(&manifest_allowing_do_thing(), capabilities, Target::Windows)
        .expect("resolution must succeed")
}

pub fn contexts_for(resolved: &Resolved, command: &str) -> Vec<ExecutionContext> {
    resolved
        .allowed_commands
        .get(command)
        .map(|cmds| cmds.iter().map(|c| c.context.clone()).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// B1 — `PD-03`: a capability that *omits* `local` must resolve to no local authority.
    /// Upstream defaults `local` to `true`; the ported manifest must not be trusted to have
    /// made the safe choice, because upstream's safe choice is a different one.
    #[test]
    fn b1_local_defaults_to_deny() {
        let cap = capability(serde_json::json!({
            "identifier": "ported-omits-local",
            "permissions": ["aci:allow-do-thing"]
        }));
        assert!(
            !cap.local,
            "PD-03: Capability.local must default to false under the fork"
        );

        let resolved = resolve(cap);
        assert!(
            contexts_for(&resolved, "plugin:aci|do-thing").is_empty(),
            "a defaulted capability must grant nothing"
        );
    }

    /// B2 — the inversion is of the *default*, not a ban: an explicit `local: true` is an
    /// intentional, enumerable grant and must survive resolution.
    #[test]
    fn b2_explicit_local_grant_is_honoured() {
        let cap = capability(serde_json::json!({
            "identifier": "explicit-local",
            "local": true,
            "permissions": ["aci:allow-do-thing"]
        }));
        let resolved = resolve(cap);
        let contexts = contexts_for(&resolved, "plugin:aci|do-thing");
        assert_eq!(
            contexts,
            vec![ExecutionContext::Local],
            "an explicit local grant must resolve to exactly the local context"
        );
    }

    /// B3 — the hostile input 08C trap 4 requires: a capability that **carries** a `remote`
    /// grant. No `remote` execution context may survive resolution — the Charter's rule that
    /// remote content holds no application IPC authority (EPIC-H2 §2.2, sharper half).
    #[test]
    fn b3_carried_remote_grant_is_stripped() {
        let cap = capability(serde_json::json!({
            "identifier": "hostile-carries-remote",
            "local": true,
            "remote": { "urls": ["https://*.attacker.example"] },
            "permissions": ["aci:allow-do-thing"]
        }));
        let resolved = resolve(cap);
        let contexts = contexts_for(&resolved, "plugin:aci|do-thing");
        assert!(
            !contexts.is_empty(),
            "the explicit local half of the grant must still resolve"
        );
        assert!(
            contexts
                .iter()
                .all(|c| matches!(c, ExecutionContext::Local)),
            "no remote execution context may survive resolution, got: {contexts:?}"
        );
    }

    /// B4 — a remote-*only* capability resolves to nothing at all: stripping the remote half
    /// of a grant with no local half must not conjure a local one.
    #[test]
    fn b4_remote_only_capability_resolves_to_nothing() {
        let cap = capability(serde_json::json!({
            "identifier": "hostile-remote-only",
            "remote": { "urls": ["https://*.attacker.example"] },
            "permissions": ["aci:allow-do-thing"]
        }));
        let resolved = resolve(cap);
        assert!(
            contexts_for(&resolved, "plugin:aci|do-thing").is_empty(),
            "a remote-only capability must resolve to no authority whatsoever"
        );
    }
}
