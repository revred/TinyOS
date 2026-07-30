# Physical AI Reference Workloads — 5-Axis CNC, Wire DED Robot Arm, Resin-Curing UV Array

Status: **draft / the 5-axis CNC is the flagship MVP demonstration; Wire DED and resin-curing are validated-generality follow-ons**

## Purpose

Three concrete physical-AI workloads anchor TinyOS's real-time control design, chosen because they stress the RT core in three genuinely different ways rather than being three variations on the same problem:

1. **5-axis CNC controller** (flagship MVP demonstration) — few axes, very high coordination precision, complex kinematics, an operator-experience bar set by industrial controllers like Fanuc's.
2. **Wire DED robot arm** (Directed Energy Deposition — a wire-fed additive manufacturing process) — arm-style kinematics plus a process parameter (wire feed, energy source power) that must track motion, not just fire on a timer.
3. **Resin-curing UV array** for a resin-based (vat photopolymerization) 3D printer — almost no motion complexity (one lift axis), but a high-channel-count, precisely-timed output array synchronized to that motion.

The point of specifying all three together, rather than just the CNC, is to prove — not merely assert — that **one RT core, with one set of primitives, genuinely serves all three**, per the explicit goal that "we should be able to run all of these workloads on TinyOS." If any one of them had required its own bespoke scheduler or its own timing model, that would be a sign the RT core wasn't general enough, and this document exists partly to surface that risk early rather than discover it after building three incompatible subsystems.

## Shared RT primitives (what makes "run all three" true)

Rather than three separate control subsystems, TinyOS specifies four shared, general-purpose RT primitives. Each workload below is a *configuration* of these primitives — a different kinematics module, a different I/O class driver, a different set of process-parameter functions — not a different kernel.

### 1. Multi-Axis Motion & Interpolation Service

- Given a target trajectory segment (line, arc, helix, or a generic parametric path), computes per-tick position/velocity/acceleration commands for every axis, respecting per-axis velocity/acceleration/jerk limits, at a deterministic tick rate declared as part of the service's WCET budget per [`CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md#real-time-discipline-kernel-and-driver-code).
- The number and kinematic arrangement of axes is not hardcoded — a **kinematics module** (a small, swappable component implementing a defined trait) translates a programmed tool-tip/end-effector path into per-axis commands for a specific machine geometry. This is a direct application of the Open/Closed principle from [`CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md#solid-principles--rust-adapted-never-compromised): adding a new machine geometry means adding a new kinematics module, never editing the interpolation core.
- Serves the CNC's 5-axis trunnion-table geometry and the Wire DED arm's serial-link geometry identically at the interpolation-core level — they differ only in which kinematics module is loaded.

### 2. Process-Synchronized Output Service

- Non-motion process outputs (spindle speed, coolant, wire feed rate, energy-source power, UV array intensity/pattern) are expressed as functions of the motion timeline — position, instantaneous path velocity, or elapsed segment time — not as independently-timed, fire-and-forget outputs. This is what makes deposition-rate control possible for Wire DED and layer-synchronized curing possible for the resin printer, using the same service.
- A process output can declare a **safety interlock condition** (see below) — e.g. "only enabled while motion is active," or "only enabled during a defined exposure window" — enforced by the kernel, not by the calling application code remembering to check.

### 3. High-Channel-Count Synchronized Array Output

- A distinct load profile from motion control: few axes updated at a high tick rate (motion) versus many channels (tens to low thousands, for a UV curing array) updated at a lower rate but with tight cross-channel synchronization and no heap allocation, per the same real-time discipline as everything else in the kernel.
- This primitive is what the resin-curing workload exercises almost exclusively — it has trivial motion requirements (one lift axis) but a demanding array-output requirement the CNC and Wire DED workloads never touch, which is precisely why it's included as a reference workload: it stress-tests a dimension of the RT core the other two don't.

### 4. Position Feedback Abstraction

- The Motion & Interpolation Service **will consume** position feedback through a `PositionFeedback` trait, never a concrete encoder driver type — Dependency Inversion, applied concretely. In the absence of bolted-on physical encoders, Tier 0 testing (QEMU/Renode) **will supply** a simulated feedback source implementing the same trait, generating physically-plausible position data (including realistic error/lag) so the interpolation and closed-loop control logic is exercised exactly as it will be once real hardware arrives. **Status honesty (2026-07-30): neither the trait nor the simulated source exists yet.** What exists today is the `motion` crate's group-frame layer above this seam — typed identities, `FeedbackFrame<32>`/`ActuationFrame<16>`, whole-epoch validation and the deterministic transport double ([`FEAT-P1-08`](../goals/features/FEAT-P1-08.md), host-tier). Per [`ADR 0010`](adr/0010-the-motion-group-is-the-unit-of-control.md), feedback enters control only as a whole-group epoch frame; the per-channel `PositionFeedback` trait sits *below* that boundary, inside the transport/drive adapter, and lands with the delivery contract's later work packages.
- This is the specific mechanism behind the "no compromises" commitment below: bolting on real encoders later means writing a new implementation of an existing trait (per the [Universal Driver Model](universal-driver-model.md)'s class-driver pattern) — it does not mean reopening or rewriting the interpolation/kinematics core.

### 5. Safety interlock primitive

- Generalizes the hardware e-stop pattern already specified for the co-bot case in [`docs/wci-spec.md`](wci-spec.md): a physical or kernel-enforced interlock signal that gates a process output (spindle, energy source, UV array) and cannot be masked or delayed by any software state above it. For Wire DED, the energy source is wired to an "axes actively interpolating" interlock so a stalled or faulted arm cannot continue depositing energy. For the resin printer, the UV array is wired to a "valid exposure window" interlock so light cannot fire outside a defined, motion-synchronized cure cycle.

## Workload 1: 5-axis CNC controller (flagship MVP demonstration)

### Operator experience bar

The reference for expected behavior is the general class of industrial CNC controllers exemplified by Fanuc-style units — this document does not reproduce any vendor's specific manual or source, only the well-established, industry-standard functional expectations that class of controller sets:

- **G-code program execution** (RS-274-style): linear moves (G00/G01), circular and helical interpolation (G02/G03), unit selection (G20/G21), feed-mode selection (G94/G95).
- **Work coordinate systems** (G54–G59-style offsets) and **tool length/radius compensation** (G43, G41/G42-style), so a program is written against the part, not against raw machine coordinates.
- **Operator panel model**: mode selection (AUTO / MDI / JOG / HANDLE-WHEEL / EDIT / REFERENCE), feed/rapid/spindle override dials, single-block execution, dry run, optional stop, machine lock — these are UX/control-flow concepts implemented in TINYCMD and the ACI capability model (an override or mode change is an ACI-gated action, same as any other command, per Design Pillar 5), not a separate privileged subsystem.
- **Diagnostics**: alarm/fault display, live machine- and work-coordinate position readout, tool offset table editor, program list/editor — all reachable both from local TINYCMD and remotely over HBP/WCI, per Design Pillar 2's remote-first UX principle.
- **Simultaneous 5-axis contouring** requires **Tool Center Point Control (TCPC / RTCP)**: keeping the programmed tool-tip path and orientation correct as the rotary axes reorient the tool, which is an inverse-kinematics transform, not just five independently-driven axes moving in parallel. This is the single hardest correctness requirement in the whole reference-workload set and is treated accordingly.

### What "no compromises" means, precisely

"No compromises" is a statement about **the quality and completeness of the motion/interpolation/kinematics core**, not a claim that TinyOS will re-implement every G/M-code and canned cycle a mature commercial controller has accumulated over decades — that scope claim would be dishonest and is explicitly avoided, consistent with how this project already handles the [Apple Silicon hardware caveat](universal-driver-model.md#the-apple-silicon-constraint-stated-plainly): say plainly what's committed and what isn't, rather than overselling.

What **is** the no-compromises commitment for MVP:

- The interpolation math (linear, circular, helical) is implemented in full double-precision, real trajectory blending across program blocks for corner smoothing — never a simplified or placeholder approximation "to be fixed later."
- Work coordinate systems and tool length/radius compensation are implemented as first-class, always-on features of the interpreter, not deferred conveniences — a program without them is not a realistic CNC program, so an interpreter without them is not a real CNC interpreter, MVP or not.
- The RTCP/TCPC kinematics transform for **one committed 5-axis geometry** (a trunnion-table configuration: two rotary axes plus three linear axes, chosen as the most common general-purpose configuration) is implemented fully and correctly for MVP — not stubbed to "3-axis only for now." Additional kinematic configurations (head-head, head-table, etc.) are added later as new kinematics modules per the Open/Closed pattern, without touching the interpolation core — this is deferred scope, not deferred correctness.
- The closed-loop control structure (position error computation, feedback consumption via the `PositionFeedback` trait) **is committed to be** real and exercised end-to-end against simulated feedback in Tier 0 testing — not a mock that gets thrown away when real encoders arrive. **It is not built yet** (2026-07-30): the delivered layer is the group-frame contracts and transport double of [`FEAT-P1-08`](../goals/features/FEAT-P1-08.md); the coupled control loop itself is the delivery contract's `MFS-05` and later.

What is **explicitly deferred**, and why that's a hardware question, not a software compromise:

- **Physical positional accuracy validation** — this requires real encoders, real drives, and a real machine structure (backlash, thermal drift, structural compliance) bolted on; no software architecture decision can substitute for that measurement, and claiming otherwise would itself be a compromise of a different kind (pretending a simulated result is a validated one).
- **The full canned-cycle library and G/M-code surface** a mature commercial controller accumulates over years of field use — MVP targets the core motion/coordinate/compensation feature set above, with the class-driver/extension pattern from the Universal Driver Model as the template for how additional cycles get added later without becoming a fork risk.
- **Additional 5-axis kinematic configurations** beyond the one committed trunnion-table geometry.

### Hardware note

Per [`SeedMVP.md`](../SeedMVP.md#5-narrowing-to-the-mvp-configuration), the MVP hardware pair (Jetson Orin Nano Super + x86_64 mini-PC) validates the RT scheduler, interpolation service, and ACI integration against **simulated axes and simulated encoder feedback** at Tier 0/1. A 5-axis motion I/O peripheral (drive command interface — step/dir, analog velocity command, or a digital servo protocol, plus real encoder input) is real hardware to be bolted on afterward, per the explicit "we will bolt on real hardware, feedback from encoders etc." scoping — consistent with how the Position Feedback Abstraction above is designed specifically so that step doesn't require touching the interpolation/kinematics core.

## Workload 2: Wire DED robot arm

- Uses the same Multi-Axis Motion & Interpolation Service with a serial-link (typically 6-DOF) arm kinematics module instead of the CNC's trunnion-table module — direct evidence the interpolation core is geometry-agnostic, not CNC-specific code that happens to also sort of work for a robot arm.
- The Process-Synchronized Output Service drives wire feed rate and energy-source power as functions of instantaneous path velocity — deposition rate control, where slowing down at a corner must proportionally reduce energy/wire feed or the deposited bead becomes inconsistent. This is a genuinely different process-synchronization pattern from the CNC's spindle/coolant control (which is closer to simple mode-based on/off with speed override) and from the resin printer's array timing (which is event/window-based, not continuous-velocity-based) — exercising the Process-Synchronized Output Service's generality across three distinct synchronization patterns.
- The safety interlock primitive gates the energy source on an "axes actively and correctly interpolating" signal — a stalled, faulted, or out-of-tolerance arm must cut deposition energy, generalizing the hardware e-stop pattern to a process-specific interlock.

## Workload 3: Resin-curing UV array

- Minimal motion requirement (a single Z-axis lift), so this workload validates almost none of the kinematics-module machinery — deliberately, because its job is to stress the High-Channel-Count Synchronized Array Output primitive instead, which the other two workloads barely touch.
- The UV array (a grid or zone-based set of individually controllable UV sources) is driven with per-channel timing precision synchronized to the lift axis's motion and to a defined exposure window — the Process-Synchronized Output Service's "event/window-based" synchronization pattern, distinct from the CNC's mode-based pattern and the Wire DED arm's continuous-velocity-based pattern.
- The safety interlock primitive gates the UV array on a "valid exposure window" signal, so uncontrolled or out-of-window light exposure (a real safety and print-quality concern for resin printing) is structurally prevented rather than relying on application-level discipline.

## Why three workloads, not one

Each workload deliberately stresses a different combination of the four shared primitives, which is what makes "we should be able to run all of these workloads on TinyOS" a validated architectural claim rather than an assumption:

| Workload | Kinematics complexity | Process-sync pattern | Array/channel load | Interlock pattern |
|---|---|---|---|---|
| 5-axis CNC | High (RTCP/TCPC, 5-axis) | Mode-based (spindle/coolant) | Low | Hardware e-stop (existing pattern) |
| Wire DED arm | Medium-high (6-DOF serial) | Continuous-velocity-based (deposition rate) | Low | Motion-active interlock (new pattern, generalized) |
| Resin-curing UV array | Trivial (1-axis lift) | Event/window-based (exposure timing) | High (many channels) | Exposure-window interlock (new pattern, generalized) |

A design that only had to serve the CNC case could plausibly hardcode CNC-specific assumptions into the RT core without anyone noticing until a very different workload came along and broke them. Specifying all three now, even though the CNC is the only one built out to full MVP depth immediately, is what keeps the shared primitives honest.

## MVP scope statement

- **In MVP scope, full depth**: the 5-axis CNC controller, per the "no compromises" commitment above, including its Fanuc-class operator experience, its trunnion-table RTCP kinematics module, and its exercise of the Motion & Interpolation Service, Process-Synchronized Output Service, and Position Feedback Abstraction.
- **In MVP scope, architecture-validated but not built to full production depth**: the Wire DED arm and resin-curing UV array workloads are used to validate that the shared primitives (particularly the Process-Synchronized Output Service's continuous-velocity and event/window synchronization patterns, and the High-Channel-Count Array Output primitive) are genuinely general — each gets a representative test/reference implementation sufficient to prove the primitive holds, without carrying the full operator-experience and safety-certification depth the CNC workload does at MVP. Growing either to full production depth is a deliberate follow-on, not blocked by anything architectural.
- **Explicitly deferred, hardware-dependent**: physical positional/deposition/cure-quality accuracy validation for all three workloads, pending the real motion I/O, encoder, wire-feed, energy-source, and UV-array peripheral hardware referenced in [`SeedMVP.md`](../SeedMVP.md) as bolt-on additions to the MVP compute hardware.

## Status

This document accompanies Section 3.1 (Physical AI Goals) and Section 5 (MVP narrowing) of [`SeedMVP.md`](../SeedMVP.md), and is the concrete elaboration of the README's Physical AI deployment-mode references. It will be revised as the kinematics modules, process-synchronization functions, and interlock wiring for each workload move from specification to implementation.
