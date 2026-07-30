# Handover 01A — The Multi-Tab UX Is Visible: One Window, Three Tabs, the Parity Suite Green From a Tab

Executes [`17G`](../hand-2026-07-29/17G-next-session-mandate-multitab-ux.md) in full, plus the
owner's mid-session rider: *"Run the OS in a browser and show a sample TCB delivering on
expectations."* Branch `os.tauru.poc`. Dated record:
[`REPORT-2026-07-30-01`](../../goals/reports/REPORT-2026-07-30-01.md); the pictures live in
[`goals/reports/assets/2026-07-30-multitab/`](../../goals/reports/assets/2026-07-30-multitab/).

## 1. What exists now

One window (`host`), sibling webviews — the `a3` shape made visible, on the Stage E console
grown into the 17G shape:

1. **A tab is a host-run TINYCMD session in-process** (`stage-e-console::tabs`):
   `shell::verbs::World` + `shell::dos::run_line`, the same crate the QEMU fixture boots.
   Six enumerated tab identities (`tab-1`…`tab-6` / sessions `TAB-1`…`TAB-6`), typed
   refusals beyond capacity, per-tab env/cwd/volume/policy — `SET GREET=…` in tab 1 is
   *visibly* `not defined` in tab 2 (headless `t1`–`t7`, IPC-path `e6`, screenshot 04).
2. **The reserved region is a host-owned sibling webview** whose label is enumerated
   nowhere in the signed manifest: it can invoke nothing (`r6`/`e8`) and only the Rust-side
   reconciler ever repaints it — identity + flavour of the focused tab, the parity verdict,
   the denial count.
3. **The signed manifest carries two disjoint grant tables** (chrome verbs vs tab verbs,
   `m5`/`r5`/`e7`): a tab cannot open tabs, the chrome cannot run lines, the session for a
   tab verb is the invoking webview's runtime-derived label. Re-signed; dev-key non-claim
   stands.
4. **The parity suite runs from the parity tab**: `cargo test -p shell --lib` then
   `cargo run -p xtask -- check-shell-parity` (the exact CI surfaces), per-test PASS/FAIL
   streamed into the tab, the **two signals rendered separately** and aggregated
   affirmative-both-or-FAIL (`s1`–`s3`). Wall: 22 host rows + 2 target signals, all green
   (screenshot 06).
5. **`SAMPLE.TCB` delivers on expectations, on screen** (the rider): seeded in every DOS
   tab, run DOS-style by typing its name at the prompt, through the real `.TCB` runner —
   echo discipline, `%DEMO%` expansion, `COPY`/`MD` side effects confined to that tab
   (`t7`, screenshot 05). The "browser" is the WebView2 engine rendering every tab;
   serving to a *remote* browser is what the resolver's unconditional remote-origin
   refusal forbids — stated as a non-claim, not delivered by accident.
6. **The unattended smoke** (`STAGE_E_SMOKE=1`) drives all of it through the pages' own
   hooks (each step from the correct webview identity), captures 7 PNGs + `smoke.json`
   (tab count, session ids, isolation result, 24 parity verdicts, denial log), exits with
   the aggregate verdict — exit 0, committed.

Two `shell` additions, test-first: `World.policy: &(dyn VerbPolicy + Sync)` so a
`World<'static>` is `Send` (`c5`, red-first as a compile failure) and `batch::prompt` made
public (`b4`) so interactive echo is byte-identical to batch echo. Shell 22/22.

## 2. Numbers and honesty

- Suites: PoC workspace 48 (console 24 lib + 7 e2e, stages 5/4/5/3); `shell` 22/22; `xtask`
  204/204; spine green; upstream **57/57 knob off, 58/58 on**, `tauri-utils` 33/33 both;
  patch metric **unchanged** (nothing under `crates/` touched).
- **Findings:** the first smoke run failed honestly — a stray desktop click had opened a
  fourth tab before the sequence began, and the hard-coded `tab-3` drove the wrong tab. The
  smoke is now label-independent (17G says ≥3 tabs) and records the role→label map in the
  JSON. Also: `cargo test -p tauri --lib` with default features dies at load on this box
  (`common-controls-v6`, the comctl32 fact from REPORT-2026-07-29-04) — the knob suite is
  `--no-default-features --features test[,tinyos]`.
- **Stated debt:** fixed-size window; hidden-not-suspended unfocused tabs; one parity-suite
  state (one meaningful parity tab at a time); the suite runner parses `cargo`'s
  human-readable output (a format change surfaces as a red row — the safe direction).
- **Non-claims:** host-side interaction model only; `LE-53` stands (not the on-target tab
  host); no `PD-*` or `TG-P*` evidence; no Tauri support claim.

## 3. Queue (the 17G standing queue, unchanged in order)

1. `FEAT-P2-05` (POSIX front-end) + three-way flavour equivalence; `FEAT-P2-06` (RT).
2. Batch control flow (`IF`/`GOTO`/`FOR`/`CALL`), redirection/pipes in the core.
3. First `D23` measurements through the batch fixture (`TG-P02`/`TG-P03` prototypes — 17G
   says prototyping from the parity tab is welcome if cheap).
4. Upstream PR (U1), ADR 0010/0011, terminal-gap spine gate, the `main` merge decision.
