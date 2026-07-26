# Atomic OS-Level Features: Building a Modern Conversational G-Code Application on TinyOS

Status: **working draft — candidate features feeding `goals/` Feature/Story decomposition**

## Purpose

[`user-stories.md`](user-stories.md) states US-10 as an explicit **post-MVP** stretch goal: a conversational/shape-based programming aid layered on top of raw G-code entry, reusing the same interpreter and interpolation service rather than a separate code path. This document is the atomic-feature decomposition that makes US-10 (and the rest of the flagship demo's user stories) buildable by a **third-party application developer** — someone who is not on the TinyOS core team, building against TinyOS's public ACI capability surface the same way any other caller would.

Every row below is a single, independently specifiable, independently testable OS-level capability — an API, a driver class, an ACI capability, a data model, a timing guarantee, or an I/O primitive. None of these are whole-subsystem descriptions or UI mockups; each is sized to plausibly become one `goals/stories/STORY-*` entry (see [`STORY-P0-01-01`](../../goals/stories/STORY-P0-01-01.md) for the target granularity) once a corresponding Feature exists under an active Epic, per the [Goal → Epic → Feature → Story → Test](../../goals/index.html) model.

This document does not commit any of these features to a Roadmap phase or a `goals/` entry on its own — per the just-in-time decomposition principle already used elsewhere in this folder (see [`test-cases.md`](test-cases.md)'s cross-reference note), that promotion happens when a `motion`-related Feature is created under an active Epic.

## How to read the tables

- **Feature** — a short, unique name for the atomic capability.
- **Description** — one sentence: what the OS must provide, not how an app would use it.
- **Crate / subsystem** — the `os/src/` crate from [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md)'s crate map that owns this feature. Where no existing crate fits cleanly, a submodule location is proposed with a one-clause justification rather than inventing a new top-level crate prematurely — consistent with that document's "nothing here is created ahead of its need" principle.
- **Primary driver** — the [`user-stories.md`](user-stories.md) story (US-#) or [`requirements.md`](requirements.md) requirement (R#) this feature most directly enables, plus a `SeedMVP.md` Goal code in parentheses where one applies directly.

Many features enable more than one story; the "Primary driver" column names the strongest single link, not an exhaustive list.

---

## A. G-Code Interpreter & Program Model APIs

Owner crate: `motion` (a G-code front-end / program-model submodule) — [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md)'s delivery strategy already states the G-code front-end grows inside `motion` alongside the interpolation core, not as a separate crate, so that a program is validated and interpolated by one coherent pipeline.

| # | Feature | Description | Crate / subsystem | Primary driver |
|---|---|---|---|---|
| A1 | Block Tokenizer | Parses a line of raw G-code text into typed word-address/value tokens with no side effects and no partial-program state mutation. | `motion` (program model) | US-9 |
| A2 | Program AST Construction | Assembles tokenized blocks into an in-memory, block-indexed program representation that resolves modal state (active plane, units, feed mode) per block. | `motion` (program model) | US-1 |
| A3 | Syntax & Modal-Conflict Validator | Validates a program or a single MDI block against the supported G/M-code surface and modal-group conflict rules, returning structured errors rather than silently accepting or silently rejecting. | `motion` (program model) | US-3 |
| A4 | Program Load/Unload API | Admits a validated program into the active program slot, atomically replacing or clearing whatever was previously loaded. | `motion` (program model) | US-1 |
| A5 | Block-by-Block Execution Cursor | Exposes the currently executing block index and an "advance exactly one block" operation as a distinct ACI-gated action from continuous run. | `motion` (interpolation service) | US-4, R1.3 |
| A6 | MDI Immediate-Execution Channel | Accepts one ad hoc block and executes it through the same interpreter/interpolation path as a program block, without a loaded program. | `motion` (program model) | US-3 |
| A7 | Dry-Run Execution Mode | Executes a program's motion path through the full interpolation/kinematics pipeline while suppressing every process output, as an explicit mode distinct from real execution. | `motion` (interpolation service) | US-5, R1.3 |
| A8 | Program Restart / Resume-from-Block | Re-establishes correct modal state (tool compensation, active work offset, feed mode) when execution resumes mid-program after a halt, instead of resuming with stale state. | `motion` (program model) | TC-7 |
| A9 | Trajectory Blending/Lookahead Configuration | Exposes per-program or per-block corner-blending tolerance and exact-stop directives to the interpolation service. | `motion` (interpolation service) | R1.2, TC-1 |
| A10 | Structured Interpreter Error/Alarm Channel | Surfaces parse errors, modal conflicts, and kinematic-infeasibility conditions as typed, ACI-visible alarm records rather than raw diagnostic text. | `motion` (program model) | R4.3, TC-5 |

## B. Real-Time Motion & Telemetry APIs

Owner crate: `motion` — the Motion & Interpolation Service, Process-Synchronized Output Service, and Position Feedback Abstraction already specified in [`docs/physical-ai-reference-workloads.md`](../../docs/physical-ai-reference-workloads.md) are the source of every telemetry and command feature below.

| # | Feature | Description | Crate / subsystem | Primary driver |
|---|---|---|---|---|
| B1 | Live Position Telemetry Feed | A WCET-bounded, subscribe-only stream of machine- and work-coordinate position, updated at a declared tick rate. | `motion` (telemetry lane) | US-2, R4.3 |
| B2 | Velocity/Feed-Rate Telemetry Feed | Live actual (post-override) feed rate and per-axis velocity, alongside the programmed values, on the same telemetry lane as B1. | `motion` (telemetry lane) | US-6 |
| B3 | RTCP/TCPC Status Channel | Reports whether RTCP is active, which kinematics module is loaded, and any proximity-to-singularity warning. | `motion` (kinematics submodule) | TC-5, R3 |
| B4 | Axis Fault/Following-Error Event Stream | Pushes a typed fault event the instant a following-error or feedback discontinuity exceeds its configured tolerance. | `motion` (Position Feedback Abstraction) | US-15, TC-8 |
| B5 | WCET-Bounded Telemetry Subscription API | Lets a caller (local shell, HBP host, or WCI session) register for telemetry at a bounded maximum rate that structurally cannot compete with the RT scheduler for priority. | `motion` / `aci` boundary | US-12 (G-RT-3, G-RT-4) |
| B6 | Feed/Rapid/Spindle Override Command API | Three independently adjustable override channels, each ACI-gated, applied without halting execution. | `motion` (Process-Synchronized Output Service) | US-6, R4.2 |
| B7 | Pause / Safe-Hold Command | A single, always-available ACI action that transitions the active program to the documented safe-hold state. | `motion` (Safety Interlock primitive) | US-7, R5.2 |
| B8 | Command-Authority Lease Query/Transfer | Exposes the current command-authority holder and a supervisor takeover path, reusing WCI's authority-lease model for local sessions too. | `motion` / `wci` boundary | US-8 |
| B9 | Kinematics Module Identity & Capability Query | Lets an app discover which kinematics module (machine geometry, axis count/arrangement) is loaded, instead of hardcoding a geometry assumption. | `motion` (kinematics submodule) | R3.2 |
| B10 | Simulated Feedback Injection Hook (Tier 0/1) | A test-only API to inject simulated position/fault data through the `PositionFeedback` trait, so an app's own test suite can exercise fault handling without real hardware. | `motion` (Position Feedback Abstraction) | TC-8 |

## C. Program Editing, Storage & Data Interchange APIs

Owner crate: `motion` (a program-store submodule) — the semantics here (block-level edits that must re-validate modal state, undo of a G-code mutation, not a generic text file) are G-code-specific, distinct from `shell`'s general-purpose file verbs in [`docs/cli-compatibility-mvp.md`](../../docs/cli-compatibility-mvp.md), which explicitly defers a general text editor. Raw persistence reuses the mandatory storage class driver from the [Universal Driver Model](../../docs/universal-driver-model.md).

| # | Feature | Description | Crate / subsystem | Primary driver |
|---|---|---|---|---|
| C1 | Program Store CRUD API | Create/read/update/delete named G-code programs in persistent storage, capability-gated like any other ACI action. | `motion` (program store) | US-1, US-9 |
| C2 | Program List/Directory Query | Enumerates stored programs with metadata (size, block count, last-modified timestamp, checksum). | `motion` (program store) | US-1 |
| C3 | Block-Level Insert/Modify/Delete API | Mutates a single word or block of a stored program without a full-file rewrite, re-validating through the same validator as a full load (A3). | `motion` (program store) | US-9 |
| C4 | Undo/Redo Journal | A bounded-depth edit-history API for program-store mutations, so an app doesn't have to re-derive edit history to offer undo. | `motion` (program store) | US-9 |
| C5 | Background Edit Session | Edits a non-active program concurrently with a different program's real-time execution, fully isolated from the executing program's state. | `motion` (program store) | US-9 |
| C6 | Program Search API | Locates a block, sequence number, or text pattern within a program, returning a position handle usable by the block cursor (A5). | `motion` (program store) | US-9 |
| C7 | Fixed-Form Snippet/Template Registry | A capability-scoped store of reusable, named block templates an app can offer as insertable snippets, distinct from the cycle library (category D). | `motion` (program store) | US-9 |
| C8 | Import/Export Interchange API | Round-trips programs and offset data through a documented file format over the mandatory storage class driver, for exchange with removable media or a host. | `motion` (program store) / `drivers` | R2.3, US-11 |
| C9 | Program-Size Admission Policy Query | Exposes the configured maximum program size/block count so an app can warn proactively rather than fail at the limit. | `motion` (program store) | derived |
| C10 | Expression/Parameter Calculator Service | Evaluates arithmetic/variable expressions used in parametric programming, independent of any one app's UI. | `motion` (program model) | US-10 (adjacent) |

## D. Cycle/Shape Library & Parameter Data Model

This is the direct enabler for US-10's conversational/shape-based programming aid. Owner crate: `motion` (a cycle/shape submodule, generated-code path shares the same validator as hand-written code from category A) — split into a dedicated `os/src/motion-cycles/` crate only if `motion` approaches the 20,000-line ceiling per [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md), consistent with the "don't pre-split" guidance in [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md).

| # | Feature | Description | Crate / subsystem | Primary driver |
|---|---|---|---|---|
| D1 | Cycle/Shape Definition Registry | A data model for named machining cycles, each with a typed parameter schema — a generic capability category, not any specific vendor's cycle set. | `motion` (cycle submodule) | US-10 |
| D2 | Parameter Schema Validation API | Validates operator-entered cycle parameters (type, range, required/optional) before any code generation occurs. | `motion` (cycle submodule) | US-10 |
| D3 | Cycle-to-G-code Generation Function | Deterministically expands a cycle plus its parameter set into a standard block sequence through the same interpreter front-end (category A) as hand-written code. | `motion` (cycle submodule) | US-10 |
| D4 | Generated-Code Provenance Tag | Every block generated from a cycle carries a traceable link to its originating cycle/parameter set, so hand-editing generated code is a visible, deliberate act. | `motion` (cycle submodule) | US-10 |
| D5 | Shape/Geometry Parameter Types | Reusable geometric primitive types (point, arc, contour list) shared between the cycle library and the toolpath preview feed (category E), avoiding two divergent geometry models. | `motion` (cycle submodule) | US-10 |
| D6 | Custom/User-Defined Cycle Registration | Lets a site or app register additional cycles beyond the built-in set through the same admission path used for driver extensions — Open/Closed applied to the cycle library. | `motion` (cycle submodule) | US-10 |
| D7 | Cycle Library Versioning & Compatibility Check | Cycles declare a schema version, so a program generated from an older library version remains reproducible as the library evolves. | `motion` (cycle submodule) | derived |
| D8 | Arbitrary/Free-Form Contour Input Model | A generic point/segment-list input type for shapes that don't fit a fixed-parameter cycle, sharing the geometry types from D5. | `motion` (cycle submodule) | US-10 |

## E. Program Simulation & Toolpath Preview Data Feed

What an app needs from the OS to render a toolpath preview — not the rendering itself, which is app-level. Owner crate: `motion` (reuses the interpolation/kinematics pipeline in non-real-time "batch" mode).

| # | Feature | Description | Crate / subsystem | Primary driver |
|---|---|---|---|---|
| E1 | Offline Path Evaluator | Runs the interpolation/kinematics pipeline against a loaded program with no real-time pacing and no real output, producing a full position/time trace for preview. | `motion` (interpolation service, batch mode) | US-5 (adjacent), US-10 |
| E2 | Toolpath Point-Stream Export API | Returns the evaluated path as a structured, app-consumable stream of position/orientation/feed/block-index samples. | `motion` (interpolation service) | US-10 |
| E3 | Live Execution Shadow Feed | While a program actually runs, exposes the same structured stream in real time so a preview can highlight the current block against the precomputed path. | `motion` (telemetry lane) | US-2 |
| E4 | Travel-Limit Pre-Check API | Evaluates a program against declared axis travel limits and reports out-of-envelope segments before the program is ever admitted for real execution. | `motion` (program model) | R5 (adjacent) |
| E5 | Tool Swept-Volume Geometry Feed | Combines tool geometry (from the tool table, category F) with the evaluated path so an app can render material-removal visualization without re-deriving tool shape. | `motion` / tool data model | derived |
| E6 | Simulation-Mode Behavior-Difference Flags | An explicit, queryable list of which process outputs or behaviors are suppressed or altered in preview/dry-run mode, so an app never has to guess or hardcode that list. | `motion` (interpolation service) | derived |
| E7 | Work-Offset-Aware Preview Recompute | Recomputes the preview against a candidate work offset without touching active machine state, for setup verification before committing a change. | `motion` (program model) | R2.1 (adjacent) |

## F. Tool & Work-Offset Management APIs

Owner crate: `motion` (data model), gated through `aci`; editable through `shell`'s TINYCMD front-ends per US-11.

| # | Feature | Description | Crate / subsystem | Primary driver |
|---|---|---|---|---|
| F1 | Work Coordinate System Table API | Read/write access to the numbered work-offset table, capability-scoped. | `motion` (data model) | R2.1, US-11 |
| F2 | Active WCS Selection & Switch-Semantics Query | Exposes which work offset is active and the defined rule for whether switching is permitted mid-program (per `requirements.md`'s open item). | `motion` (data model) | TC-2 |
| F3 | Tool Offset Table API | Read/write access to tool length and radius/diameter compensation entries. | `motion` (data model) | R2.2, R2.3 |
| F4 | Tool Compensation Activation State Query | Reports which compensations are currently active for the executing tool, independent of the raw table data. | `motion` (data model) | TC-3 |
| F5 | Tool Measurement Input Channel | Accepts a measured offset value (e.g. from a touch-probe workflow or manual entry) and writes it into the tool/work-offset table through the same validated path as any other write. | `motion` (data model) | derived |
| F6 | Tool Metadata/Type Registry | Associates a tool number with descriptive metadata (type, diameter, holder geometry) usable by both compensation math and toolpath preview rendering (E5). | `motion` (data model) | derived |
| F7 | Offset Table Import/Export | Round-trips tool/WCS tables through the same interchange mechanism as programs (C8), for backup or transfer between devices. | `motion` (data model) / `drivers` | R2.3 |
| F8 | Offset-Change Audit Trail | Every write to the tool or WCS table is logged with full ACI provenance, since these values directly affect where the tool physically goes. | `aci` | R4.4 |

## G. Operator Mode & Override Control APIs

Owner crate: `motion` for state and command paths, `aci` for the capability gate — every mode change and override is explicitly an ACI-gated action per R4.4, never an implicit UI-only state.

| # | Feature | Description | Crate / subsystem | Primary driver |
|---|---|---|---|---|
| G1 | Mode State Machine API | Exposes and transitions between AUTO / MDI / JOG / HANDLE-WHEEL / EDIT / REFERENCE as an explicit, ACI-gated state. | `motion` / `aci` | R4.1 |
| G2 | Jog Command API (Continuous & Incremental) | Issues bounded, capability-gated manual axis motion, distinct from program execution. | `motion` (interpolation service) | R4.1 |
| G3 | Handwheel Input Binding API | Maps a physical or virtual handwheel input device to incremental axis motion at a selectable multiplier. | `motion` / `drivers` (HID class) | R4.1 |
| G4 | Machine-Reference/Home Sequence API | Initiates and reports completion of the axis-homing sequence required before absolute machine-coordinate motion is trusted. | `motion` (interpolation service) | R4.1 |
| G5 | Machine-Lock Diagnostic Mode | Suppresses actual axis motion while still executing/validating program logic — a diagnostic mode distinct from dry-run's process-output suppression (A7). | `motion` (interpolation service) | R4.3 (adjacent) |
| G6 | Optional-Stop / Block-Skip Directive Handling | Exposes program-level optional-stop and block-skip flags as operator-toggleable execution modifiers. | `motion` (program model) | derived (industry-standard convention per `references.md`) |
| G7 | Mode-Change Provenance Logging | Every mode transition is logged with the same audit discipline as any other ACI-gated action. | `aci` | R4.4 |

## H. Modern UX-Enabling Capabilities

This is the section explicitly requested for "much more modern UX than a mid-2010s shop-floor CNC control interface." Every entry is grounded in an existing TinyOS architectural pattern (Universal Driver Model class drivers, the ACI capability/telemetry model, the `compute`/`inference` crates) — none of these invent an unrelated UI framework.

| # | Feature | Description | Crate / subsystem | Primary driver |
|---|---|---|---|---|
| H1 | GPU-Accelerated Compositor Surface Class Driver | A mandatory display-class driver (per the [Universal Driver Model](../../docs/universal-driver-model.md)) exposing damage-region-based surface composition rather than a single full-frame text console, enabling a fluid touchscreen UI. | `drivers` (display class) | derived (G-HW-2) |
| H2 | Multitouch Input Class Driver | A mandatory HID-class extension reporting multi-point touch/gesture events, not just single-pointer input, on the same UDI contract discipline as any other class driver. | `drivers` (HID class) | derived (G-HW-2) |
| H3 | Vector/2D Scene-Graph Primitive API | OS-provided path/shape/text-layout primitives an app composes into UI, exposed as an extension surface of the compositor class driver (H1), so every app doesn't reimplement a rasterizer. | `drivers` (display class extension) | derived |
| H4 | Asynchronous Event & Notification Bus | A pub/sub channel, off the RT path, for UI-relevant events (alarm raised, mode changed, telemetry threshold crossed) so an app reacts to state changes instead of polling. | `aci` (event subscription) | derived (G-RT-4) |
| H5 | Theming & Accessibility Configuration Schema | A plain-text config schema, consistent with TinyOS's existing `TINYOS.CFG` philosophy, for color scheme, contrast, and text-scaling preferences an app reads at startup and on live change. | `config` | derived (G-RT-6) |
| H6 | Live 3D Toolpath Scene Data Feed | Extends the toolpath point-stream (E2) with orientation and tool-geometry data pre-shaped for GPU-buffer upload, avoiding a CPU-side re-projection step before a rendered 3D preview. | `motion` / `compute` | US-10 |
| H7 | GPU Buffer Handoff via Unified Memory Manager | Lets an app hand a toolpath/geometry buffer to GPU-side rendering code without an explicit copy, on hardware where the UMM (per [`docs/inference-architecture.md`](../../docs/inference-architecture.md)) supports true unified memory, falling back to explicit copy elsewhere behind the same handle API. | `compute` (Unified Memory Manager) | derived (G-HW-6) |
| H8 | Natural-Language Command Intake via Local Inference | An ACI capability that accepts a natural-language operator request and resolves it, through the local LLM per `docs/inference-architecture.md`, into one or more pre-registered ACI capability calls — never a freeform "execute this" path. | `inference` | derived (G-AI-2) |
| H9 | Voice Input Class Driver | A generic audio-input class driver whose captured stream is the transport for the natural-language intake capability (H8), decoupled from any specific microphone vendor. | `drivers` (audio/sensor class) | derived |
| H10 | Responsive-Input Latency Budget Declaration | A contractual requirement on the display/input class drivers (H1, H2) that input-to-compositor-acknowledgment latency is bounded and measured, since a laggy touch UI would undermine the modern-UX goal as surely as a missing feature. | `drivers` (display/HID class) | derived |
| H11 | Live Alarm/Status Iconography Data Model | A structured (not free-text-only) alarm/status representation, extending the interpreter error channel (A10), so a modern UI renders consistent icons/severity coloring instead of parsing text. | `motion` (program model) | R4.3 |

## I. Remote & Collaborative Development APIs

An app developer's own tooling needs: deploying and debugging the conversational-programming app itself, reusing the existing deploy and secure-channel machinery rather than inventing a parallel one.

| # | Feature | Description | Crate / subsystem | Primary driver |
|---|---|---|---|---|
| I1 | App Hot-Deploy Endpoint | The app is deployed/updated as a hot-swappable component through the existing `deploy-device`/`deploy-client` machinery — atomic swap with automatic health-check rollback per [`docs/deploy-protocol.md`](../../docs/deploy-protocol.md). | `deploy-device` / `deploy-client` | derived (G-DX-3) |
| I2 | App Capability Manifest | The app declares its required ACI capabilities up front, admission-controlled the same way a driver manifest is admitted under the Universal Driver Model. | `aci` | derived (G-AI-2) |
| I3 | Structured Debug/Trace Export Channel | A capability-scoped stream of app-level structured log/trace events, reachable over HBP/WCI for remote debugging, distinct from and never delaying the RT telemetry lane. | `aci` / `bridge-device` / `wci` | US-12, US-13 |
| I4 | Remote Session Attach for Live State Inspection | Lets an authenticated remote developer session query current app/ACI state (never RT-path internals) for debugging, without impersonating an operator session. | `aci` / `wci` | US-13 |
| I5 | App State-Transfer Hook for Hot-Deploy | An optional routine the app implements so in-progress UI/session state survives a hot-deploy swap rather than resetting on every update, per the open item in `docs/deploy-protocol.md`. | `deploy-device` | derived |
| I6 | Deploy Health-Check Contract for App Components | The app defines what "healthy" means (UI thread responsive, ACI session established) so a failed deploy aborts cleanly per the existing deploy protocol's failure semantics. | `deploy-device` | derived |
| I7 | Versioned App-Facing ACI Compatibility Contract | The ACI capability surface an app is built against is versioned, analogous to the DCI's versioning discipline, so a kernel-side ACI change never silently breaks a deployed app. | `aci` | derived (G-HW-3 pattern reused) |

## J. Safety & ACI Integration Requirements

Every feature above is subject to these — they are restated here as their own atomic, testable requirements because they are the ones most likely to be treated as implicit rather than verified, and this project's Non-Negotiables treat them as blocking, not aspirational.

| # | Feature | Description | Crate / subsystem | Primary driver |
|---|---|---|---|---|
| J1 | Mandatory Capability Declaration for Every App Action | No app action reaches motion, tool data, or program storage without a pre-registered, typed ACI capability — no generic "run command" escape hatch. | `aci` | R4.4 (G-AI-2) |
| J2 | No-Bypass Guarantee Across Input Modalities | A touch gesture, a spoken command, and a typed command all resolve to the same policy-gated capability call — no input modality gets a shorter path to execution. | `aci` | README Non-Negotiable #2 |
| J3 | Full Command Provenance for Every UI-Originated Action | Who (session/user), what capability, what parameters, what was actually executed — logged identically regardless of which input modality produced the action. | `aci` | R4.4 |
| J4 | Explainable Denial Channel | When the ACI denies a UI-originated action, it returns a structured, renderable reason, so the app can show the operator why, not just that it failed. | `aci` | derived (G-AI-5) |
| J5 | Fail-Safe Default on UI/Session Loss | Losing the app's session (crash, disconnect) resolves the machine to the existing safe-hold behavior, identical to an HBP/WCI "operator gone" event — never continued execution of the last known UI state. | `motion` (Safety Interlock) | US-14 (adjacent), R5.2 |
| J6 | Hardware E-Stop Path Independence for GUI/Voice Input | Restated explicitly for any touch/voice-driven app: the physical e-stop is never mediated by the app, its framework, or its input drivers. | `motion` (Safety Interlock) | US-14, R5.1 |
| J7 | Rate-Limited Capability Calls per Session | Every ACI capability an app can call carries a declared rate limit, so a runaway UI loop (e.g. a stuck touch event firing repeatedly) cannot flood the command lane. | `aci` | derived (G-AI-2) |
| J8 | Capability-Scoped Multi-Session Isolation | A monitor-only remote viewer session cannot reach write-capable calls even if it renders the same UI as an operator session — enforced server-side, never by merely hiding controls client-side. | `aci` / `wci` | US-12, US-13 (G-RC-3) |

---

## Summary

86 atomic features across 10 categories (A–J): interpreter/program-model (10), real-time motion/telemetry (10), program editing/storage/interchange (10), cycle/shape library (8), simulation/preview data feed (7), tool/work-offset management (8), operator mode/override control (7), modern-UX-enabling primitives (11), remote/collaborative developer tooling (7), and safety/ACI integration (8).

## References

See [`references.md`](references.md) for full citation of the source manuals used as background for this document, including the *Manual Guide (Milling and Turning)* conversational-programming reference. Consistent with that document's rule, nothing in this file reproduces the source manual's text, screen layouts, dialog wording, or parameter tables — each feature above is described as a generic OS-level capability category, informed by (a) the well-established, industry-wide product class of shop-floor/conversational CNC programming tools referenced generically in `references.md`, and (b) TinyOS's own existing architecture (`docs/physical-ai-reference-workloads.md`, `docs/universal-driver-model.md`, `docs/mvp-delivery-strategy.md`, `docs/inference-architecture.md`, `docs/deploy-protocol.md`).

## Status

Working draft. None of the features above are yet promoted to `goals/` Feature/Story entries — per this folder's existing convention (see [`test-cases.md`](test-cases.md) and [`user-stories.md`](user-stories.md)'s own cross-reference sections), that promotion happens once a `motion`-related Feature exists under an active Epic, starting with the categories that back US-1 through US-8 (already MVP-committed) before the post-MVP categories (D, E, H) that back US-10.
