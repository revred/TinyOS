# Ti64 Interface V1 — Strategy & Build Specification

Status: Proposed → becomes **binding** on owner sign-off
Supersedes: `SPEC.md` where they disagree (SPEC.md remains valid for §5 honesty rule,
§6 render budget, §7 keyboard contract, §8 TDD stories — this document extends them).
Reference implementation (pixel truth): `work/tinyos-console-ux/v3-console.html` +
`console.css` + `console-core.js` + `console-v2.js` + `console-bodies.js`.

---

## Part A — Strategy

### A1. The decision

The design in `v3-console.html` is **V1 of the Ti64 operator interface**. It is one
build with three runtime layouts, a workload-driven sidebar, five tab kinds, and a
host-painted system line. Nothing else is V1. All earlier explorations were deleted;
if you find a mock that disagrees with this document, the mock is wrong.

### A2. Why this design carries V1

1. **It matches the architecture.** The UI is a caller above the ACI gate (Design
   Pillar 2). Every button resolves to a shell verb, an xtask command, or a tab —
   never a new capability. The workload selector mirrors what the OS actually runs.
2. **It is frugal by construction.** No framework, no build step, append-only
   transcript, one 4 Hz clock, ~400 DOM nodes, ≤ 40 KB total. It can run on the
   WebView2 host today and an on-target renderer later without a rewrite.
3. **It cannot overclaim.** Every number on screen is `live`, `pending
   (uncalibrated)`, or `absent (LE-xx)`. A screenshot of this UI is safe evidence.

### A3. Delivery phases

| Phase | Deliverable | Definition of done |
| --- | --- | --- |
| V1.0 | The five `ui/` files in `stage-e-console-app/`, static reference parity | Checklist H passes pixel-for-pixel against the reference build |
| V1.1 | Wired to the Tauri host (`run_line`, `read_tab`, `read_console`) | Browser-side `Session` deleted; same screens, real transcripts |
| V1.2 | Generated metadata (`verbs.json`, `workbench.json` from xtask) | SPEC.md stories UX-05/06 green; no hand-edited tool lists |
| V1.3 | Responsive window + suspended background tabs | SPEC.md stories UX-01/02 green |

### A4. Rules for the implementing agent (read twice)

You may be a smaller model. These rules exist so you cannot fail:

1. **Copy, never re-derive.** The reference files are the truth. When this document
   and your instinct disagree, copy the reference. When this document and the
   reference disagree, copy the reference and file a note.
2. **No inventions.** Do not add a verb, a colour, an icon, a panel, an animation,
   or a "nice touch". V1 is closed. Anything not in this document is out.
3. **No renames.** `tx01` is `tx01`. `Ti64` is `Ti64`. Element ids in Part D are
   load-bearing (scripts and tests query them).
4. **One change, one check.** After each file lands, run the matching checklist
   items in Part H before touching the next file.
5. **If a checklist item fails, stop and fix it.** Do not continue past a red item;
   later work will hide it.

---

## Part B — Identity and naming (exact strings)

| Thing | String | Never |
| --- | --- | --- |
| Product short form in chrome | `Ti-OS` | TinyOS (chrome), TINY-OS |
| Master-menu button | `Ti64` (with droplet icon) | Start, Menu, TinyOS |
| OS workload in the selector | `Ti64 · Workload` | BUILD, OS, System |
| Tab names | `tx01` … `tx99` (lowercase, zero-padded 2) | Tab-1, TAB-1, x01, T1 |
| Session identity everywhere | same `tx` name as the tab | any second id |
| Shell output (VER banner, DIR, golden text) | **unchanged** `TinyOS …` | do not "fix" shell output to Ti-OS — it is the Rust core's real output |

---

## Part C — Visual contract

### C1. Palette — Windows Terminal "Campbell", verbatim

Copied from `external/WindowsTerminal/src/cascadia/TerminalSettingsModel/defaults.json`
into `:root` of `console.css`. **No raw hex anywhere else.** Semantic tokens only:

```
--t-prompt #16C60C   --t-echo #CCCCCC   --t-out #F2F2F2   --t-meta #767676
--t-head #61D6D6     --t-warn #F9F1A5   --t-err #E74856   --t-deny #C50F1F
--t-pass #16C60C     --t-fail #E74856   --t-pending #C19C00 --t-absent #767676
--t-host #F9F1A5     surfaces: --s-0 #0C0C0C · --s-1 #121212 · --s-2 #1A1A1A ·
--s-3 #222222 · --line #2A2A2A · --line-hot #3B78FF
```

Font: `"Cascadia Mono", Consolas, monospace`, 14 px / 1.45. No webfont download.

### C2. Transcript colouriser (the "CMD colour plugin")

`console-core.js :: paintLine()` — copy it whole. Per emitted line, ONE
DocumentFragment of ≤ 8 spans, appended, never re-rendered. Rules it encodes:
prompt green / typed verb bright-yellow / args white; `<DIR>` rows blue+yellow;
file rows white name + class-coloured extension (`.TXT` cyan, `.TCB`/`.SYS`
bright-green bold, `.CFG` purple); numbers tabular yellow; dates/labels grey;
`origin=`/`trust=` values cyan; env `KEY=VALUE` cyan/white; task rows cyan name +
green/yellow state; errors `--t-err`; denials `--t-deny` bold; host commentary
`--t-meta`. Input text `--c-byellow`, caret `--c-bgreen`.

### C3. Icons (exact glyphs)

Tab kinds: `⊞` MS-DOS · `❯` Linux · `⏱` RT-OS · `◈` agent · `✓` parity.
Tools: `⚒ ✓ ▤ ▥ ◫ ⏱ ⚿ ☰ ⇄ ⌁ ⚙ ☡ ⚑` per `console-core.js WORKBENCH[].ic`.
Workload mark: the **dumbbell SVG** (constant `KETTLEBELL` in the reference —
keep the constant name, it is queried). Ti64 button icon: the **oxidised-titanium
droplet SVG** (gradient id `tiweld`: silver → straw → bronze → purple → blue →
cyan, rim `tirim`, sheen `tisheen`). Both SVGs: copy verbatim from the reference,
single-quoted attributes where embedded in JS strings.

**Icon axis rule:** every glyph in the left column (corner, rail rows, dock
toggle) centres on one vertical line: a 24 px cell starting 12 px in when
expanded; centred in the 46 px stub when compressed (rail rows compensate their
2 px selection border with `margin-left:-2px`).

---

## Part D — Region stack and geometry

Top to bottom (ids are mandatory):

```
#topbar   grid: var(--side-w,232px) 4px 1fr        height 34px
  ├ #corner      workload selector (col 1)
  └ .tabs #tabs  tab strip + ⇄ switcher chip (col 3)
#body     grid: same columns · rows minmax(0,1fr) · overflow hidden
  ├ #side        workload rail (col 1; width 100%)
  ├ #grip        4px col-resize separator (col 2; keyboard: Alt+←/→, clamp 150–420)
  ├ #main        session panes (col 3; grid, min-height 0)
  └ #gui         GUI desktop pane (col 3; shown when body[data-gui="1"], #main hidden)
#dockrow  flex                                       ~46px
  ├ #sidetoggle  rail compress/expand, width = rail column (46px compressed)
  └ #dock        six meters (host cpu · static pool · regression · parity ·
                 denials · page-faults), .meter padding 3px 10px 4px
.inputrow prompt + <input id="line">                 ~37px
.reserved SYSTEM LINE (last line, unlabelled)        28px, padding-left 0
  ├ #start   Ti64 button: min-width 92px, height 24px, margin-left 2px, gold
  │          gradient pill, border --c-yellow, inverts when open
  ├ #reserved status text (identity · layout · tabs · parity · denials)
  ├ #budget  "N nodes · X ms · 4 Hz · N tab(s)"
  └ #layoutpick  Focus | Cockpit | Split (far right)
```

Compressed state `body[data-side="0"]`: both grids' first column becomes 46 px
(`--side-w` untouched; the collapsed rule overrides the track). The grip gets
`pointer-events:none`. `#side` stays visible as the icon rail — never
`display:none`.

Height rules: no per-pane header row in Focus or Cockpit session panes (the tab
strip + system line already carry identity); Split keeps headers on both cells
(`tx01 · MS-DOS` + focused/pinned) because two panes must be tellable apart.

---

## Part E — Behaviour contracts

### E1. Workload selector (`#corner` + `#benchlist`)

- Reads `[dumbbell] {workload.n} ▾`; `Ti64 · Workload` when `view === "os"`.
- Click / `Ctrl+Shift+W` opens the list: 5 entries (`os`, `dev`, `agent`, `rt`,
  `search`), each `glyph · name+description · state`, state coloured
  running/dormant/absent. OS entry uses the dumbbell.
- Choosing sets `view`, resets `drill = null`, repaints the rail. Esc closes,
  outside-click closes, arrows rove, Enter picks.
- Data: the `WORKLOADS` array (reference lines — copy whole). Schema:
  `{id, n, ic, state: running|dormant|absent, d, tools: Tool[], groups: string[]}`
  where `groups` pulls shared tools from `TinyOS.WORKBENCH` by its `g` field.

### E2. Rail drill-down (`#side`)

State: `drill` (null | Tool). Two levels only:

- **Level 0** — the workload's tools: `wl.tools ++ WORKBENCH[g ∈ wl.groups]`.
  Row = 24px icon + name over value (two-line grid). Rows with `act` (NEW …
  SESSION/TAB) execute immediately. All other rows set `drill = tool`.
- **Level 1** — the tool is the top-level menu: `←` back row (name + workload,
  surface --s-2), then a `.detail2` block (description + evidence line coloured
  by state), then the option tree from `drillOptions()`: `run VERB` if the tool
  has a verb, entries from `SUBS[tool.t]`, always ending with `note to session`.
  Option kinds: `verb` → `session.run(verb)`; `line` → meta
  `"$ cargo run -p xtask -- …  (runs on the host — xtask, not a shell verb)"`;
  `note` → meta text; `act` → call it.
- Compressed: same two levels, icons only, `.detail2` hidden; clicking a rail
  icon expands AND drills. Tooltip = `name — value\ndetail`.

### E3. Tabs (`#tabs`)

`[kind-glyph] txNN` and nothing else; tooltip `txNN · KIND · Ctrl+N`. Numbering
from a monotonic `tabSeq` (never reused). No `+` buttons in the strip — new tabs
come from the owning workload's rail or the master menu. Switching: click,
`Ctrl+Tab`, `Ctrl+1…6`, `Ctrl+Space` switcher (lists `Ctrl+N txNN · KIND ·
lines`). `Ctrl+T` duplicates the focused kind. Boot opens `dos`, `rt`, `agent`
and focuses tx01, which pre-runs `VER · DIR · SET · TASKMGR · TASKKILL RT-CTRL ·
TYPE README.TXT` so colour semantics are visible immediately.

### E4. Tab kinds (bodies in `console-bodies.js` — copy whole)

| kind | body | context column (Cockpit) |
| --- | --- | --- |
| dos/posix | transcript `.screen` | SESSION & EVIDENCE: identity/flavour/cwd/volume/denials · authority ledger · goals spine |
| rt | mode row (AUTO/MDI/JOG/HANDLE/EDIT/REF) · X Y Z A C DRO (machine grey-white, work green, 3 decimals) · jog ± buttons · overrides sliders | MACHINE STATE: deadline monitor (mechanism-only caveat, ADR 0005/LE-09/LE-27) · interlocks · E-STOP out-of-band panel · WCI authority lease (holder = the RT tab's own tx name) · alarms |
| agent | proposal cards: `cap(args)` + risk chip (read/actuates/destructive) + why + Approve/Deny + token cost; destructive shows "refused at the grant table" | BUDGET & ADMISSION: granted/spent/per-turn + bar · admission (VRAM, rate, verdict) · UMM/mmap notes · Phase-6-not-built caveat |
| parity | three-signal wall, `F8` runs (all `no signal yet` → `running…` → green/PASS) | MACHINE STATE |

RT prompt grammar (intercepted before the DOS core): `MODE <m>`, `JOG <axis>
<delta>`, `OVR <feed|rapid|spindle> <pct>`, `DRO`, `ALARM`, `ESTOP` (prints the
out-of-band statement — never actuates). A jog outside JOG/HANDLE **must** produce
a red denial line AND a spoor entry. Agent prompt: every input creates a pending
proposal; nothing executes until Approve.

### E5. Layouts (`body[data-layout]`)

`focus` (one pane; boots with rail compressed) · `cockpit` (pane + 330 px context
column, default) · `split` (two panes; `Ctrl+\` pins the next tab as secondary;
pinned tab shows `▹` in the strip; empty second cell invites "pin a second tab —
Ctrl+\"). Picker lives at the far right of the system line; `Ctrl+Shift+1/2/3`;
`?layout=` query param seeds it.

### E6. Master menu (`#start` → `#menu`)

Opens with `Ctrl+Esc` or the Ti64 button; anchored above the system line, gold
border, vertical `Ti-OS 0.2` banner, category column (Programs · Workload tools ·
Documents · Settings · Help), item list (icon + label + description), Find box
filtering across all categories. Keyboard: focus lands in Find; `↑↓` move, `→`
into items, Enter opens, Esc closes. Every entry resolves to an existing tab,
verb, or note. `Shut down…` prints the safe-hold refusal.

### E7. GUI flavour (`F12`)

Swaps `#main` for `#gui` (tile grid over the same WORKBENCH data + RT/Agent/
TINYCMD tiles, 44 px hit targets, arrow-grid roving). **Everything else stays**:
topbar, rail, dock, prompt, system line. Esc or F12 returns.

### E8. Honesty rules (hard gate)

- Legal shell verbs are exactly `console-core.js VERBS` (mirrors
  `os/src/shell/src/dos.rs`). A control may name a verb ONLY from that table —
  `HELP`, `PARITY`, `CPU` are known illegal examples that were removed; do not
  reintroduce them.
- Every tool/meter/workload carries `live | pending | absent` and its colour;
  `pending` may animate but never prints a quotable number; `absent` names its
  loose end (`LE-53`, `Non-Negotiable 12`) instead of a value and is never hidden.

---

## Part F — Data & host wiring (V1.1)

The browser-side `Session` class exists only so the UX runs without a host. In
`stage-e-console-app` replace exactly these seams, nothing else:

| Reference call | Tauri call |
| --- | --- |
| `new Session(...)` per tab | `invoke("open_tab", {kind})` + registry |
| `session.run(line)` | `invoke("run_line", {line})` (tab identity = webview label) |
| 4 Hz tick reading `session.lines` | `invoke("read_tab")` → append `lines_since(cursor)` |
| tab list / focus | `invoke("read_console")`, `invoke("focus_tab")` |
| system line text | painted by `main.rs::reconcile` ONLY (contract unchanged: no verbs, label in no grant table) |

The RT and Agent models (`console-v2.js`) remain display models until Phases 3/6
provide real state; their caveat lines must remain on screen until then.

---

## Part G — File map and work order

```
ui/console.css        1. copy from reference, unchanged
ui/console-core.js    2. copy; delete nothing yet (Session powers the demo)
ui/console-v2.js      3. copy
ui/console-bodies.js  4. copy
ui/index.html         5. from v3-console.html: whole file is the chrome in V1.0
ui/reserved.html      6. keep contract; restyle to .reserved tokens, no badge
```

Then the TDD stories in SPEC.md §8, in order: UX-01 (responsive window), UX-02
(suspended tabs), UX-03 (append-only `lines_since`), UX-04 (this chrome under the
smoke hooks, incl. `window.smokeKey`), UX-05 (`verbs.json` generated from
`VerbKind::ALL`), UX-06 (`workbench.json` generated from `goals/assurance/*.tsv`).

---

## Part H — Acceptance checklist (run every item; all must pass)

Boot (cockpit, 1440×860):
1. Corner reads `[dumbbell] Ti64 · Workloa… ▾`; rail lists NEW MS-DOS SESSION,
   NEW LINUX SESSION, then CPU/MEMORY/PAGE-FAULTS/SPEED/VULNERABILITIES/TASK/IPC/
   NETWORK/WORKLOAD MONITORS/RISK/MODES ROWS with state dots and values.
2. Tabs read `⊞ tx01  ⏱ tx02  ◈ tx03`; only the `⇄` chip besides them.
3. Transcript shows coloured VER/DIR/SET/TASKMGR output; the TASKKILL RT-CTRL
   denial is deep red and appears in the evidence column's authority ledger.
4. System line: Ti64 pill 2 px from the left edge; text `tx01 MS-DOS · Cockpit —
   3 tab(s) — parity: not run — authority denials: 1`; `nodes · ms · 4 Hz`
   readout; Focus/Cockpit/Split at the far right, Cockpit pressed.
5. No scrollbar on `document.body` (scrollHeight == clientHeight).

Interactions:
6. `Ctrl+B` → 46 px icon rail, one icon axis with corner + dock toggle; click a
   rail icon → expands with that tool drilled (← back row on top).
7. MEMORY USAGE drill → `run MEM` prints the pool map; DEVELOPER TOOLS drill →
   four `$ cargo run -p xtask -- …` meta lines, no `Bad command or file name`
   anywhere in the whole UI.
8. Workload switch to Agent → rail repaints to 4 agent tools; corner shows `◈
   Agent`; switch back → `Ti64 · Workload`.
9. RT tab: `MODE AUTO` then `JOG X +0.1` → red denial + spoor; `MODE JOG` then
   jog → X moves 0.100; E-STOP panel states it cannot be driven from here.
10. Agent tab: type a prompt → pending proposal; Approve charges tokens to the
    budget bar; the `storage.format` card reads "refused at the grant table".
11. `F8` in parity tab → three signals go green, OVERALL: PASS; system line
    parity state follows.
12. `F12` → tile desktop replaces only the session pane; corner, rail, tabs,
    dock, prompt, system line all still visible; Esc returns.
13. `Ctrl+Esc` → master menu with banner, 5 categories, Find filters across all;
    Shut down… prints the refusal.
14. `Ctrl+Space`, `Ctrl+Tab`, `Ctrl+1..3`, `Ctrl+\` (split + `▹`), `Alt+←/→`,
    `F1` map — all work; every interactive element reachable by keyboard alone
    with a visible focus ring.

Budget:
15. ≤ 450 DOM nodes at boot; paint tick < 1 ms shown; timers stop when
    `document.hidden`; total payload ≤ 40 KB uncompressed.

## Part I — Non-claims (unchanged)

No `PD-*`/`TG-P*`/timing evidence; GUI flavour is not a compositor (`LE-53`
stands); the CPU meter is a shape, not a measurement; Windows Terminal is a
palette reference only — no code copied, linked, or built.
