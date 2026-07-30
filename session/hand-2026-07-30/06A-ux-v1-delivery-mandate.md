# Handover 06A — Next-Session Mandate: Deliver the Ti64 Console UX V1

The start-here work order, in the `13F`/`17G`/`04A` mandate pattern. Ordered by the owner
2026-07-30: **deliver the V1 operator-console UX** specified under
[`work/UX-V1/`](../../work/UX-V1/). This order is the owner sign-off
`V1-STRATEGY.md` declares itself binding on.

## 0. The truth path — read this before the binding docs

The two governing documents are, in authority order:

1. [`work/UX-V1/V1-STRATEGY.md`](../../work/UX-V1/V1-STRATEGY.md) — **binding**. Exact
   strings (Part B), visual contract (Part C), region geometry with load-bearing element
   ids (Part D), behaviour contracts (Part E), host wiring seams (Part F), file order
   (Part G), the all-must-pass acceptance checklist (Part H), non-claims (Part I).
2. [`work/UX-V1/SPEC.md`](../../work/UX-V1/SPEC.md) — remains valid for §5 (honesty rule),
   §6 (render budget), §7 (keyboard contract), §8 (TDD stories `STORY-UX-01..06`);
   superseded by V1-STRATEGY wherever they disagree.

**Path correction, stated once so a smaller model never hunts:** both documents cite the
reference implementation at `work/tinyos-console-ux/`. That directory does not exist. The
reference files live at **`work/UX-V1/`** — `v3-console.html` (pixel truth; three layouts
in one build, `?layout=focus|cockpit|split`), `console.css`, `console-core.js`,
`console-v2.js`, `console-bodies.js`. Every "copy from reference" instruction resolves
there. Verified present and coherent this session: `KETTLEBELL` and the `tiweld` droplet
gradient live in `v3-console.html`; the `VERBS` display table lives in `console-core.js`.

Rule A4.1 stands: **copy, never re-derive**. When instinct and the reference disagree,
the reference wins.

## 1. Scope guard — what this work may and may not touch

- Target: `external/tauri/tinyos-poc/stage-e-console-app/ui/**` and the reconciler in
  `stage-e-console-app/src/main.rs`; `stage-e-console/src/**` only at the Part F seams
  (`open_tab`/`run_line`/`read_tab`/`read_console`/`focus_tab`, plus the `lines_since`
  cursor for `STORY-UX-03`).
- `os/src/**`: **no shell-crate change is required or permitted.** The only sanctioned
  `os/src` touch is `xtask` gaining the two generators (`STORY-UX-05` `ui/verbs.json`
  from `shell::verbs::VerbKind::ALL` + the manifest grant tables; `STORY-UX-06`
  `ui/workbench.json` from `goals/assurance/*.tsv` + `goals/performance/catalogue.tsv`).
- Shell output is the Rust core's real output — **never** rebrand `TinyOS …` strings to
  `Ti-OS` (Part B). The parity golden must not change; if a UX step would change it, the
  step is wrong.
- Invariants that must not regress (SPEC §1 table): host-owned reserved/system line with
  `main.rs::reconcile` as its only writer and its label in no grant table; two disjoint
  grant tables; session identity = runtime webview label; per-tab `World`; the
  three-signal parity rule rendered separately; denials visible, never swallowed.

## 2. The work, phased exactly as V1-STRATEGY Part A3/G — commit gate per phase

1. **V1.0 — static reference parity.** Copy the five files into
   `stage-e-console-app/ui/` in Part G order (css → core → v2 → bodies → chrome from
   `v3-console.html`; `reserved.html` restyled to `.reserved` tokens, contract
   untouched, no badge). One change, one check: run the matching Part H items after each
   file; stop on any red. Done = checklist H passes pixel-for-pixel against
   `work/UX-V1/v3-console.html`.
2. **V1.1 — wire the Tauri host.** Delete the browser-side `Session` class; replace
   exactly the Part F seams, nothing else. Same screens, real transcripts; the e2e smoke
   re-run (with `window.smokeKey` from `STORY-UX-04` so the unattended run drives the
   keyboard grammar, not the mouse) replaces the screenshot set.
3. **V1.2 — generated metadata.** `STORY-UX-05`/`-06`, red first as SPEC §8 states (a
   `VerbKind` with no `verbs.json` entry fails; a `workbench.json` entry claiming `live`
   without a Report id fails). This is what keeps the console honest as the verb set
   grows — no hand-edited tool lists survive this phase.
4. **V1.3 — responsive window + suspended tabs.** `STORY-UX-01` (kill
   `resizable(false)`/`WIN_W`/`WIN_H`; reconciler derives regions from
   `WindowEvent::Resized`; red first via the resize e2e) and `STORY-UX-02` (unfocused
   tabs suspended, not hidden: no `read_tab` from an unfocused label within 2 s), with
   `STORY-UX-03`'s append-only `lines_since(cursor)` landing with or before it (SPEC §6
   render budget item 1 depends on it).

RT and Agent tab bodies remain **display models** (`console-v2.js`) with their caveat
lines on screen until Phases 3/6 provide real state — Part F says so; do not wire them
to anything.

## 3. Interlock with the open backlog (registered 2026-07-30, commit `96945fa`)

- **`LE-59` (rich/coloured IO)** — delivered by V1.0's `paintLine()` colouriser (Part
  C2). Close it citing this delivery **only if** the 6.5-rule-3 constraint is kept:
  colour is applied by the renderer over classified output, never payload escapes.
- **`LE-60` (monitors surface)** — partially delivered by the workbench rail **with the
  §5 honesty states** (`live`/`pending (uncalibrated)`/`absent (LE-xx)`); the kernel
  counters stay open Stories. Do not flip `LE-60` closed; annotate progress in its row
  only if the register grammar allows, else leave for the close-out session.
- **`LE-61` (mode banner + flavour bar)** — the workload selector and system line carry
  most of it; whatever the reconciler's system-line text does not yet state about
  emulation reality stays open under `LE-61`.
- **`LE-57` (`HELP`)** — **not this work.** V1-STRATEGY E8 is a hard gate: the UI may
  name only verbs in the `VERBS` table (mirror of `dos.rs`); `HELP` is a recorded
  illegal example until the shell verb ships. When `LE-57` lands later, it enters the UI
  through the `STORY-UX-05` generator, not by hand.
- **`LE-58` (editor)** — untouched by this mandate.

## 4. Acceptance — all of it or it isn't done

1. V1-STRATEGY **Part H, items 1–15**, every item green, in a fresh boot at 1440×860.
2. SPEC **§6 render budget**: append cost < 1 ms after 1 000 lines and exactly one node
   per line; 5 000-line ring; one 4 Hz clock paused on `document.hidden`; ≤ 450 DOM
   nodes at boot; ≤ 40 KB added payload; the `nodes · ms` readout visible on the system
   line.
3. SPEC **§7 keyboard contract**: no pointer required for any function; roving tabindex;
   visible focus ring; `F1` map.
4. Existing gates stay green: `cargo test -p stage-e-console` (all targets), the e2e
   smoke run end-to-end (screenshots + `smoke.json` recommitted as a dated report in
   `goals/reports/`), three-signal parity untouched, `check-assurance-spine` green.
5. The `08C`-pattern close-out: a dated Report with the Part H checklist as its evidence
   table, statuses updated, and whichever of `LE-59`/`LE-60`/`LE-61` rows are honestly
   affected updated in the same commit.

## 5. Bounds and non-claims (Part I, restated)

Nothing here is `PD-*`/`TG-P*`/timing evidence. The GUI flavour (`F12`) is not a
compositor and implies no on-target graphics stack — `LE-53` stands. The CPU meter is a
shape wired to `--t-pending`, never a quotable number (`ADR 0005`;
`qualified-platforms.tsv` qualifies none). Windows Terminal is a palette/legibility
reference only — no code copied, linked, or built. Estimated shape: V1.0+V1.1 one
focused session; V1.2+V1.3 a second. Sequence exactly as numbered — the generators
(V1.2) before the window work (V1.3) only because UX-06's honesty data feeds the rail
the resize test exercises; do not reorder.

## 6. Session-discipline notes for the implementer

Install the hooks (`git config core.hooksPath .githooks`) and read
`agent/CONCURRENT_SESSIONS.md` first — the soak logger appends to
`goals/reports/_soak-p0-03-01.log` on its own cadence; leave its rows alone. Stage
narrowly; `work/UX-V1/` is the committed design record — do not edit it to match what
you built; if the build must diverge, file the note Part A4.1 asks for and record it in
your handover.
