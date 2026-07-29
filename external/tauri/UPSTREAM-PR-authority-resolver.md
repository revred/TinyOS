# Draft upstream PR — `feat(core): pluggable authority resolver for RuntimeAuthority`

Target: `tauri-apps/tauri`, `dev` branch. Drafted 2026-07-29 from the TinyOS fork PoC
(Stage C of `session/hand-2026-07-29/08C`); prepared per ADR 0007 constraint 6 —
*upstream-first for mechanical seams*. Not yet submitted.

## Title

feat(core): allow `RuntimeAuthority` to defer access resolution to an external resolver

## Motivation

`RuntimeAuthority::resolve_access` is the single point where every IPC allow/deny verdict is
decided, but it is a concrete `BTreeMap` lookup with no seam. Embedders who resolve authority
somewhere else — an OS policy engine, a broker process, an audit-logging wrapper, a test
harness that wants deterministic denials — currently have no way to interpose without forking
this file. Everything *around* the decision is already beautifully seam-shaped
(`tauri-runtime` for windowing, per-webview IPC handlers for identity); the decision itself is
the one thing that is not.

## Design

- New public trait in `tauri::ipc`:

  ```rust
  pub trait AuthorityResolver: Send + Sync + 'static {
    fn resolve_access(
      &self,
      command: &str,
      window: &str,
      webview: &str,
      origin: &Origin,
    ) -> Option<Vec<ResolvedCommand>>;
  }
  ```

  The signature is exactly `RuntimeAuthority::resolve_access`'s: identity arguments are
  runtime-derived labels (never caller-supplied — the property the existing call sites
  already guarantee), `None` denies.

- `RuntimeAuthority` gains an optional `Box<dyn AuthorityResolver>`; `resolve_access`
  delegates to it when present and is byte-for-byte unchanged when absent.

- `Manager::set_authority_resolver` installs one.

- Semantics choice worth reviewing: **an installed resolver makes authority fully governed**
  — `has_app_manifest()` answers true, so the `on_message` fast path that lets local app
  commands through for apps with no ACL manifest no longer bypasses the resolver. Rationale:
  a resolver that can be bypassed for exactly the commands nobody enumerated is not an
  authority seam; and an embedder who installs one has, by construction, an app-level policy.
  This mirrors the existing behaviour where defining any app ACL manifest turns enforcement
  on.

## What does not change

- No behaviour change unless a resolver is installed; the full unit suite passes unmodified.
- Scope resolution (`ScopeManager`, `CommandScope`, `GlobalScope`) is untouched: an external
  resolver controls the allow/deny verdict and may attach `scope_id`s from resolved commands
  it returns; scope *interpretation* remains plugin-side, as today.
- `resolve_access_message` (the debug-mode denial diagnostics) still reads the built-in
  tables; with an external resolver the generic denial message is used.

## Diff summary (measured on the fork, tag `tauri-runtime-wry-v2.11.4`)

- `crates/tauri/src/ipc/authority.rs`: +36 (trait, field, delegation, `set_resolver`,
  `has_app_manifest` composition)
- `crates/tauri/src/ipc/mod.rs`: +2/−1 (export)
- `crates/tauri/src/lib.rs`: +12 (`Manager::set_authority_resolver`)

## Tests included

Five end-to-end tests over `MockRuntime` (no platform webview needed):

1. no resolver installed → upstream behaviour, byte for byte;
2. an installed resolver wrapping an empty upstream authority denies (full governance);
3. an external deny-by-default table allows exactly its entries, end to end through
   `on_message`;
4. a granted command still refuses a remote origin;
5. a grant to another webview label does not authorise this one (identity remains
   runtime-derived through the seam).
