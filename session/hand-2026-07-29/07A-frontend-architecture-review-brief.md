# Cover Note — Independent Architecture Review: A Tauri-Based Operator Frontend for TinyOS

**Prepared for an independent reviewer. Self-contained; no prior knowledge of this repository
assumed.** Handover 07A doubles as the session record.

**What is wanted:** an adversarial design review of a proposed frontend architecture, and a verdict
on six named questions in §7. **What is not wanted:** a code review — see §2, there is no code.

**Declared interest, stated up front.** This note is written by the agent that produced most of the
analysis it cites, and that recorded an objection to the architecture it is now briefing you to
review. §6 states that objection, states the strongest case against it, and records a correction the
objection needs. Treat §6 as a position to adjudicate, not a conclusion to ratify. **If the review
concludes the objection is wrong, that is a useful outcome and it is the reason for commissioning
you.**

---

## 1. The architecture under review

TinyOS is a from-scratch, 64-bit, real-time operating system. Its Phase 2 deliverable is an
**operator command environment**: one window, many tabs, three interchangeable command "flavours"
(MS-DOS-style, POSIX-style, and native real-time verbs), over one canonical verb core with **one**
authorisation point. A tab may switch from a command interface to a GUI mode.

The proposal under review is that this frontend be built on **Tauri** — a Rust application core
bound to a system webview — with:

- **IPC** as typed, capability-scoped commands between the Rust core and the web frontend;
- **plugins** following Tauri's plugin and permission model;
- **rendering** of a multi-tab terminal inspired by Windows Terminal, including a segmented status
  line carrying real-time health, dotted-leader result rows, and pass/partial/fail glyphs;
- **performance** bounded such that no frontend work can cause a real-time deadline miss.

**Modifying Tauri is in scope.** The owner's position, recorded 2026-07-29: Tauri, `tao` and the IPC
internals may be **forked and modified** to fit the security framework. Tauri is Apache-2.0/MIT, so
this is permitted outright. This materially changes the analysis and §6.5 sets out how — reviewers
should not evaluate the proposal as a constraint to live within, but as **a codebase to adapt**.

## 2. Ground truth — please read this before anything else

**The frontend architecture is entirely on paper. None of it is implemented.**

The repository contains seven Rust crates: `kernel`, `exec`, `hal`, `hal-x86_64`, `hal-arm64`, `os`,
`xtask`. There is no shell crate, no UI, display, window, compositor, font, glyph or terminal module
anywhere. Verified by inspection, not assumed.

Also absent, and each one is load-bearing for this architecture:

| Missing | Consequence |
|---|---|
| **A filesystem** | 15 of the 22 specified shell verbs cannot be implemented. Tracked as `LE-48`; the storage decision is unmade |
| **The ACI capability engine** | The authorisation point the whole design turns on. Only stand-in traits exist (`ChannelPolicy`, `CapabilityPolicy`) |
| **A window/input/display service** | No pixels reach a screen today by any path |
| **A webview/rendering engine** | Tauri ships none — it binds the *platform's* webview. TinyOS has no platform webview |

What *does* exist is a real-time kernel core: scheduler with priority inheritance and WCET
enforcement, preemption, address spaces with W^X, local IPC (bounded message channels and
shared-memory grants), an audit journal, and a machine-checked assurance spine. 621 host tests and a
set of QEMU fixtures.

**So this is a design review of a proposal, against an existing kernel and an explicit security
charter.** That framing matters: reviewers who assume an implementation exists will calibrate wrongly.

## 3. What is elaborated, and where to find it

| Area | Where | State |
|---|---|---|
| **Tab-host requirements** — trusted path, per-tab authority, clipboard label propagation, real-time doctrine | `goals/epics/EPIC-P2.md` §6 | **Strong.** Written as testable requirements, not prose |
| **Flavour model and single authorisation point** — why three front-ends must not become three policy paths | `EPIC-P2.md` §3 | **Strong.** Names the parser-differential risk explicitly |
| **Windows Terminal as reference *and* counter-example** | `EPIC-P2.md` §6.4 | **Strong.** Adopts the interaction model, explicitly rejects the authority model |
| **Visual target and its injection surface** | `EPIC-P2.md` §6.5 | Good. Includes the escape-sequence/untrusted-text rule |
| **Tauri's actual internals**, mapped onto the OS protection contracts | `docs/tauri-internals-review.md` | **Strong.** Source-grounded, pinned to `tauri-apps/tauri` `872428f` |
| **The protection model** — 14 `PD-*` contracts, 20 security controls, 5 containment classes, 14 code-admission gates | `SECURITY_CHARTER.md`, `goals/security/` | **Strong.** Machine-checked |
| **Application-lane position for Tauri** (distinct from the shell question) | `goals/epics/EPIC-H2.md` | Specified, not decomposed |

## 4. What is *not* elaborated

Listed plainly, because a review that rediscovers these has been wasted.

- **Rendering architecture.** Nothing on font rasterisation, glyph caching, damage tracking,
  repaint scheduling, or compositing. `EPIC-P2` §6.5 specifies what it should *look* like and §6.6
  says repaint must be preemptible and frame-dropping — **there is no design connecting the two.**
- **Input stack.** No keyboard/scancode path, no IME story, and the secure-attention key required by
  §6.3 is named as a requirement with no mechanism behind it.
- **Plugins as such.** This is the weakest area relative to its prominence in the brief.
  `EPIC-P2` does not discuss plugins at all. Whether the *frontend* plugin model and the *OS driver*
  model should be one mechanism or two is **completely unaddressed** — see §7 Q3.
- **Performance budgets for the frontend.** Performance domains are *selected* for the relevant
  contracts, but **nothing is measured and no frontend guardrail has evidence.** The repository
  carries a 625-cell performance catalogue; 11 of 391 release gates have dated evidence, none of
  them frontend.
- **The webview engine.** No decision, no candidate, no evaluation. This is the largest single
  unpriced item in the proposal.
- **Session lifecycle.** Tab creation/destruction, persistence, crash isolation of one tab from
  another, and what happens to a GUI tab that hangs.
- **A frontend-specific threat model.** The `PD-*` contracts are general; nothing maps them onto the
  tab host's own attack surface as a document.

## 5. What is strong, and worth preserving through any redesign

1. **The security doctrine precedes the features and is written to be testable.** `EPIC-P2` §3 and
   §6 state requirements in a form that becomes boundary tests. This is unusual and it is the
   project's main asset here.
2. **There is no root, by construction.** No `sudo`, no setuid, no UID 0, no escalation verb — and
   authority-amplifying flags (`-r`, `--force`) are treated as *separate capabilities* rather than
   conveniences. That is a stronger stance than either DOS or POSIX and it is stated as a non-goal
   that must not be "fixed" for familiarity.
3. **Windows Terminal is used as a counter-example.** The design already identifies that a WT tab
   inherits the user's full access token and that WT has no trusted path — and requires the opposite.
   The prior art was read critically rather than copied.
4. **Tauri's own internals are better aligned than expected.** Caller identity is derived from the
   Rust side rather than the message payload; origin is recomputed per call so navigation changes
   authority; and the permission ACL is resolved at *build time* and baked into the binary. Those
   are genuinely good properties (details in `docs/tauri-internals-review.md`).
5. **`tauri-runtime` is a real trait seam.** A port is `impl Runtime for TinyOsRuntime`, not a fork.
6. **The assurance spine is machine-checked.** Contracts exist before code, and drift fails CI.

## 6. What needs attention — the contested question, stated fairly

### 6.1 The question

**Can a webview-based frontend satisfy `EPIC-P2` §6.3's trusted path?**

§6.3 requires a reserved region of the window that **no tab content can ever paint**, "owned by the
shell host, structurally unreachable by tab content", plus a secure-attention key handled before any
tab sees input, and an unspoofable indication of which flavour is active. The threat is a GUI tab
painting a convincing fake command prompt and harvesting what an operator types into it — on a
machine where the operator may be admitting real-time tasks.

### 6.2 The recorded objection, and a correction it needs

The objection on file (`LE-53`) is that a single-webview frontend cannot express §6.3, because the
reserved region and the tab content would be the same DOM in the same renderer.

**That claim is too strong, and I am correcting it here.** Tauri supports **multiple webviews per
window** (`Window::add_child`, behind its `unstable` feature, with a `multiwebview` example in-tree).
The reserved region could therefore be a *separate webview* owned by the host, with tab content in
sibling webviews — and §6.3 becomes expressible, enforced by isolation *between* webviews rather
than by discipline within one DOM. `LE-53`'s wording needs amending and the reviewer should not
treat "inexpressible" as established.

### 6.3 The sharpened question, which is the one worth reviewing

Inter-webview isolation is supplied by the **webview engine**. On Windows, macOS and Linux that
engine is the platform's, built by Microsoft, Apple or the WebKit/GTK projects. **TinyOS has no
platform webview and would have to build or port one.**

So the architecture appears to reduce to:

> Build a browser engine with sound process/origin isolation, then rely on that isolation for the
> operating system's trusted path.

**Is that sound, or is it circular?** The trusted path is the mechanism protecting the operator from
hostile tab content; the engine providing it would be the largest and most attack-exposed component
in the system. This is the single question I most want an independent verdict on.

### 6.4 Four further tensions the reviewer should weigh

- **Dependency inversion.** `EPIC-P2` records that it *gates* the horizons for the application ABI,
  the browser, and the browser-hosted lab. A webview frontend makes Phase 2 depend on the browser
  horizon that depends on Phase 2.
- **Image budget.** The production kernel image is ~17 KB against an **8 MiB ceiling**, and webview
  runtimes are already recorded as optional profiles *"never additions to the core image by
  definition."* The shell is core. Either the rule or the architecture has to give.
- **Executable memory.** A JavaScript engine JITs. `PD-04` seals executable memory and the
  code-admission gates govern any path that creates it. This needs an explicit admission decision,
  not a configuration flag.
- **Real-time.** §6.6 requires the tab host to run below the real-time floor, be preemptible, and
  drop frames rather than block. Whether a webview engine can be made to honour that — and be
  *shown* to — is unaddressed.

### 6.5 The fork changes the balance — and concentrates the risk on one thing

Because Tauri may be modified, most of §6.4 and most of the criticisms in
`docs/tauri-internals-review.md` stop being objections and become **work items**:

| Objection | Survives a fork? |
|---|---|
| `Capability.local` defaults to `true`, inverting `PD-03` | **No.** A default and a schema |
| `__TAURI_INVOKE_KEY__` is a caller-supplied bearer secret | **No.** Replace with kernel-derived domain identity |
| ACL is a string-keyed in-process filter | **Largely no.** Authority resolution can defer to the real ACI engine |
| IPC is unbounded, no backpressure (`PD-05`) | **No.** Replace the transport with a bounded, fails-closed channel |
| Multi-webview is `unstable`-gated | **No.** A feature gate |
| **The engine: no renderer, JIT, real-time behaviour** | **Yes — entirely untouched** |

**This should be stated plainly because it cuts in the architecture's favour**: most of what the
review faults Tauri for is upstream's reasonable choice for a desktop app framework, not a constraint
on TinyOS. The fork removes every objection it can reach.

**What it leaves is the one that carried the weight anyway.** Forking Tauri does not produce a
rendering engine — `wry` binds the *platform's* webview, and TinyOS has no platform webview. The
engine, the JIT admission question and the real-time behaviour all live in a component that is not
Tauri and not in this repository.

Three consequences the reviewer should weigh:

1. **Prefer the seams to the patches.** `tauri-runtime` is already a trait, so a TinyOS
   window/webview binding is `impl Runtime for TinyOsRuntime` and **neither `tao` nor `wry` needs
   patching at all** — replacing `tao` wholesale is cheaper than modifying it. `RuntimeAuthority`,
   by contrast, is a concrete struct with no seam, so that is where a patch set genuinely belongs
   (and a resolver trait there might be upstreamable rather than carried).
2. **Two binding repository rules collide with a vendored fork.** Rule 7 requires all code under
   `os/src/`; rule 4 caps any crate at 20,000 lines with `CODING_STANDARDS.md` stating *"do not ask
   for an exception; there isn't one."* Measured at `872428f`, **`tauri` is 32,457 lines** — 1.6× the
   ceiling — with `tauri-utils` at 15,452. A fork inside the workspace is non-compliant on day one.
   It must live outside as a pinned dependency, be split, or the rule must be amended deliberately.
3. **Security maintenance transfers with the patch.** Upstream's advisory flow stops applying to
   modified surface. The health metric worth fixing now is **the size of the patch set against
   unmodified upstream**; past a few hundred reviewable lines the fork has become a rewrite and
   should be recognised as one.

### 6.6 Attention items outside the contested question

- **Nothing about frontend performance is measured.** Related: one recorded finding has an accept
  path running **17.6–39.1× its budgets** and is the most serious unanalysed result in the
  repository (`LE-42`).
- **Timing baselines have never met a Linux CI runner** (`LE-23`) — the numbers exist but the claim
  is untested.
- **Plugin/driver conflation is an unforced risk.** With no position recorded (§4), the two models
  may be designed independently and later discovered to overlap.

## 7. Where I want help — the eight questions

**Answer these assuming Tauri may be modified** (§6.5). Questions 1, 2 and 5 are the ones a fork does
not dissolve.

1. **Trusted path.** Given §6.3 and §6.2's correction: is a separate-webview reserved region an
   acceptable basis for an OS trusted path *when the OS itself supplies the engine enforcing the
   isolation*? If not, what is the minimum mechanism that would be acceptable?
2. **Circularity.** Is §6.3's reduction real — building a browser engine and then trusting it for the
   trusted path — and if so what breaks it? A small dedicated trusted renderer for the reserved
   region only, a firmware-backed indicator, a hardware attention key, something else?
3. **Fork surface.** Is §6.5's "prefer the seams to the patches" the right strategy — implement
   `tauri-runtime`'s traits and leave `tao`/`wry` unpatched, confining modification to authority
   resolution and IPC transport? What patch-set size should be treated as the alarm threshold at
   which the fork should be acknowledged as a rewrite?
4. **Governance collision.** A vendored fork breaks rule 7 (all code under `os/src/`) and rule 4
   (20,000-line crate ceiling; `tauri` is 32,457). Which is right: hold the fork outside the
   workspace as a pinned dependency, split it, or amend the rule? The rules are deliberately
   inflexible, so this needs a considered answer rather than an exception.
5. **Engine.** This is now the largest unpriced item in the proposal and the fork does not touch it.
   Port an existing engine, build a restricted one, or avoid a webview for the shell entirely? What
   would each cost, and which is compatible with a real-time system and an 8 MiB core-image rule?
6. **Plugins vs drivers.** Should the frontend plugin model and the OS device-driver model be one
   mechanism or two? Both mediate untrusted extension code against a capability system, and neither
   is designed yet. **This is the question with the most freedom still available**, and a fork widens
   it further, since Tauri's plugin model can be reshaped rather than adopted.
7. **Performance invariants to fix now.** Which frontend budgets should be fixed *before* any code
   exists, so they cannot be retrofitted? The current candidate is a strict text-buffer / renderer
   separation, so the renderer can be starved independently and drop frames. Right invariant, or is
   there a better one?
8. **Sequencing and scope.** Is there a path that delivers operator UX value early *without* the
   engine dependency — a host-side console over the existing serial/deploy link — and does taking it
   risk entrenching something that must later be undone on-target? And is "DOS + POSIX + real-time
   flavours, multi-tab, GUI tabs, trusted path, webview rendering" a coherent Phase 2, or several
   phases wearing one name? If it should be cut, what is the smallest version that still proves the
   thesis?

## 8. Reading order for the reviewer

1. This note.
2. `goals/epics/EPIC-P2.md` — §1 (the storage precondition), §3 (authorisation), §6 (the tab host).
   **The core of the review.**
3. `docs/tauri-internals-review.md` — what Tauri actually does, mapped to the protection contracts.
4. `SECURITY_CHARTER.md` — `PD-01`…`PD-14` and the code-admission gates.
5. `session/hand-2026-07-29/03A-tauri-and-the-tab-host.md` — the recorded objection, as filed.
6. `agent.md` — the ten non-negotiable rules; rules 1, 2, 9 and 10 bear directly.
7. Optional: `docs/whole-system-context.md` for how the frontend sits among the other destinations.

## 9. Form of the review

Most useful, in order: **a verdict on each §7 question**; a list of anything in §5 that is weaker
than claimed; anything in §4 that is more urgent than presented; and any failure mode not considered
at all. Disagreement with §6 is welcome and expected — the objection is on file precisely so it can
be tested.

Least useful: style, naming, and anything predicated on code existing.

---

## Session record

No code, no contracts, no register change. Two corrections owed and made:

- **`LE-53`'s "inexpressible" is too strong** (§6.2). Tauri supports multiple webviews per window, so
  a host-owned reserved region is expressible. The row should be amended when contiguity allows — it
  is blocked behind `LE-53` itself being uncommitted, which a concurrent session is also waiting on.
- **`06A` §4 and the earlier "reference only, never built upon" recommendation are superseded** by
  the owner's fork decision. `docs/tauri-internals-review.md` §7 has been rewritten accordingly
  (fork strategy, seam-versus-patch, the rule 4/rule 7 collision, and the measured crate sizes).
  `06A` is left as written, per the convention that dated session documents are an immutable record.

The working-tree assurance gate is currently red on another session's in-progress
`TEST-P1-06-01-A`; this session's subset was verified over clean `HEAD` in a throwaway worktree
(green: 53 loose ends, 91 status headers) per `agent/CONCURRENT_SESSIONS.md`.
