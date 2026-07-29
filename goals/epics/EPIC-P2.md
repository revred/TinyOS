# EPIC-P2 — The Operator Command Environment: DOS, POSIX and Real-Time, One Authorisation Path

Status: **Specified — no Feature document written and no Story started. Blocked on a storage decision (`LE-48`) that this Epic proposes an answer to but does not take.**
Roadmap phase: **Phase 2 — Shell & UX**, per [`README.md`](../../README.md) and [`backlog.md`](backlog.md).
Introduced in: [`session/hand-2026-07-28/44A-dos-parity-standing.md`](../../session/hand-2026-07-28/44A-dos-parity-standing.md), which registered `LE-48` and asked where DOS parity stood.
Depends on: [`EPIC-P0`](EPIC-P0.md) per `backlog.md`. **Also gates** `EPIC-P3`, `EPIC-H1` and `EPIC-H5`, all of which name `EPIC-P2` as a dependency — this Epic is on the critical path to connectivity, the game-proving path and the browser lab.

## Goal

One command environment that an operator from **three** different worlds can drive without retraining, over **one** canonical verb core with **one** authorisation point.

- **`DOS` flavour** — `DIR`, `COPY`, `DEL`, `/switches`. The MS-DOS 4+ ergonomics `SeedMVP.md` §1 item 2 fixes as founding intent.
- **`POSIX` flavour** — `ls`, `cp`, `rm`, `-flags`, pipes, redirection.
- **`RT` flavour** — **native TinyOS real-time verbs that exist in neither MS-DOS nor vanilla Linux.** This is the flavour that justifies the operating system, and §4 is its own section because it is not a syntax skin over the other two.

**The operator chooses the flavour**; the flavour changes *syntax and vocabulary only*. It never changes what is permitted, who may do it, or what is recorded. That sentence is the whole security architecture and §3 is what enforces it.

**The front end is one window with tabs**, not a bare serial prompt: each tab is an independent session with its own flavour, and **a tab may be switched from a command interface to a GUI UX mode**. §6 is the model and the security problems tabs introduce that a single prompt does not have.

## Goals verified (from `SeedMVP.md` §3)

Per `backlog.md`'s `EPIC-P2` row: **`G-RT-5`** (DOS-familiar *and* POSIX-familiar operator experience over one canonical core), **`G-RT-6`** (plain-text versionable configuration — never a hidden binary registry), **`G-SEC-5`** (origin, signer, trust, entitlement, quarantine and derivation labels survive rename, copy, extraction, conversion, compilation, IPC and storage), **`G-SEC-7`** (downloads begin non-executable in quarantine; complex parsers run in disposable resource-bounded sandboxes).

`G-SEC-5` is the one that constrains the design hardest, and §5 explains why.

## 1. The precondition, stated first

**At least 15 of the 22 specified verbs cannot be implemented, because there is nothing to implement them against.**

`DIR`, `CD`, `COPY`, `MOVE`/`REN`, `DEL`, `MD`, `RD`, `TYPE`, `FIND`, `SORT`, `MORE`, `TREE`, `ATTRIB`, `VOL` and the `.TCB` batch runtime all presuppose a filesystem. **No filesystem crate exists anywhere in [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md)'s 18-crate map** — storage appears once, as a Phase 3 block-device *class driver*, which is not a filesystem. Three source files already record the absence in passing (`exec/src/iat.rs`, `kernel/src/spoor_journal.rs`, `os/src/main.rs`: *"no filesystem yet"*). And the ordering inverts: `shell` is Phase 2, the storage driver is Phase 3.

That is **`LE-48`**, and it is a decision this Epic must not take unilaterally. **What this Epic proposes**, for the owner to accept or reject:

> A **volume abstraction with a RAM-backed volume first** (`FEAT-P2-02`). It unblocks all 15 verbs with no block device, no disk driver and no Phase 3 dependency; it is how the embedded system image already works; and it puts the seam in place so the Phase 3 storage driver becomes a second backing behind an interface that already has tests. A FAT-shaped layout is the right target because DOS semantics (8.3 names, attributes, volume labels) are what the `DOS` flavour must expose, and because it is the format board firmware already reads.

**Two consequences that are not negotiable if that proposal is accepted:** the volume must carry `G-SEC-5` labels from its first commit (§5), and `shell` must be reordered behind it rather than starting in parallel.

## 2. What "DOS parity" can and cannot mean

**Ergonomic parity only. Never binary compatibility.** MS-DOS 4 was 16-bit real-mode and segmented; TinyOS is **64-bit-only by charter**. No DOS `.COM` or `.EXE` will ever execute here. `SeedMVP.md` records the `MsDOS/` submodule as *"a historical command-behavior reference only, not built upon"*, and `README.md` says *"the soul of MS-DOS"*. **No Report, Story or marketing sentence from this Epic may imply a compatibility claim**, and that prohibition is an exit criterion because it is the kind of claim that drifts by accident.

## 3. The security doctrine — features never arrive ahead of it

The mandate is *features with a strong security doctrine in place*, not features followed by hardening. Each rule below is stated so it can become a boundary test rather than a paragraph.

### 3.1 There is no root, and the POSIX flavour must not invent one

**`agent.md` rule 2: no privileged bypass for any caller — not the local shell, not a remote host, not an LLM agent.** So the POSIX flavour imports POSIX *syntax*, not POSIX *authority*:

- **No `sudo`, no `su`, no setuid, no UID 0, no administrator mode, and no escalation verb of any kind.** There is no privilege to escalate *to*; authority is held as ACI capabilities, and a capability an operator was not granted cannot be acquired from inside the shell.
- **Root-style command-line arguments are the specific trap the mandate names.** `-r`, `-f`, `--force`, `--no-preserve-root` and their DOS cousins are **authority amplifiers, not conveniences**: they multiply the blast radius of a single typo or a single injected argument. Each one requires its own capability, distinct from the verb's — holding `delete` does not confer `delete --recursive`.
- **`chmod`/`chown` are already out of scope** per [`docs/cli-compatibility-mvp.md`](../../docs/cli-compatibility-mvp.md): TinyOS has a single capability registry, not a UID/GID/mode triad, and `attrib-view` exposes capability scope instead. That non-goal is a security property and must not be "fixed" later by someone adding a permission model for familiarity.

### 3.2 Three flavours, exactly one authorisation point

**This is why "one canonical core with N front-ends" is a security architecture and not merely DRY.** Two — now three — parsers must never become three policy paths.

- A front-end's only job is to **lower** typed text to a canonical verb plus fully-resolved arguments. It performs **no authorisation, no I/O and no side effects.**
- **Authorisation happens once, on the canonical form**, after resolution and never on the raw string. Authorising a string and then acting on a different interpretation of it is the classic parser-differential vulnerability, and three front-ends is three chances to introduce it.
- **The equivalence test is also the security test.** `SeedMVP.md` already requires golden-file acceptance tests running *"each MVP verb through both DOS and POSIX front-ends against a fixture"* asserting *"equivalent underlying action"*. Extended to three flavours, that test is precisely the proof that no flavour is a privileged path. **Same test, two purposes — and it is a Red clause, not a follow-up.**

### 3.3 Canonicalise, then authorise

Every one of these happens **before** the policy decision, and the decision plus the audit record are written against the canonical form (with the raw input retained alongside, never in place of it):

path resolution and `..` traversal · wildcard and glob expansion · `%VAR%` and `$VAR` expansion · `%1`–`%9` batch parameter substitution · case folding and 8.3 short-name aliasing · redirection targets · device-name interpretation.

**Two named traps, one per heritage, because each is a real historical vulnerability class:**

- **DOS device names.** `CON`, `PRN`, `NUL`, `AUX`, `COM1`–`COM9`, `LPT1`–`LPT9` are pseudo-files in DOS lineage and remain a live vulnerability class on Windows to this day. A path resolver that treats them as ordinary names, or that treats them specially *after* authorising, is wrong. Resolution must be explicit and total.
- **8.3 aliasing and case-insensitivity.** If one object has two names, an authorisation made on one and an action taken on the other is a bypass. **One canonical identity per object**, decided in the volume layer, not the parser.

### 3.4 The parsers are hostile-format parsers

A command line is **external input**: typed by a human, piped from another verb, read from a `.TCB` batch file, or emitted by an LLM through the ACI. `SeedMVP.md` already lists *"TINYCMD's DOS/POSIX front-ends"* under fuzz testing. Therefore each front-end declares hostile inputs and boundary tests in `feature-contracts.tsv`, carries adversarial and fuzz coverage rather than happy-path coverage, and **fails closed on ambiguity — an unparseable or ambiguous command is refused, never guessed.**

### 3.5 Flavour selection must never change authority — and auto-detection is a hazard

[`docs/cli-compatibility-mvp.md`](../../docs/cli-compatibility-mvp.md) offers that *"the shell can auto-detect from the first token's casing/flag style."* **As a security matter that is a hazard and this Epic narrows it.** A misdetected flavour applies the wrong semantics to the same tokens — and `DEL /S x` versus `del -s x` differ in *recursion*, which is blast radius.

**Ruling: flavour is explicit and per-session, defaulting to a configured value. Auto-detection, if implemented at all, is confined to non-destructive verbs and may never be the reason a destructive or RT-mutating verb was accepted.** A destructive verb typed in an unselected flavour is refused with the correct spelling shown, not silently reinterpreted.

### 3.6 Quarantine, labels and active content

`backlog.md` already assigns this to `EPIC-P2` by name: *"`EPIC-P2` must quarantine downloads and sandbox active content."* So: anything arriving from off-device begins **non-executable and quarantined** (`G-SEC-7`); a `.TCB` batch file is **authority-bearing content, not inert data** — an off-device batch file is not executed on the strength of having a filename; and complex parsers run resource-bounded.

### 3.7 The shell holds no direct authority over the real-time core

`README.md`: the shell *"never has direct write access to real-time task state, drivers, or bus I/O."* Every mutation — including `task-kill` and every RT verb in §4 — is an ACI-mediated request that the core may refuse. **Refusal is a normal, tested outcome, not an error path.**

### 3.8 Everything is recorded

Every verb emits a spoor record carrying category, actor, action, outcome and canonical form, on the existing `kernel::spoor_journal` substrate. **An operator action that leaves no trace is a defect**, and that includes refused actions — a denied command is exactly the one worth having a record of.

## 4. The `RT` flavour — the part that is neither DOS nor Linux

The first two flavours buy familiarity. **This one is why the operating system exists**, and it is a distinct verb set rather than a syntax skin, because the objects it names — deadlines, budgets, slack, jitter, inheritance chains, qualification records — have no DOS or POSIX equivalent to be a skin over.

### 4.1 The doctrine that governs every RT verb

> **The prompt is a soft-real-time observer of a hard-real-time system, and it must be structurally incapable of causing a deadline miss.**

Neither DOS nor Linux has this problem, and it is not a quality target — it is the design constraint:

- Every RT verb is **bounded**: no unbounded loop, no allocation, a stated worst case.
- It **never takes a lock a real-time task needs**, and never blocks on one. `LE-22`'s degrade/inheritance composition and `LE-49`'s per-task-versus-per-lock release are live evidence that this area is subtle.
- It runs **below the real-time priority floor**, and its **output is droppable rather than blocking** — a serial write must never stall a deadline. A dropped line is reported as dropped.
- **Observation and mutation are different authority classes.** Reading the task table is cheap and safe. Changing admission, priority or budget at runtime is a **safety action**: it requires its own capability, a schedulability re-check, and a fail-safe refusal.

### 4.2 The verb set, as a proposal for `FEAT-P2-06` to refine

| RT verb | What it does | Why neither DOS nor Linux has it |
| --- | --- | --- |
| `admit` | Offer a task with period, WCET and deadline; **the set is re-checked for schedulability and the admission is refused if it would not hold** | Linux admits and hopes; there is no admission control to fail |
| `deadline` / `slack` | Live per-task slack, time-to-deadline, miss counts | `ps` has no concept of a deadline |
| `wcet` | Budgets, trip counts, degrade state, restart policy | No analogue |
| `jitter` | Per-path latency distribution and tail, not an average | Averages are what mislead; the tail is the product |
| `prio` | Base **and current boosted** priority, plus the lock and holder responsible | The inheritance chain is invisible in both |
| `trace` | Bounded ring of scheduling events, drop-counted | `ftrace` is unbounded and perturbs |
| `budget` | CPU budget accounting per task and per domain | No analogue |
| `spoor` | Query the audit journal | Exists, and no shell surfaces it |
| `tier` | Which timing tier a number came from — Tier 0, hardware, qualified | Nothing analogous exists anywhere |
| `qualify` | The platform's **`Q1`–`Q4` secure-world qualification record** and whether a worst-case bound is quotable at all | `ADR 0005`'s answer, made operator-visible |
| `gate` | Release-gate evidence status for a domain | Nothing analogous |
| `mode` | Show/select flavour — and per §3.5, never an authority change | — |

**`tier` and `qualify` are the two worth defending.** They put `ADR 0005` in front of the operator: a number's provenance and whether it may be promoted to a bound become a question the shell answers, rather than a discipline that lives only in handovers. That is the same prose-versus-register lesson `LE-43` taught, applied to the operator interface.

### 4.3 Time as a first-class shell primitive

`timeout(1)` approximates this in seconds with no guarantee. On TinyOS a command can be **deadline-scoped** — run, and if it has not completed within a stated budget, abort deterministically and report the abort. **The shell can offer a guarantee its Linux counterpart cannot**, and that is worth building deliberately rather than discovering later.

## 5. Why `G-SEC-5` decides the storage design

`G-SEC-5` requires origin, signer, trust, entitlement, quarantine and derivation labels to **survive rename, copy, extraction, conversion, compilation, IPC and storage.** `COPY`, `MOVE`, `REN` and `TYPE`-into-redirection are therefore **label-propagation carriers**, not byte movers.

**FAT has nowhere to put a label.** So the volume layer needs side-band metadata from its first commit, and the choice cannot be deferred:

- **Retrofitting label propagation into an already-written `copy` is how gaps happen** — the one path that forgets is the bypass, and `G-SEC-5` says *every* transformation.
- This is why `FEAT-P2-02` is ordered second rather than last, and why it is a **security** Feature rather than a convenience one.

## 6. The front end: one window, many tabs, one of them a GUI

**TinyOS's front end is a single-window, multi-tab system.** Each tab holds an independent session, and
**a tab can be switched out of the command interface into a GUI UX mode.** `TASKMGR`'s full-screen blue
view is therefore not a separate application — it is a tab.

This is a better fit for the flavour model than a single prompt: §3.5 requires flavour to be **explicit
and per-session**, and a tab *is* the session. An operator can hold a `DOS` tab, a `POSIX` tab and an
`RT` tab open simultaneously, which is exactly the intended experience.

**But tabs introduce three security problems a single prompt does not have, and each one is a
requirement, not a caveat.**

### 6.1 A tab is an authority boundary, not a view

**Capabilities granted to one tab's session do not leak to another.** If they did, "open a new tab"
would be an escalation verb by another name — precisely what §3.1 excludes. Each tab carries its own ACI
session identity, its own flavour, its own audit actor. A tab cannot enumerate, read the input of, or act
on behalf of another tab. **No tab is a parent of another**, and closing one does not transfer anything
to the survivors.

### 6.2 The clipboard is a label-propagation carrier

Copy, paste and drag between tabs are **cross-session transfers of content**. `G-SEC-5` requires origin,
signer, trust, entitlement, quarantine and derivation labels to survive *every* transformation — so the
clipboard is in exactly the same class as `COPY` and `MOVE` (§5), and pasting quarantined content into an
`RT` tab must carry its quarantine with it. **A clipboard that strips labels is a laundering path**, and
it is the one nobody thinks to test.

### 6.3 The trusted path — a GUI tab must not be able to impersonate a prompt

This is the sharpest of the three and it has a known answer. A GUI tab paints pixels; a command tab is
also pixels; therefore **a GUI tab can draw a convincing fake command prompt** and harvest whatever an
operator types into it — including into what looks like an `RT` tab about to `admit` a task. Neither DOS
nor a Linux terminal solves this, and on a machine with real-time authority the payoff is higher.

The requirement:

- **A reserved region of the window that no tab content can ever paint.** It displays which tab has
  focus, which flavour is active, and whether the tab is a command interface or a GUI. Owned by the shell
  host, structurally unreachable by tab content.
- **An unspoofable way to reach a real command tab** — a secure-attention key handled by the host before
  any tab sees input.
- **Input is routed to the focused tab only.** No tab observes another's keystrokes; there is no
  cross-tab input API to get wrong.
- **The active flavour must be unspoofably visible**, because §3.5 already established that a
  misidentified flavour changes blast radius. A GUI tab painting "DOS" while the session is `POSIX` is the
  same defect class as a misfiring auto-detector.

### 6.4 Prior art: Windows Terminal, as a reference and as a counter-example

[`WindowsTerminal/`](../../WindowsTerminal) is a submodule on exactly the terms `MsDOS/` is held: **a
reference for behaviour and structure, never code TinyOS builds on.** It is MIT-licensed, as is `MsDOS/`
and as is TinyOS per [`ADR 0006`](../../docs/adr/0006-mit-licence-confirmed-and-open-core-optionality-dropped.md),
so the reference carries no licensing obligation. It is also C++, and
[`CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md)'s language policy is not negotiable — **nothing is
ported.**

**Worth studying:**

- **The profile model.** A Terminal profile is a named launch configuration a tab is opened against. That
  maps almost exactly onto **per-tab flavour sessions** (§3.5, §6), and it is the strongest single reason
  to look at it: the ergonomics of "new tab, which profile?" are already solved there.
- **Plain-text settings.** Terminal's settings are a versionable text file, which is `G-RT-6`'s
  requirement — *never a hidden binary registry* — reached independently.
- **The text-buffer / renderer separation.** Terminal keeps the buffer distinct from what paints it. That
  is the structure §6.6 needs in order to **drop frames instead of blocking**, and it is worth copying as
  a *shape*.
- Tab and pane management, key-binding dispatch, and the fact that a tab host is a real component rather
  than a decoration.

**Explicitly a counter-example, and this is the more valuable half:**

- **A Terminal tab inherits the user's full access token.** Every tab has the same authority, and opening
  one is free. **§6.1 forbids exactly that** — a tab must be an authority boundary, not a view. Windows
  Terminal is not a security boundary and does not claim to be; TinyOS's tab host must be.
- **There is no trusted path.** Nothing stops tab content painting a convincing fake prompt, because on a
  desktop OS that threat is out of scope. **§6.3 is not inherited from this reference — it is the thing
  the reference does not do**, and it must be designed here.
- **Rendering is best-effort by design.** Perfectly correct for a desktop terminal; §6.5 makes it a
  bounded, preemptible, frame-dropping obligation instead.

**The rule for anyone reading it:** take the *interaction model*, take the *component boundaries*, take the
settings-as-text instinct. **Do not take the authority model — there isn't one.**

### 6.5 The visual target — and the injection surface it opens

The reference look is a segmented status prompt over tabs: user and host, path, a state segment carrying
branch-and-dirty counts, a right-aligned clock, and command output as dotted-leader rows with
pass/partial/fail glyphs and an `n/m` count, closing on a summary block. **It is worth adopting, and
`README.md`'s claim about DOS 4 is exactly why** — *"that era of interface got a lot right for operators
who need clarity under pressure."* A dotted leader and a count are clarity under pressure.

**What to adopt, translated rather than copied:**

- **The segmented status line, carrying RT state instead of version control.** The reference shows
  `branch ↑2 +5`; the TinyOS analogue is the **active flavour**, the tab identity, and live health —
  deadline misses, degrade state, dropped frames. **The prompt becomes an instrument**, which is a better
  use of that space than a branch name.
- **Right-aligned time from the monotonic timebase**, with its **tier** visible (§4.2's `tier`). A
  wall-clock alone on a real-time system is the less useful of the two.
- **Dotted leaders, `n/m` counts, and a summary block.** Legible at a glance and legible in a serial log.

**Three rules that come with it, and each is a requirement:**

1. **Never colour alone.** Glyph **and** word **and** colour. An operator under pressure, a colour-blind
   operator, and a serial capture must all read the same status. Colour is the third signal, never the
   only one.
2. **Degrade to ASCII.** The early console and any serial capture have no font for `✓`/`⚠`. The status
   vocabulary is defined so it renders in both, and the ASCII form is the canonical one for Reports.
3. **Untrusted text must not be able to paint.** *This is the one the pretty prompt introduces.* Filenames,
   volume labels, task names, `G-SEC-5` labels and spoor fields are **attacker-influenced strings that the
   prompt renders**. A filename containing escape sequences could move the cursor, recolour the screen, or
   overwrite the reserved trusted region from §6.3 — the classic terminal-escape injection, and here it
   defeats the trusted path directly. **All untrusted text is rendered inert before display**, and the
   test plants escape sequences in a filename rather than arguing the point.

**Explicitly not adopted: translucency, blur and acrylic.** Per-frame compositing of a blurred backdrop is
unbounded GPU and CPU work on a machine whose whole thesis is bounded work (§6.6). It buys nothing an
operator needs. If it is ever wanted, it is off by default and bounded, never in the reserved region.

### 6.6 The GUI tab is bound by the same real-time doctrine

A repaint is unbounded work. §4.1 therefore applies without exception: the tab host and any GUI mode run
**below the real-time priority floor**, are **preemptible**, and **drop frames rather than block**. A
dropped frame is reported as dropped. **No tab, GUI or otherwise, may cause a deadline miss** — and the
GUI tab is the most likely thing in this Epic to try.

Rendering a *file* in a GUI tab is a parser surface, so `G-SEC-7` applies: active content isolated,
complex parsers in disposable resource-bounded sandboxes, downloads non-executable in quarantine.

## 7. Features, prioritised, and the dependency chain

**No Feature document exists yet.** Ids are reserved here; each document is written when its Feature starts, per `agent.md`'s just-in-time decomposition rule — this Epic deliberately does not pre-build eight stubs.

```text
FEAT-P2-01  Canonical verb core + ACI authorisation seam  ── no front-end, no I/O ──► FIRST, ALWAYS
                     │
                     ├──► FEAT-P2-02  Volume abstraction + G-SEC-5 labels + RAM volume   (answers LE-48)
                     │              │
                     │              ├──► FEAT-P2-04  DOS flavour        ─┐
                     │              ├──► FEAT-P2-05  POSIX flavour      ─┼──► equivalence test across all three
                     │              └──► FEAT-P2-07  .TCB batch runtime  │
                     │                                                   │
                     ├──► FEAT-P2-06  RT flavour  ──────────────────────┘   (needs no filesystem at all)
                     │
                     └──► FEAT-P2-03  Tab host: single window, per-tab session
                                      identity, trusted path, console services
                                          │              (CLS, MORE, TREE need this)
                                          ├──► FEAT-P2-08  TASKMGR, as a tab
                                          └──► FEAT-P2-09  GUI UX mode tab
```

| Priority | Feature | Summary | Depends on |
| --- | --- | --- | --- |
| **1** | `FEAT-P2-01` | Canonical verb core, capability-checked request objects, single ACI authorisation point, spoor emission. **No syntax front-end.** | `EPIC-P0` |
| **2** | `FEAT-P2-02` | Volume abstraction, `G-SEC-5` label carriage, RAM-backed FAT-shaped volume — **the `LE-48` answer** | `-01` |
| **3** | `FEAT-P2-06` | **The `RT` flavour** and its bounded-observer doctrine | `-01` |
| **4** | `FEAT-P2-03` | **Tab host**: single window, many tabs, **per-tab session identity and authority boundary** (§6.1), **the trusted-path reserved region and secure-attention key** (§6.3), label-preserving clipboard (§6.2), plus console services — cursor control, paging, screen model | `-01` |
| **5** | `FEAT-P2-04` | `DOS` flavour front-end — hostile-format parser | `-01`, `-02` |
| **6** | `FEAT-P2-05` | `POSIX` flavour front-end — hostile-format parser, authority-amplifier discipline | `-01`, `-02` |
| **7** | `FEAT-P2-07` | `.TCB` batch runtime — authority-bearing content, quarantine gates | `-04` |
| **8** | `FEAT-P2-08` | `TASKMGR` full-screen live view, **as a tab** rather than a separate application | `-03`, `-06` |
| **9** | `FEAT-P2-09` | **GUI UX mode tab** — a tab switched out of the command interface. Bound by §6.3's trusted path and §6.6's frame-dropping obligation; renders content under `G-SEC-7` | `-03` |

### Ordering rationale

**`-01` is first and it is not negotiable.** If a front-end lands before the authorisation seam exists, it will grow its own policy path, and §3.2 is precisely the architecture that prevents that. One core, one decision point — built in that order or not at all.

**`-02` second, because of `G-SEC-5` rather than because of `DIR`.** Labels must be designed into the volume, not retrofitted through three flavours' file verbs (§5).

**`-06`, the `RT` flavour, third — deliberately ahead of both familiar flavours.** Four reasons, and the first is the important one: **it needs no filesystem**, so it is the only user-visible part of this Epic that is unblocked *today* if `LE-48` is not resolved. It is also the differentiator, so it should not be the thing that slips; it is the flavour whose verbs touch the real-time set and therefore the one whose authority model most needs to be exercised early; and `TASKMGR` depends on it.

**`-04` before `-05`** only because `.TCB` batch is DOS-flavoured and the `MsDOS/` reference exists — not because DOS matters more. **Each flavour lands with the three-way equivalence test**, so whichever is second inherits the proof obligation for the pair.

**`-07` late, because a batch runtime is an authority multiplier**, and it should not exist before the thing it multiplies is proven.

## 8. Exit criteria

- **All 22 canonical verbs implemented once**, reachable identically from `DOS` and `POSIX`, with the RT verb set reachable from `RT`.
- **The three-way equivalence test is green and is a Red-first Test document**: the same fixture, driven through every flavour, produces byte-identical canonical verbs and identical authorisation decisions. **This is the proof that no flavour is a privileged path.**
- **A boundary test proves there is no escalation verb.** No `sudo`, `su`, setuid or administrator mode exists, and an operator cannot acquire a capability they were not granted from inside the shell — including through batch, redirection, pipes or a misdetected flavour.
- **Each authority amplifier requires its own capability**, demonstrated by a test where the verb is permitted and the amplifier is refused.
- **`G-SEC-5` labels survive** rename, copy, extraction and redirection, demonstrated per transformation rather than argued.
- **`G-SEC-7`**: an off-device file arrives non-executable and quarantined, and a `.TCB` from off-device does not run on the strength of its name.
- **Every RT verb has a stated worst case**, and a test proves the shell cannot cause a deadline miss under adversarial input — including output backpressure, where lines are dropped and *reported* as dropped rather than blocking.
- **`G-RT-6`**: configuration is plain text, diffable, with no binary registry anywhere.
- **A tab is an authority boundary**: a test proves capabilities do not leak between tabs, that opening a tab grants nothing, and that no tab can read another's input.
- **The trusted path holds**: a test proves tab content cannot paint the reserved region, cannot suppress the secure-attention key, and cannot misreport the active flavour.
- **The clipboard preserves `G-SEC-5` labels** across tabs, including quarantine.
- **Untrusted text cannot move the cursor.** A filename, task name, label or spoor field containing escape sequences is rendered inert — proven by a test that plants them (§6.6).
- **No tab can cause a deadline miss**, GUI mode included, under adversarial repaint load — frames drop and are reported as dropped.
- **No artifact claims DOS binary compatibility** (§2).
- **`LE-48` is closed** — the storage decision recorded either way.

## 9. Explicitly out of scope

- **A multi-user POSIX permission model.** Already a recorded non-goal; §3.1 makes it a security property, not a gap.
- **Any escalation mechanism.** Not deferred — excluded.
- **A block-device driver.** Phase 3. `FEAT-P2-02`'s RAM volume exists so this Epic does not wait for it.
- **Executing DOS or Linux binaries.** `.COM`/`.EXE`/ELF loading is not this Epic; `exec`/`FEAT-P0-05` owns native `PE64`/`TXE`, and anything from off-device goes through the code-admission gates. **A shell verb is not a route around `RCG-01`.**
- **A POSIX-shell scripting language.** `.TCB` first, per the MVP spec; a POSIX scripting mode is a later addition once the DOS-flavoured path is proven.
- **Full historical DOS command set or full GNU coreutils.** 22 verbs, deliberately.
- **Networking, CAN, USB.** `EPIC-P3`, which depends on this Epic.
- **A window manager, multiple top-level windows, or a compositor.** The front end is **one** window with tabs (§6). Anything beyond that is not this Epic.
- **Porting any part of `WindowsTerminal/` or `MsDOS/`.** Both are references under §6.4's rule; the language policy forbids the C/C++ either way.
