# Handover 07A — 06A Executed: V1.0 + V1.1 of the Ti64 Console Are Delivered

The execution record for [`06A`](06A-ux-v1-delivery-mandate.md) phases V1.0 and V1.1, in
the `08C` close-out pattern. Evidence: [`REPORT-2026-07-30-03`](../../goals/reports/REPORT-2026-07-30-03.md)
(keyboard-driven smoke, screenshots, `smoke.json`). V1.2 (generated metadata) and V1.3
(responsive window + suspended tabs) remain for the next session, exactly as 06A §5
estimated and sequenced — do not reorder them.

## 1. What landed, commit by commit

1. **V1.0 (`de0a9d4`)** — the five `work/UX-V1/` reference files land in
   `stage-e-console-app/ui/` byte-identical (verified with `cmp` per file; A4.1
   copy-never-re-derive means pixel parity holds by construction). `reserved.html`
   restyled to the `.reserved` tokens, 28 px, no badge, contract untouched.
2. **V1.1 (this handover's commit)** — the Part F seams, exactly:
   - `new Session(...)` → `invoke("open_tab", {kind, slot})`; the browser-side `Session`
     class (and its seed volume) is **deleted** from `console-core.js`.
   - `session.run(line)` → `invoke("run_line", {line})` under the tab's own webview
     label; the 4 Hz tick → `invoke("read_tab")`; tab list/focus → `read_console` /
     `focus_tab`; the system line → painted by `main.rs::reconcile` only.
   - The smoke is re-established as the V1 sequence, driven through the keyboard
     grammar via `window.smokeKey` (`STORY-UX-04`'s hook), not the mouse.

## 2. The architecture decision V1.1 forced (read before touching V1.2/V1.3)

The v3 reference is a **single page** holding both chrome affordances (open/focus tabs)
and session affordances (run lines). The host splits those across **disjoint grant
tables keyed by webview label** (R5/E7 — the demo's central denial). No single webview
may hold both. Part F's seam table is silent on this contradiction, so per A4.1 a note
is filed here:

**Resolution — identity satellites + a rendering-layer UI bus.** The `console` webview
renders every visible pixel (the reference build, unmodified geometry). Each open tab
gets a **1×1 identity satellite** (`tab.html`) whose only job is to hold the tab verbs
under its own runtime label: it polls `read_tab`, executes `run`/`parity` intents, and
relays snapshots. Chrome ↔ satellite traffic travels a same-origin BroadcastChannel
(+ `localStorage` fallback), which carries **display data and UI gestures only — no
capability crosses the bus**. Every invoke still resolves at the authority seam under
the identity that legitimately holds the verb; a direct `run_line` from the chrome is
still denied and still lands red in the transcript. E6/E7/E8 and R1–R6 are untouched
and green.

Why not the alternatives: rendering tab panes in per-tab webviews forces the rail, the
master menu and the input row (which inject verbs into sessions) to hold both grant
tables or to smuggle a new "run in focused session" chrome verb — a capability the 17G
design exists to deny. The satellite model keeps the resolver's story exact and keeps
the whole visual contract in one page (grip drag, layouts, GUI flavour all remain pure
CSS/DOM).

**Part B repair mid-session:** the first smoke exposed a second-id defect — the chrome
numbered UI tabs (`tx04`) while the host numbered slots (`tab-2` → painted `tx02`).
Fixed by `TabRegistry::open_at(kind, slot)` (+ typed `SlotTaken`, test T8): the chrome
opens the enumerated slot matching the tx name, so strip, session identity and system
line are one name. Display-model tabs (rt/agent) hold tx names with no host session.

## 3. Divergence notes (Part A4.1) — reference/spec vs delivered

1. **Payload budget cannot hold.** Part H item 15 says ≤ 40 KB (SPEC §6 says ≤ 32 KB);
   the verbatim reference itself is ~110 KB across the five files (~97 KB after the
   Session deletion). Copy-never-re-derive outranks the figure. Owner decision needed:
   correct the figure or order a minify story. Do **not** shrink the files by hand.
2. **System line carries no layout token** (`· Cockpit`): layout is chrome state the
   host cannot know, and inventing a channel for it was out of V1.1's seam budget. The
   V1.3 region-derivation work needs a chrome→host geometry seam anyway — land the
   layout token through that, with the owner's sign-off on the seam's shape.
3. **System line counts host-backed sessions only** (boot says `1 tab(s)` while the
   strip shows tx01–tx03): rt/agent are display models (Part F), not sessions; the
   host does not count what it does not run (§5 honesty). Becomes moot in Phases 3/6.
4. **A focused display tab does not move the system line** — it keeps naming the last
   host-backed session. Same root cause as 3.
5. **`NEW LINUX SESSION` prints the FEAT-P2-05 pending note** instead of opening a
   browser-faked POSIX session — §5 honesty beats hostless demo behaviour.
6. **Host sessions boot with an empty transcript** (no synthetic banner line): shell
   output is the Rust core's real output, nothing else (Part B last row).
7. **The parity body renders the aggregate state** (`not run`/`running…`/`PASS`/`FAIL`)
   for all three signal rows; real per-signal values are in the suite state and
   `smoke.json`. Per-signal rendering rides with V1.2's `console-bodies.js` touch.
8. **Satellites do not pause on `document.hidden`** (they are 1×1, always shown, and
   must keep polling under their own identity). The chrome's single 4 Hz clock does
   pause. Real suspension is `STORY-UX-02` (V1.3) — its "no `read_tab` from an
   unfocused label within 2 s" test will force the satellite pause path.
9. **One reference bug fixed in `console-core.js::mountTranscript`:** the reference
   scrolls `.screen` on append, but the scrolling element is its parent `.scrollpane`,
   so appended lines (including denials) landed below the fold. The delivered build
   also scrolls the parent. One line; behaviour the reference clearly intended.
10. **Transcript classification is renderer-side** over the real shell's register
   strings (`Access denied…`, `Bad command or file name`, …) — exactly LE-59's 6.5-3
   constraint (colour over classified output, never payload escapes). LE-59 is closed
   citing this delivery.

## 4. State of the backlog interlock (06A §3)

- **LE-59** — **closed** this session (colouriser over classified real output; the
  constraint held).
- **LE-60 / LE-61** — rows untouched (open), per the mandate: the rail + honesty
  states and the MODES/FLAGS row deliver part of the surface; kernel counters and the
  emulation-reality banner remain open work.
- **LE-57 (`HELP`)** — untouched; the UI names only `VERBS`-table verbs. When HELP
  ships it enters via the `STORY-UX-05` generator.
- **LE-58 (editor)** — untouched.

## 5. Next session (V1.2 + V1.3) — start here

1. Re-read 06A §2 items 3–4 and SPEC §8. **Generators before window work** (UX-06's
   honesty data feeds the rail the resize test exercises).
2. `STORY-UX-05`/`-06` red first: a `VerbKind` with no `verbs.json` entry fails; a
   `workbench.json` entry claiming `live` without a Report id fails. xtask is the only
   sanctioned `os/src` touch. Consider registering the UX stories' contract rows in
   the spine with that tranche (this session followed the 17G/host-tooling precedent:
   report-tracked, no new spine rows).
3. `STORY-UX-01`: kill `resizable(false)`/`WIN_W`/`WIN_H`; the reconciler derives
   regions from `WindowEvent::Resized`. This is where divergences 2 (layout token) and
   the fixed reserved-overlay rectangle (`RESERVED_X/Y/W` in `main.rs`) get retired.
4. `STORY-UX-02`/`-03`: satellite suspension + `lines_since(cursor)` (the bus currently
   relays whole transcripts; the chrome already appends deltas client-side).
5. Then the full Part H checklist, every item, fresh boot 1440×860, and the close-out
   report against it.

## 5a. Addendum (owner feedback, same day): the smoke no longer touches the desktop

The first delivery inherited 17G's `CopyFromScreen` capture, which needs the window
visible and unobstructed — so the smoke set the window always-on-top and stole focus
per shot, and it blocked the owner's view while they analysed regression reports (it
also cost three interrupted runs). That was never an IPC need: every check reads host
state over the invoke path and `smokeKey` dispatches in-page events. Capture now uses
`PrintWindow` + `PW_RENDERFULLCONTENT` (DWM renders the window's own surface
regardless of z-order); `set_always_on_top` and every `set_focus` are deleted, and a
minimized window is restored only in that one case. The smoke runs green fully
occluded, and the captures are cleaner (no desktop bleed at the frame).

## 6. Session discipline notes

Hooks were installed; staging was narrow throughout; the soak logger's
`goals/reports/_soak-p0-03-01.log` rows were left alone. `loose-ends.tsv` was edited
one row (LE-59 state/closed_in) and validated with `check-spine-files` before the next
tool call. No concurrent commits arrived mid-session.
