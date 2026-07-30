# FEAT-P1-08 — Motion-Group Synchronisation Foundation: Contracts Before Transport

Status: **In progress — `STORY-P1-08-01` and `STORY-P1-08-02` Verified at the host tier (data contracts, validation, the deterministic transport double, and typed axis/end-effector/group feedback ownership per `ADR 0011`/`R4`); every later work package (`MFS-02`…`MFS-11`: periodic release, full simulator, collector, executor, atomic commit on a timeline, process image, EtherCAT, NIC, CiA-402, HIL) is undecomposed and unclaimed, and the `LAT-PHYS-10` architecture gate is open (`LE-63`)**
Epic: [`EPIC-P1`](../epics/EPIC-P1.md)
Introduced in: [`work/case-motion-controller/foundational-motion-synchronisation-delivery.md`](../../work/case-motion-controller/foundational-motion-synchronisation-delivery.md), the delivery contract this Feature promotes; decision recorded in [`ADR 0010`](../../docs/adr/0010-the-motion-group-is-the-unit-of-control.md)

## Description

The Ti64 motion platform requires up to **16 controlled drive axes** and **32
position/velocity feedback channels**, with two or more axes influencing the same end
effector: one coherent feedback epoch for the whole motion group, one coupled control
calculation over that epoch, one atomic time-tagged command commit for the whole group.
[`ADR 0010`](../../docs/adr/0010-the-motion-group-is-the-unit-of-control.md) fixes the
architecture: **the motion group is the unit of control, and EtherCAT is a transport
implementation** behind a narrow contract, never the architecture itself.

This Feature is the first promotable increment of that delivery contract — `MFS-01`
plus `MFS-03`'s minimal conformance double: the `motion` crate with typed
group/axis/feedback/epoch identities, fixed-capacity `FeedbackFrame`/`ActuationFrame`
types, mandatory-mask and epoch-order validation, the `MotionGroupTransport` contract,
and a deterministic in-memory transport double proving stage-all-or-nothing, single-use
commit tokens and late-commit-fails-closed — **without EtherCAT, interpolation, a NIC,
a periodic scheduler binding, or any physical I/O**.

Per the backlog's standing note, `motion` gets its Feature under the Epic current when
active development starts — that is `EPIC-P1`, and the placement is deliberate beyond
timing: the epoch/commit semantics this Feature fixes are exactly the determinism
contract `EPIC-P1` exists to prove, and `MFS-02` (periodic phase-aligned release) lands
on `FEAT-P1-04`'s preemption/WCET machinery.

Under the standing 08A hardware-evidence mandate this Feature adds **no hardware claim
and no board work**; it was ordered by the owner on 2026-07-30 as the CNC foundation
delivery and is host-tier only by design.

## Crate(s) involved

`os/src/motion/` — new crate, `#![forbid(unsafe_code)]`, `no_std`, fixed capacity, no
allocator (it joins the no-heap gate's shipped-crate list from birth). Nothing else
changes: no `kernel`, `hal` or `xtask` implementation line is touched by `-01`.

## Depends on

- `FEAT-P0-03`'s fixed-capacity discipline and `hal::time`'s seam pattern as prior art
  (patterns, not code dependencies — the crate depends only on `core`).
- Later work packages depend on `FEAT-P1-04` (periodic release), `FEAT-P0-07`/`MFS-07`
  (bounded process-image handoff) and `LE-26`'s NIC reality (`MFS-09`); none of that is
  claimed here.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P1-08-01`](../stories/STORY-P1-08-01.md) | Motion-group data contracts: typed identities, fixed frames, mandatory-mask and epoch-order validation, the `MotionGroupTransport` contract, and the deterministic in-memory double | Verified (Host, 2026-07-30) |
| [`STORY-P1-08-02`](../stories/STORY-P1-08-02.md) | Typed feedback ownership — axis, end-effector, group/process — so the Hexapod probe and metrology channels share one validated epoch and the `R4` axis-cast is a driven rejection | Verified (Host, 2026-07-30) |

Later Stories are decomposed just-in-time from the delivery contract's `MFS-02`…`MFS-11`
table when each is started; pre-building the whole tree here would violate the
decompose-just-in-time rule.

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) ·
implementation **C1** · subject **C2, C3** · boundary tests **BND-03, BND-14, BND-15,
BND-17** · **PD-05, PD-07, PD-08, PD-12** · **RCG-01, RCG-13, RCG-14**.

The declared destination split (delivery contract §10) is C2 for the EtherCAT/NIC
device service and C3 for the signed motion/control task, with a bounded fixed-layout
process-image handoff between them and **no general IPC queue in the servo dependency
chain**. This Feature's first Story runs no task and creates no domain; what it fixes
now is the part of that split that lives in the data plane:

- **Feedback process images are hostile input.** They originate at a compromisable C2
  transport/device service. Every frame is validated whole — group identity, epoch
  order, mandatory mask, per-sample identity and quality — and an invalid epoch is
  rejected without touching the active command (`PD-12`).
- **No complex hostile-format parser in the cyclic path** (`BND-03`): frames are fixed
  layout and fixed capacity; ESI XML and every variable-length commissioning input are
  excluded from the RT plane by construction and stay out of this crate entirely.
- **Frames carry data, never authority** (`BND-14`, `RCG-01`): accepting a feedback
  frame or staging a command frame conveys no capability, and recovery from `Faulted`
  is an explicit capability-gated action outside this Story's scope.
- **Fixed capacity everywhere** (`BND-15`, `PD-08`): 16/32 are compile-time bounds;
  no path allocates, blocks, retries or grows.

## Exit criteria

1. The `motion` crate exists with typed identities, fixed frames, validation, the
   transport contract and the deterministic double, all delivered test-first — **done,
   `STORY-P1-08-01`**.
2. The remaining delivery-contract work packages are promoted as Stories under this
   Feature (or a successor Feature per re-decomposition) before any of their code is
   written; each carries its own contract row and Test document first.
3. No claim above `Code-live` on the delivery contract's claim ladder is made by this
   Feature; `Tier-0-live` and above belong to the Stories that produce the fixtures
   and Reports.

## Explicit non-goals

Restated from the delivery contract §12, binding here: no servo power stage, no
FOC/current control on the CPU, no GPIO-as-servo-network, no replacement of hardwired
e-stop/STO, no G-code interpreter, no kinematics geometries, no GPU/inference work in
an RT dependency, no cycle-period promise, no safety-certification claim from software
tests. EtherCAT, the NIC/DMA driver, CiA-402 and every hardware/timing claim wait for
their own Stories and for the 08A sprint's hardware priorities.

## Named debt this Feature raises

- **`LE-62`** — the motion foundation's hardware/transport dependency chain (periodic
  release binding, process image, EtherCAT MainDevice, NIC/DMA on a board that today
  has no NIC path per `LE-26`, CiA-402 drive, HIL rig) is explicit registered debt,
  not prose.
- `STORY-P1-08-01` selects `D21` (CAN, USB and field I/O), whose subsystem does not
  exist; that selection is stated open debt in
  [`assurance/open-debt.tsv`](../assurance/open-debt.tsv) per `LE-35`'s rule.
