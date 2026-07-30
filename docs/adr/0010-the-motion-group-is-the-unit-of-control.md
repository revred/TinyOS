# ADR 0010 — The Motion Group Is the Unit of Control; EtherCAT Is a Transport Implementation

Status: **Accepted** (2026-07-30)
Date: 2026-07-30
Introduced in: [`work/case-motion-controller/foundational-motion-synchronisation-delivery.md`](../../work/case-motion-controller/foundational-motion-synchronisation-delivery.md) §11, which requires this decision to be recorded before `MFS-01` starts

## Context

The 5-axis CNC flagship (`G-PA-8`) and the wider Ti64 motion platform require up to 16
controlled drive axes and up to 32 position/velocity feedback channels, with two or more
axes influencing the same end effector. [`docs/physical-ai-reference-workloads.md`](../physical-ai-reference-workloads.md)
is the design authority for that workload; the delivery contract in
`work/case-motion-controller/` turns it into a promotable work-package sequence.

Two architectural defaults were available, and the difference between them is not
stylistic:

1. **Axis-first.** Each axis is controlled independently — one feedback callback, one
   control loop, one command write per axis — and "synchronisation" is whatever the
   transport happens to provide. This is how hobby controllers and many
   independent-axis servo installations are built.
2. **Group-first.** The motion group is the unit of control: one coherent feedback
   epoch for the whole group, one coupled control calculation over that epoch, one
   atomic time-tagged command commit for the whole group. Axes are members of a group;
   they are never independently controlled while the group is in active control.

Axis-first cannot express the workloads that justify this platform: trunnion-table
RTCP/TCPC, gantry squaring, two drives on one linear axis, master/follower gearing, and
end-effector control all require the control calculation to see every relevant feedback
channel from the *same* instant and to emit every correlated command for the *same*
apply instant. An API that invokes one callback per axis reintroduces independent-axis
control through the back door, and no transport feature can repair that afterwards.

Separately, EtherCAT is the first deterministic motion transport, and EtherCAT concepts
(PDO offsets, object numbers, working counters, Distributed Clocks) exert constant
pressure to leak upward into control code that would then be unportable and untestable
without a bus.

## Decision

1. **The motion group is the unit of control.** The public motion contracts take and
   produce whole-group frames: `FeedbackFrame` (one epoch, all selected feedback
   channels, one validity mask) in, `ActuationFrame` (one apply epoch, all selected
   axis commands, one validity mask) out. A frame is accepted completely or rejected
   completely. There is no per-axis feedback callback and no per-axis "write now"
   escape path in the public motion boundary.
2. **State transitions are group transitions.** Fault, hold and recovery policy apply
   to the declared group. One axis may not remain actively commanded while the rest of
   its group has faulted, unless a deployment profile explicitly defines a degraded
   group and proves that transition safe.
3. **EtherCAT is a transport implementation, not the architecture.** The coupled
   control code depends only on the `MotionGroupTransport` contract. PDO indices,
   EtherCAT object numbers, working counters, Distributed Clock details and NIC
   specifics stay inside the transport implementation. The deterministic in-memory
   simulator is the first implementation of the contract; EtherCAT is the first
   physical one; both run the same conformance suite (the Liskov rule in
   [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md) applied to a trait
   with two implementors).
4. **Ti64 does not absorb the drive's inner loops.** Commutation, current/torque loops,
   fast velocity loops, encoder electrical interfaces and STO behaviour belong to the
   intelligent drive. The physical e-stop/STO chain is wired outside TinyOS; software
   observes it and may never be the mechanism that makes it effective.

## Consequences

- The first promotable increment (`MFS-01` + the minimal `MFS-03` double, promoted as
  [`FEAT-P1-08`](../../goals/features/FEAT-P1-08.md)) delivers typed group/axis/
  feedback/epoch identities, fixed-capacity frames, mandatory-mask and epoch-order
  validation, the narrow transport contract, and a deterministic in-memory transport
  double — with no EtherCAT, no NIC and no physical I/O. Control semantics get proven
  before any fieldbus exists.
- A conforming `CoupledController` receives the full `FeedbackFrame` and produces the
  full `ActuationFrame`. Reviewers reject per-axis control APIs at the public motion
  boundary as a violation of this ADR, not as a style preference.
- Commissioning-plane work (ESI XML, SDO configuration, topology admission) is never
  parsed in the RT cycle. It produces a fixed runtime process-image plan; changing that
  plan requires leaving `Running` and re-admitting the group (`BND-03`, `PD-12`: no
  complex hostile-format parser in the cyclic path).
- Timing claims are governed by the existing evidence machinery: `ADR 0005`'s
  qualification rule applies unchanged, and nothing in the motion foundation may state
  a cycle-period, skew, age, WCET or jitter figure without the claim ladder in the
  delivery contract (`Designed` → `Code-live` → `Tier-0-live` → `HIL-live` →
  `Hardware-live` → `Timing-qualified` → `Production-ready`).
