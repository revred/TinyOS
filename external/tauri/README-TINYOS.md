# TinyOS Tauri Fork — PoC repository

This repository executes the PoC ordered by TinyOS
`session/hand-2026-07-29/08C-tauri-poc-execution-cover-note.md`, under the constraints of
`docs/adr/0007-modifying-tauri-is-in-scope-at-the-seams.md` and `goals/epics/EPIC-H2.md` §2.

**Baseline: upstream tag `tauri-runtime-wry-v2.11.4` (`ca90b46`)** — the release tag matching
`tauri-runtime-wry` 2.11.4 per ADR 0007 constraint 3. All modifications are commits on top of the
tag; the health metric is `git diff --stat tauri-runtime-wry-v2.11.4` (cumulative across stages,
review §7.3 / ADR 0007 constraint 2).

## The exclusion rule (ADR 0007 constraint 4 — binding)

**No crate in this repository may ever be a member of the TinyOS `os/` workspace or a `path =`
dependency of any TinyOS workspace crate.** `tauri` measures over the 20,000-line crate ceiling and
`agent.md` rules 4 and 7 are not amended. The moment a TinyOS workspace crate names this tree by
`path =`, a reference has silently become an in-workspace fork — the failure
`session/hand-2026-07-29/06A-tauri-internals-reviewed.md` §4 warns about. This rule travels with
every declaration of this repository (submodule entry, README, lockfile comment).

TinyOS-side PoC code lives in `tinyos-poc/` here, in its own cargo workspace, path-depending on the
vendored crates — permitted in this direction only.

## Stage 0 — review reproduction at the tag (2026-07-29)

Verdict: **PASS**. Every claim of `docs/tauri-internals-review.md` survives at the tag.

| Claim (review §) | At `dev@872428f` | At tag `ca90b46` | Survives |
|---|---|---|---|
| `tauri-runtime-wry` version | 2.11.4 | 2.11.4 (`crates/tauri-runtime-wry/Cargo.toml`) | yes |
| `wry` / `tao` pins | 0.55 / 0.35 | 0.55.0 / 0.35.0 | yes |
| `resolve_access(&cmd_name, window.label(), self.label(), &origin)` from Rust-side objects (§2.2) | `webview/mod.rs:1518` | `webview/mod.rs:1458` | yes (line drift only) |
| Origin from current URL per call (§2.3) | `~1505` | `~1445` (`is_local_url` → `Origin::Local/Remote`) | yes |
| `RuntimeAuthority` = two `BTreeMap<String, Vec<ResolvedCommand>>`, concrete struct, no trait (§2.4) | `authority.rs:28` | `authority.rs:~29` | yes |
| `Capability.local` defaults `true` (§3) | asserted | `capability.rs:147-148`, `default_capability_local() -> bool { true }` | yes |
| `remote` URL-pattern contexts exist (§3) | asserted | `capability.rs:146` `pub remote: Option<CapabilityRemote>` | yes |
| `__TAURI_INVOKE_KEY__` bearer secret (§2.5) | asserted | present: `app.rs`, `ipc/protocol.rs`, `webview/mod.rs` | yes |
| multiwebview behind `unstable` (§4) | asserted | `tauri/Cargo.toml:202`, `Window::add_child` at `window/mod.rs:1129` | yes |
| `MockRuntime` in-tree (08C trap 2) | asserted | `crates/tauri/src/test/mock_runtime.rs:1149` | yes |
| Crate sizes: tauri 32,457 / tauri-utils 15,452 / tauri-runtime 2,683 / tauri-runtime-wry 6,719 | at `dev` | **32,207 / 15,220 / 2,607 / 6,718** (src `*.rs`, `wc -l`) | yes — smaller at tag, same 1.6× ceiling conclusion; review's numbers remain correct for its own pin |

No review amendment required: no claim fell; the size figures differ because the review measured a
later `dev` commit, which its Provenance section already states.

## Stage E — the host-side operator console (2026-07-29, 13F Deliverable A)

Verdict: **PASS** (Stage E cannot kill the fork; a failure would have been a finding about
the console lane). `tinyos-poc/stage-e-console` (core, headless-testable) +
`tinyos-poc/stage-e-console-app` (the WebView2 window). Composition proven end to end:

- **Signed manifest**: ed25519 over the verb enumeration (`launch_fixture`, `send_line`,
  `read_stream`, `terminate`); verification is the only path to a resolver — fail-closed by
  type. The committed signing key is a **PoC dev key, not a custody model**.
- **Authority**: `ConsoleAuthority` over the Stage C `AuthorityResolver` seam —
  deny-by-default, remote refused unconditionally, identity is the runtime-derived webview
  label, every denial recorded and rendered in the UI.
- **Target**: TinyOS-under-QEMU through the *same command surface CI uses*
  (`cargo run -p xtask -- qemu-x86_64 --fixture=<name> --serial-capture=<path>`), serial
  tailed live into the console pane, verdict = `xtask`'s own exit-code mapping.
- **Windowed smoke evidence** (`STAGE_E_SMOKE=1`, page-driven invokes): `measure` fixture
  PASS with the `TINYOS-MEAS/2` envelope streamed; unlisted verb `format_disk` denied by
  the signed manifest, denial visible.
- **Patch metric unchanged**: `16 files changed, 224 insertions(+), 19 deletions(-)` vs the
  tag — the console is `tinyos-poc/`-side, zero vendored-crate lines.
- Upstream suite still green both knob positions: tauri 57/57 off, 58/58 on;
  `tauri-utils` 33/33 both.
- `send_line` resolves through the manifest and then reports the honest transport fact:
  the target kernel's serial is TX-only today (no UART RX path in `hal-x86_64`) — recorded
  in TinyOS `docs/terminal-gap-analysis.md`.
- Two build-tooling stubs, not part of the measured diff (the `vswhom-sys` pattern):
  `stage-e-console-app/rc_shim.rs` replaces the absent Microsoft Resource Compiler with an
  empty-`.res` emitter (so the exe carries no icon/version resources, and
  `common-controls-v6` stays off — no manifest-activated comctl32 v6).

## 17G — the single-window multi-tab OS UX, visually confirmed (2026-07-30)

The Stage E console grown into the `EPIC-P2` §6 *interaction model*, host-side (the `a3`
sibling-webview shape made visible). Evidence: TinyOS `goals/reports/REPORT-2026-07-30-01.md`
plus 7 committed screenshots and `smoke.json` under
`goals/reports/assets/2026-07-30-multitab/`.

- **One window, sibling webviews**: `reserved` (host-owned strip — its label is enumerated
  nowhere in the manifest, so it holds no verbs, and only the Rust-side reconciler repaints
  it), `console` (tab-bar chrome), `tab-1`…`tab-6` (content).
- **A tab is a host-run TINYCMD session in-process** (`shell::verbs::World` +
  `shell::dos::run_line`, path-dependency `tinyos-poc → os/src/shell`, the legal direction):
  per-tab env/cwd/volume/policy, isolation visible (`SET` in one tab, `not defined` in the
  next), `SAMPLE.TCB` run by name through the real `.TCB` runner.
- **The signed manifest carries two disjoint grant tables** (chrome verbs vs tab verbs); a
  tab verb's session identity is the invoking webview's runtime-derived label.
- **The parity suite runs from the parity tab** via the CI command surfaces
  (`cargo test -p shell --lib`, `cargo run -p xtask -- check-shell-parity`), with the
  two-signal rule rendered and aggregated affirmative-both-or-FAIL.
- Patch metric **unchanged** (nothing under `crates/` touched); upstream suite green both
  knob positions (57/57 off, 58/58 on; `tauri-utils` 33/33 both — run headless:
  `--no-default-features --features test[,tinyos]`, since default features include
  `common-controls-v6`, which cannot load on this machine).
