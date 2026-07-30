# TinyOS operator console — UX specification (17G successor)

Status: Proposed
Scope: `external/tauri/tinyos-poc/stage-e-console-app/ui/` and the reconciler in
`stage-e-console-app/src/main.rs`. No `os/src/**` change is required by this document,
and none is permitted by it — the UX is a presentation layer above the ACI gate
(Design Pillar 2). Anything here that would need a shell-crate change is called out
explicitly and deferred to a Story.

This file is written to be executed by a coding agent (Claude CLI or equivalent) without
further design input. Every acceptance criterion is observable; nothing is aesthetic-only.

---

## 1. What this replaces

The 17G console (`ui/index.html`, `ui/tab.html`, `ui/reserved.html`) is correct in
architecture and thin in ergonomics: three unrelated stylesheets, a hard-coded 1280×800
non-resizable window, a 250 ms poll per webview that repaints the whole transcript by
`textContent` assignment, no keyboard grammar beyond the caret, and no place for the
developer-workbench tooling the project actually has.

What is kept, unchanged, and must not regress:

| Kept | Where it lives today |
| --- | --- |
| Host-owned reserved region, no verbs, Rust-only repaint | `reserved.html`, `main.rs::reconcile` |
| Two disjoint grant tables (chrome verbs vs tab verbs) | `console-manifest.json`, `commands.rs` |
| Session identity = invoking webview's runtime label | `commands.rs` |
| Per-tab `World` (env, cwd, volume, policy) | `stage-e-console::tabs` |
| Three-signal parity rule rendered separately | `parity_suite.rs`, `tab.html` |
| Denials visible, never swallowed | `authority.rs` → `read_console` |

---

## 2. Files to write

Drop these into `stage-e-console-app/ui/`. The reference implementation of every one of
them is in `work/tinyos-console-ux/` in this repository — copy, do not re-derive.

```
ui/console.css        shared visual layer: Campbell palette + semantic tokens + primitives
ui/console-core.js    transcript renderer, roving-tabindex helper, meter mounts, clock
ui/console-v2.js      RT operator-panel model + agent proposal/budget model (§10)
ui/console-bodies.js  one body + one context column per tab kind, layout-agnostic
ui/index.html         chrome: workload selector + rail + tab strip  (chrome grant table)
ui/tab.html           session body per kind: shell · rt · agent · parity (tab grant table)
ui/reserved.html      unchanged contract; mounted BELOW the prompt, unlabelled (§10.1)
```

The delivery reference is `work/tinyos-console-ux/v3-console.html` (three layouts in one
build, `?layout=focus|cockpit|split`). It is the **only** reference: the turn-1 options and
the v2 build have been deleted rather than left to rot, because a stale mock in a repo gets
implemented by someone eventually.

`console-core.js`'s `Session` class in the reference files is a browser-side stand-in so
the UX can be reviewed without a Tauri host. **In the repo build it is deleted**; each
`session.run(line)` becomes `invoke("run_line", { line })` and each 250 ms tick becomes
the existing `invoke("read_tab")` / `invoke("read_console")`. No new verb is introduced by
this specification. The verb table in `console-core.js` (`VERBS`) is display metadata only
and must be regenerated from `shell::verbs::VerbKind::ALL` rather than hand-maintained —
see §8, `STORY-UX-05`.

---

## 3. Colour and type

Palette is Windows Terminal **Campbell**, copied verbatim from
`external/WindowsTerminal/src/cascadia/TerminalSettingsModel/defaults.json` into
`:root` in `console.css`. Font is Cascadia Mono, falling back to Consolas.

No option file, and no future screen, may use a raw hex. Only the semantic tokens:

| Token | Campbell source | Means |
| --- | --- | --- |
| `--t-prompt` | brightGreen `#16C60C` | the prompt, and only the prompt |
| `--t-echo` | white `#CCCCCC` | the line the operator typed, echoed back |
| `--t-out` | brightWhite `#F2F2F2` | verb output |
| `--t-meta` | brightBlack `#767676` | host commentary, batch echo, hints |
| `--t-head` | brightCyan `#61D6D6` | section and column headings |
| `--t-warn` | brightYellow `#F9F1A5` | degraded but not failed |
| `--t-err` | brightRed `#E74856` | `Bad command or file name`, file not found |
| `--t-deny` | red `#C50F1F` | authority denial — a distinct colour from an error |
| `--t-pass` / `--t-fail` | brightGreen / brightRed | test and signal verdicts |
| `--t-pending` | yellow `#C19C00` | instrumented, not yet calibrated |
| `--t-absent` | brightBlack | no evidence exists — see §5 |
| `--t-host` | brightYellow | the host-owned reserved region, nothing else |

Rationale for the split between `--t-err` and `--t-deny`: an error is the shell answering;
a denial is the policy engine answering. Non-Negotiable 3 means the operator must be able
to tell those apart at a glance without reading the words.

Unicode and non-Latin text are carried by the font stack and `white-space: pre-wrap`; no
per-glyph handling is added. Emoji are not used.

---

## 4. The five options

| id | Name | Interaction spine | Best for |
| --- | --- | --- | --- |
| A | Workbench Rail | function keys (CUA); rail row = launcher (Enter) *and* palette injection (Space) | OS developer at a workbench |
| B | Frugal purist | `Ctrl+K` palette over verbs + tools + flavours; two chrome lines total | lowest cost per pixel; RT/field use |
| C | Two panes | Alt-mnemonic menu bar; `Tab` cycles panes; tree + detail | learning the tool surface; drill-down |
| D | Mode-switched | `Alt+1/2/3` re-weights one layout: developer / operator / auditor | all three audiences in one binary |
| E | GUI flavour | tiles, 44 px targets, arrow-grid; tabs unmount | touch/kiosk, thin-client, demos |

They are not exclusive. A and B compose (rail hidden → B); D is the mode wrapper any of
A/B/C can live inside; E is a flavour of the same session set, not a second application.
The recommended landing is **D as the shell, A as its developer mode, B as its operator
mode, E behind the GUI flavour** — but that decision is the owner's, not this document's.

---

## 5. Honesty rule for every number on screen

Each workbench entry and meter carries exactly one state, and the state decides the
colour, so a screenshot cannot overclaim:

- `live` — evidence exists in this repository today (e.g. `MEM`'s static pool map,
  `shell` 22/22, `xtask` 204/204, the three parity signals, the denial count).
- `pending` — instrumented and landing now; shown in yellow, labelled
  `metering (uncalibrated)`. CPU and speed measures sit here. A `pending` meter may
  animate, but **may never print a number that could be quoted as a bound** — `ADR 0005`
  says worst-case bounds are quotable only from a qualified platform, and
  `goals/assurance/qualified-platforms.tsv` currently qualifies none.
- `absent` — no evidence; the row states the loose end instead of a value
  (`page-faults → no evidence yet (LE-53)`, `network → not linked in (Non-Negotiable 12)`).

An `absent` row is never hidden. Hiding it would make the console look more finished than
the project is, which is the failure mode `README.md`'s corrected "Default supported set"
section exists to warn about.

---

## 6. Render budget — acceptance thresholds

The console must cost less than the thing it observes. These are testable, not aspirational.

1. **Append-only transcript.** One `<span class="l l-*">` per emitted line, appended.
   Never re-render the whole buffer (today's `screen.textContent = snapshot.transcript`
   re-lays out the entire scrollback every 250 ms). Verified by: after 1 000 emitted
   lines, a new line's append costs < 1 ms and produces exactly one added node.
2. **Ring-buffered scrollback.** Cap at 5 000 lines per session; drop from the head.
   Memory per tab is then bounded and provable.
3. **One clock for the document, 4 Hz max**, paused on `document.hidden`. No `rAF` loop,
   no per-tab timer. Unfocused tabs must be *suspended*, not merely hidden — this closes
   the stated debt in `session/hand-2026-07-30/01A` §2.
4. **No layout thrash on keystroke.** The prompt input is a plain `<input>`; nothing
   recomputes on `input`, only on `Enter`.
5. **`contain: content`** on the transcript so scrollback growth cannot invalidate the
   chrome's layout.
6. **No dependencies.** No framework, no bundler, no web font download; total added
   payload ≤ 32 KB uncompressed across all five files. Current reference: css 9.4 KB,
   core 23 KB.
7. **Visible budget.** Every option prints `nodes N · paint X ms` in its status line.
   This is not decoration: a regression shows up in a screenshot.

Suggested CI hook (a follow-on Story, not this one): the smoke run already captures
`smoke.json` — add `ui_nodes` and `ui_paint_ms` and fail the run above thresholds.

---

## 7. Keyboard contract

Every option must satisfy: **no pointer is required to reach any function.**

Global, all options:

| Key | Action |
| --- | --- |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | next / previous tab |
| `Ctrl+1…6` | tab by ordinal (matches the six enumerated tab identities) |
| `Ctrl+T` | new tab in the current flavour |
| `Esc` | close any overlay; never closes a session |
| `↑` / `↓` at the prompt | command history for that session only |
| `F1` | keyboard map overlay |

Per option: A adds `F2` rail, `F3` new tab, `F5` run focused tool, `F6` pane cycle.
B adds `Ctrl+K` palette. C adds `Alt+F/S/W/T/H` menus and `F9` swap panes.
D adds `Alt+1/2/3` mode and `F8` run parity suite. E uses arrows + `Enter` + `Esc`.

Focus rules: one `Tab` stop per group with a **roving `tabindex`** inside it (implemented
once in `console-core.js::rove`), `aria-selected` on tabs, `aria-current` on the focused
workbench row, `role="status"` + `aria-live="polite"` on the reserved region so a screen
reader announces authority changes. A visible focus ring is mandatory
(`:focus-visible { outline: 1px solid var(--line-hot) }`), including on the transcript.

---

## 8. Work order (TDD, repo grammar)

Written so each item is red before it is green, per `agent/CODING_STANDARDS.md`.

- **`STORY-UX-01` — responsive window.** Remove `resizable(false)` and the `WIN_W/WIN_H`
  constants from `main.rs`; the reconciler positions the three regions from the window's
  current inner size on `WindowEvent::Resized`. Red first: an e2e test that resizes the
  mock window and asserts the tab webview's height equals `h - reserved - chrome`.
  Closes the "fixed-size window" debt.
- **`STORY-UX-02` — suspended background tabs.** Reconciler calls the hide path *and*
  stops the tab's poll (page-side, via `document.hidden`). Red first: assert no
  `read_tab` invocation arrives from an unfocused label within 2 s.
- **`STORY-UX-03` — append-only transcript.** `read_tab` returns `lines_since(cursor)`
  instead of the whole transcript; the page appends. Red first: a test asserting the
  second poll of an unchanged session returns zero lines.
- **`STORY-UX-04` — the chosen option's chrome.** `index.html` rewritten against
  `console.css`; keyboard contract §7 under test via the existing smoke hooks
  (add `window.smokeKey(name)` so the unattended run drives the grammar, not the mouse).
- **`STORY-UX-05` — generated verb metadata.** `xtask` emits `ui/verbs.json` from
  `shell::verbs::VerbKind::ALL` + the manifest's two grant tables; the palette and the
  workbench read it. Red first: a test that fails when a `VerbKind` exists with no
  `verbs.json` entry. This is what keeps the UX honest as the verb set grows.
- **`STORY-UX-06` — workbench state source.** A single `ui/workbench.json`, generated by
  `xtask` from `goals/assurance/*.tsv` + `goals/performance/catalogue.tsv`, carrying each
  tool's `live | pending | absent` state and its loose-end id. Red first: a test that
  fails if any entry claims `live` without a Report id.

`STORY-UX-05` and `-06` are what make this design *organically upgradable*: new verbs and
newly-earned evidence appear in the console because the generator saw them, not because
someone remembered to edit HTML.

---

## 10. v2 — the delivered shape

Turn-1 options A–E were an exploration. What ships is one console:

### 10.1 Region order — settled, not a layout choice

`[corner selector | tab strip] → [workbench | session] → meter dock → prompt → system line`.

The host-painted line is the **last line of the window, under the prompt, in every layout
and every option**, and it carries **no label about itself**. Its contract is untouched: it
holds no verbs, its webview label is enumerated in no grant table, and `main.rs::reconcile`
is its only writer. Announcing "HOST-OWNED" on screen spent a badge's worth of pixels on a
property the operator cannot act on; the guarantee lives in the manifest and the resolver,
where it is enforced, not in a caption. Reading order now matches causality — you type on
the second-to-last line and the authority consequence lands on the last one.

### 10.1a What varies, and what does not

Fixed by owner preference (no option may re-open these): system-line placement and
labelling; the top-left corner as the workbench selector, over a collapsible, resizable
panel; per-tab
flavours; three ways to switch tabs; the meter dock visible while typing; GUI flavour on
`F12`.

The one remaining axis is **how much of the machine is visible while you type**, expressed
as three layouts of one build, switchable at runtime (`Ctrl+Shift+1/2/3`, or
`?layout=focus|cockpit|split`):

| Layout | Main region | Chosen when |
| --- | --- | --- |
| `focus` | one session pane, workbench beside it | headless edge node; minimum resident UI |
| `cockpit` | body + context column that follows the tab kind | supervising one machine or one model |
| `split` | two tab bodies at once (`Ctrl+\` pins the second) | DRO beside the proposal gate; shell beside the machine |

Because a layout only arranges bodies — it never decides what a body says — adding a tab
kind later costs one renderer in `console-bodies.js` and no layout work.

### 10.2 The workload selector — the top-left corner

**The OS runs workloads as services, and the top-left corner names the one you are in.**
It sits in the tab strip's row, aligned to the rail's column, and reads
`◍ Ti64 · Workload ▾` when the OS is the only workload resident. Clicking it drops the
running-workload list; choosing one repaints the whole rail with that workload's tools.

| Workload | State today | Rail carries |
| --- | --- | --- |
| `Ti64` — the OS itself | running, never stopped | new sessions, runtime meters, safety, link |
| `Developer` | running | dev tools, regression, speed, new parity tab |
| `Agent` | dormant | agent tab, token budget, ACI grant table, runtime (absent) |
| `RT control` | dormant | RT tab, motion, process-sync, deadline monitor |
| `System search` | absent | index status, volume search — nothing built |

Each entry shows its state in the colour the honesty rule (§5) assigns: running green,
dormant yellow, absent grey. A workload that is not built says so in the list rather than
being hidden, so the selector doubles as the answer to "what is this machine actually doing
right now".

This is why the selector is not a "view" or a "bench": those are ways of looking, and this
is a statement about what the OS is running. A tool appears in the rail only if its
workload is selected — you cannot reach RT tooling while standing in the agent workload,
which is the separation the ACI enforces underneath.

`Ctrl+B` compresses the rail into a full-height **46 px icon strip** — every tool still
present as its glyph, name/value/detail on hover. The navigation is a **drill-down**, and it
is the same at both widths:

1. **Level 0 — the workload's tools.** Icon on the left, described on the right (name +
   live value). Command rows (`NEW MS-DOS SESSION`…) act immediately.
2. **Level 1 — the tool becomes the top-level menu.** Clicking a tool row (or its rail
   icon, which also expands) replaces the sidebar with that sub-workload: a `←` header row
   naming the tool (click returns to level 0), its description and evidence state, then its
   **option tree** — `run MEM`, per-fixture xtask commands, the denial journal, each option
   again an icon described on the right. Host-side commands print as `$ cargo run -p xtask
   -- …` meta lines, explicitly marked as xtask and not shell verbs.

So the hierarchy reads workload → tool → options, and the sidebar always shows exactly one
level with the way back on top, like a VS Code view container. The panel carries **no title of its own**:
the corner already names the bench, and printing it twice was the one repeated string on
the screen.
`Ctrl+Shift+W` opens the workload list; the grip drags and `Alt+←/→` resizes from
the keyboard. Default 232 px, clamped 150–420 px. Both rows share one column ruler, so the
corner and the panel can never disagree about where the left column ends.

### 10.3 Tab toggling — explicit, three ways

Numbered `^1…^6` badges in the strip (matching the six enumerated tab identities),
`Ctrl+Tab` to cycle, and `Ctrl+Space` for a switcher listing each tab's session id, kind
and line count. `Ctrl+T` opens a tab of the focused tab's kind.

### 10.3 Tab strip — iconographic, and it is not the launcher

A tab is `[icon] x01` and nothing more: the kind's glyph (`⊞` MS-DOS, `❯` Linux, `⏱` RT-OS,
`◈` agent, `✓` parity) plus its ordinal, with session id and shortcut in the tooltip. The
strip's job is switching, not describing — the identity in full already lives on the system
line, and repeating it in every tab was noise that grew with the tab count.

**New sessions are opened from the bench that owns them, never from the strip:** MS-DOS,
Linux and parity tabs from the developer-tools bench (`BUILD`); the RT tab from the RT
bench; the agent tab from the agent bench. One consequence worth stating — opening a tab
now costs a bench visit, which is correct: a tab kind you have no bench for is a tab kind
you have no tooling for.

Switching: `Ctrl+Tab`, `Ctrl+1…6`, or `Ctrl+Space` for the switcher (the `⇄` chip).

### 10.3a Layout picker lives on the system line

Focus / Cockpit / Split sit at the **far right of the system line**, opposite the master
menu at the far left. Posture and identity therefore share one row, and the tab strip stays
purely a strip of tabs.

### 10.3b Master menu — the non-command-prompt entry point

At the far left of the system line: `◧ TinyOS` (`Ctrl+Esc`). Vertical banner, category
column (Programs · Workbench · Documents · Settings · Help), an entry list with icon,
label and one-line description, and a Find box searching every category. `↑↓` moves, `→`
enters the list, `Enter` opens, `Esc` closes.

It opens no path the ACI gate does not already own — every entry resolves to the same verb
or tab a keyboard user reaches directly. `Shut down…` refuses honestly: a console has no
authority to power the machine down; the RT core reaches safe hold through the watchdog
path.

### 10.3c Universal from here

§10.1, §10.1a, §10.3, §10.3a and §10.3b are the house style: every option presented from
now on uses the unlabelled system line below the prompt with the master menu at its far
left and the layout picker at its far right, iconographic `[icon] x01` tabs, and per-bench
tab creation. An option that re-opens one of these is a mistake, not a variation.

### 10.4 Tab kinds

| Kind | Body | Grounded in |
| --- | --- | --- |
| `dos` / `posix` | transcript + prompt | `shell/src/dos.rs` |
| `rt` | mode selector, DRO, overrides, deadline monitor, interlocks, authority lease, alarms | `docs/physical-ai-reference-workloads.md`, `docs/wci-spec.md`, ADR 0005 |
| `agent` | proposal gate, token budget, admission control, UMM/mmap state | `docs/inference-architecture.md`, Design Pillar 5 |
| `parity` | three-signal wall | `parity_suite.rs` |
| GUI flavour | tile desktop over the same sessions, meter dock retained | 17G `a3` shape |

### 10.5 RT tab — rules

- Mode is an ACI-gated action, not a UI state: a jog issued outside JOG/HANDLE is **denied
  and journaled**, and the denial appears in the transcript and the spoor journal. The
  panel must be able to show itself being refused.
- The DRO is fed by the Tier 0 **simulated** `PositionFeedback` implementation and says so
  on screen. No positional-accuracy claim is made or implied.
- The deadline/jitter panel shows periods and "budget declared", never a measured bound —
  ADR 0005 restricts bounds to a qualified platform, `qualified-platforms.tsv` lists none,
  and LE-27 records that no ARM64 code here has executed.
- E-stop is displayed as **out of band**: the panel states that it cannot be asserted,
  masked or cleared from the console, matching `wci-spec.md`'s guarantee.
- Command authority renders the WCI single-writer lease (holder, heartbeat, transport), so
  "who may command this machine right now" is never ambiguous.

### 10.6 Agent tab — the frugal token extractor

- The model never emits executable text into the console. Each output is rendered as a
  **pre-registered ACI capability call** with its arguments, risk class, token cost and
  rationale, in a pending state until a human approves or denies it.
- A capability outside the agent's grant table is shown as *refused at the grant table* —
  it was never offered, which is the visible form of "no privileged bypass exists".
- Frugality is the tab's subject, not a footnote: granted / spent / per-turn cap, charged
  on approval, so token cost is attributable per action exactly as authority is.
- Admission control (VRAM footprint, submission rate, verdict) is shown as *admission*,
  never as priority — a stalled model degrades through the ACI and cannot touch an RT
  deadline (Non-Negotiable 6).
- Phase 6 is not built; the tab says so. It exists so the runtime cannot land without its
  gate, its budget and its provenance already in place.

### 10.7 Edge-device posture

On a headless edge node the same build runs with the sidebar collapsed and the GUI flavour
never mounted: corner stub + one tab + dock + prompt + system line. That is the frugal
configuration, and it is a *state* of the shipped console, not a second product.

## 9. Non-claims

- Nothing here is evidence for `PD-*`, `TG-P*`, or any timing bound.
- The GUI flavour (E) is a presentation of the same host-side sessions; it is not a
  window manager, not a compositor, and implies no on-target graphics stack (`LE-53`
  stands — this remains the host-side interaction model).
- The animated CPU meter in the reference files is a *shape*, not a measurement; it is
  wired to `--t-pending` and the string `metering (uncalibrated)` for exactly that reason.
- Windows Terminal is used as a palette and legibility reference only. No Windows Terminal
  code is copied, linked, or built (`external/README.md` contract, ADR 0008/0009).
