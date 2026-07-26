# Requirements: 5-Axis Motion Controller Workload

Status: **working draft — feeds into `docs/physical-ai-reference-workloads.md` and `goals/` as items are confirmed**

Scope follows [`docs/physical-ai-reference-workloads.md`](../../docs/physical-ai-reference-workloads.md)'s existing "no compromises" boundary: full-depth software correctness for the items below, with physical positional-accuracy validation deferred until real encoders/drives are bolted onto the MVP hardware.

## R1 — Program execution

- R1.1 Execute a sequential motion program (a G-code-style block list) with rapid positioning, linear interpolation, and circular/helical interpolation moves.
- R1.2 Support trajectory blending/lookahead across consecutive blocks so cornering doesn't require a full stop between segments, unless the program explicitly requests an exact-stop.
- R1.3 Support single-block execution (one block per operator confirmation) and dry-run (motion simulated/traced without engaging real output) as distinct, ACI-gated modes.

## R2 — Coordinate systems and tool data

- R2.1 Support a numbered work coordinate system offset table (multiple part/fixture origins selectable per program), independent of machine (home-referenced) coordinates.
- R2.2 Support tool length compensation and tool radius/diameter compensation as first-class, always-available features — not deferred conveniences (per the "no compromises" commitment).
- R2.3 Maintain and expose a tool offset table editable through TINYCMD (both DOS and POSIX front-ends, per [`docs/cli-compatibility-mvp.md`](../../docs/cli-compatibility-mvp.md)) and remotely over HBP/WCI.

## R3 — Simultaneous 5-axis motion (RTCP/TCPC)

- R3.1 For a programmed tool-tip path plus tool orientation, compute the inverse-kinematics transform that keeps the tool tip on the programmed path as the rotary axes reorient the tool — the committed geometry is a trunnion-table configuration (two rotary axes, three linear axes), per `docs/physical-ai-reference-workloads.md`.
- R3.2 The kinematics transform lives as a swappable submodule inside the `motion` crate (per [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md)), so additional machine geometries can be added later without touching the interpolation core.
- R3.3 RTCP/TCPC behavior must be verifiable against simulated axes (via the `PositionFeedback` trait's Tier 0 simulated implementation) before any physical machine is involved.

## R4 — Operator experience (Fanuc-class bar)

- R4.1 Mode selection: automatic program execution, manual data input (MDI), continuous jog, incremental jog, handwheel jog, program edit, and machine-reference/home — each an explicit, ACI-gated mode, not an implicit UI state.
- R4.2 Override controls: feed rate override, rapid override, and spindle-speed override, each independently adjustable during program execution without halting it.
- R4.3 Diagnostics: live position readout in both machine and work coordinates, an alarm/fault display, and a program list/editor — reachable from local TINYCMD and remotely over HBP/WCI, per Design Pillar 2's remote-first UX principle.
- R4.4 Every mode change and override adjustment is an ACI-gated action with full provenance, exactly like any other command — there is no separate, less-audited "operator panel" privilege path.

## R5 — Safety

- R5.1 A hardware e-stop, once real motion hardware is attached, is wired outside every software layer per Non-Negotiable #4 (G-PA-4) — never mediated by TINYCMD, ACI, or any network state.
- R5.2 A motion fault (axis fault, following-error exceeding tolerance, unexpected feedback discontinuity) resolves to a documented safe-hold state without requiring operator intervention to *reach* safety.

## Open items to confirm against the real machine

These are explicitly flagged as things the case owner's hands-on access to a real Fanuc-controlled 5-axis machine should help confirm or correct, not assumptions to build against blindly:

- Exact behavior expected when switching work coordinate systems mid-program (should this be program-start-only, or allowed mid-block-list?).
- Whether single-block mode should re-engage tool compensation state identically to continuous mode, or has different edge-case behavior at block boundaries.
- The specific rotary-axis travel-limit and singularity-avoidance behavior expected near the trunnion table's kinematic singularities (a known hard problem in 5-axis RTCP, worth confirming against real operator expectations rather than inventing a policy in the abstract).
