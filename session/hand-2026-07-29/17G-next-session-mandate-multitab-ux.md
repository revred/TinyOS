# Handover 17G — Mandate: Visual Confirmation of the Single-Window Multi-Tab UX, Parity Suite Runnable From a Tab

The start-here document for the next session, in the `08C`/`13F` cover-note pattern.
Ordered by the owner 2026-07-30: *"full visual confirmation of the capability of the single
window multi-TAB OS UX with an ability to run all the parity tests on MSDOS."*
Follows [`16G`](16G-tinycmd-vertical-slice.md) (TINYCMD exists, parity gate green) and
[`15G`](15G-fork-vendored-in-tree.md) (one repo). Branch: `os.tauru.poc`.

## 0. The correction that keeps this honest (standing, from 13F §0)

`LE-53` is not reopened: Tauri remains disqualified as the **on-target** tab host, and
`EPIC-P2` §6.3's real trusted path needs the OS underneath. What this mandate orders is the
**host-side operator console grown into the multi-tab shape** — the `EPIC-H4` lane the Stage E
console already proved, now demonstrating `EPIC-P2` §6's *interaction model*: one window, many
tabs, a host-owned reserved region no tab content can paint. Stage A's `a3` test already
proved that reserved-region-as-sibling-webview shape on MockRuntime; this makes it visible.

## 1. What to build — `stage-e-console` grows tabs

In `external/tauri/tinyos-poc/` (in-repo since ADR 0009). The `shell` crate compiles on the
host, so **a tab is a host-run TINYCMD session in-process** (`World` + `dos::run_line`), and
one tab drives the real kernel under QEMU through the existing harness.

1. **Single window, ≥3 tabs**: two independent DOS-session tabs, one target-parity tab.
   Tab content webviews are siblings under one window; the **reserved region is a separate
   host-owned webview** (the `a3` shape) showing focused-tab identity and flavour —
   repainted only by the Rust side, never reachable from tab content.
2. **Per-tab session boundary** (§6.1 at host level): each tab owns its `World` — its own
   env, cwd, volume view and policy. `SET X=1` in tab 1 must be invisible in tab 2, visibly.
3. **The parity suite runs from a tab**: one action runs *all* the MS-DOS parity tests —
   the 20 host `shell` tests, the QEMU `shell-batch` fixture, and `check-shell-parity` —
   with per-test PASS/FAIL rendered in the tab and an overall verdict in the reserved region.
4. **Authority stays manifest-shaped**: the signed-manifest resolver extends its verb
   enumeration (`open_tab`, `focus_tab`, `run_line`, `run_parity`, …); an unlisted verb's
   denial stays visible in the UI. Re-sign with `sign-manifest`; the dev-key non-claim stands.

## 2. Acceptance — "full visual confirmation" means artifacts, not adjectives

1. **Screenshots, committed**: an unattended smoke mode (extend `STAGE_E_SMOKE`) drives the
   window — tabs opened, `DIR` run in tab 1, isolation shown against tab 2, parity suite run
   — capturing PNG screenshots at each step (host screenshot API is fine) into
   `goals/reports/assets/2026-07-30-multitab/`, referenced from the report. A reader must be
   able to *see* the tab bar, the reserved region, a live `DIR`, and the parity PASS wall.
2. **Smoke JSON** with: tab count, per-tab session ids, the isolation check result, per-test
   parity verdicts, denial log — machine evidence beside the pictures.
3. **The two-signal rule holds** for the parity tab: fixture exit verdict AND transcript
   comparison, exactly as `check-shell-parity` computes them.
4. Upstream suite still green both knob positions; the vendored-crate patch metric stays
   unchanged (console work is `tinyos-poc/`-side only).

## 3. Non-claims (state them in the report)

Host-side demonstration of the *interaction model* only: not the on-target tab host, not
`§6.3`'s kernel-enforced trusted path, no `PD-01/07/08/12` evidence, no performance claim —
the `TG-P*` rows stay red (though `TG-P02`/`TG-P03` prototyping from the parity tab is
welcome if cheap). No support claim for Tauri anywhere (`EPIC-H2` §4).

## 4. Sequencing

1. Tab/session model in `stage-e-console` core (headless tests first — sessions isolated,
   policy per tab).  2. Window shape: sibling webviews + reserved region.  3. Parity-suite
   runner + smoke mode + screenshots.  4. Report (`REPORT-2026-07-30-*`), handover, spine
   green, push. Then the standing queue: `FEAT-P2-05`/`-06`, batch control flow, upstream PR,
   ADR 0010/0011, terminal-gap spine gate, the merge decision.
