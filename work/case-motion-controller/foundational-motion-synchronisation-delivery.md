# Foundational Motion Synchronisation — Delivery Contract

Status: **working delivery contract — promoted 2026-07-30: [`FEAT-P1-08`](../../goals/features/FEAT-P1-08.md)
under `EPIC-P1` with [`ADR 0010`](../../docs/adr/0010-the-motion-group-is-the-unit-of-control.md)
(the motion group is the unit of control; EtherCAT is a transport implementation).
§13's first increment — `MFS-01` plus `MFS-03`'s minimal conformance double — is
**Code-live** ([`STORY-P1-08-01`](../../goals/stories/STORY-P1-08-01.md), host tier, 51
tests Red-first, [`REPORT-2026-07-30-04`](../../goals/reports/REPORT-2026-07-30-04.md)).
Every other work package remains Designed; `LE-62` registers the full remaining
transport/hardware chain. No timing claim exists at any level of this document.**

## Purpose

This document turns the 5-axis CNC use case into the programming foundation required for
the wider Ti64 motion platform:

- up to **16 controlled drive axes**;
- up to **32 position/velocity feedback channels**;
- two or more axes allowed to influence the position and velocity of the same end
  effector;
- one coherent feedback epoch for the complete motion group;
- one coupled control calculation over that epoch;
- one atomic, time-tagged command commit for the complete motion group;
- EtherCAT as the first deterministic motion transport.

It is a delivery document, not an architecture claim. The current repository has useful
RT-kernel mechanisms, but it does not yet contain a `motion` crate, a `PositionFeedback`
implementation, a periodic task-release model, an EtherCAT stack, a NIC/DMA driver, a
coherent feedback-frame type, or an atomic multi-axis command type. Nothing below may be
labelled live until its stated evidence gate has passed.

The design authority remains
[`docs/physical-ai-reference-workloads.md`](../../docs/physical-ai-reference-workloads.md).
Work becomes committed only when promoted into the repository's
[Goal → Epic → Feature → Story → Test → Report](../../goals/index.html) chain.

---

## 1. Delivery outcome

The foundation is delivered when TinyOS can execute this cycle without allocation,
blocking, partial input acceptance, or partial output:

```text
EtherCAT/SYNC epoch N
        │
        ├── all selected drives sample their feedback
        │
        ▼
FeedbackFrame<32> at epoch N
        │  validate completeness, identity, age, order and quality
        ▼
Coupled motion-group state estimate
        │  programmed trajectory + RTCP/kinematics + limits + process state
        ▼
One group-level control calculation
        │
        ▼
ActuationFrame<16> for apply_epoch N+1
        │  stage completely or reject completely
        ▼
Atomic commit for apply_epoch N+1
        │
        └── all selected drives latch their command on the same epoch
```

The nominal first hardware cycle is **1 ms / 1 kHz**. This is a development target, not
a worst-case bound. A move to 500 µs, 250 µs, or 100 µs is permitted only after the
complete 16-axis process image, selected drives, exact topology, TinyOS computation and
qualified hardware evidence support it.

### Definition of success

The first end-to-end foundation demonstration shall show:

1. two physical axes participating in one correlated end-effector control problem;
2. two feedback channels per axis where the selected drives expose them;
3. a disturbance or following error on one axis causing a calculated, bounded correction
   to both affected axes;
4. coherent acquisition and atomic application proven from epoch records, not inferred
   from visually simultaneous movement;
5. the same software contract running a 16-axis/32-feedback deterministic HIL simulation;
6. incomplete, stale, late or topologically invalid cycles resolving to the declared
   motion-group safe state;
7. hardware e-stop and STO remaining independent of every software path.

---

## 2. Non-negotiable control boundary

Ti64 does not replace the safety-certified power electronics and fast inner loops of an
intelligent servo drive.

| Owned by the intelligent drive | Owned by Ti64 |
|---|---|
| Motor commutation and FOC | CNC block lookahead and trajectory generation |
| Current/torque loop | RTCP/TCPC and machine kinematics |
| Fast motor velocity loop | Coherent 16-axis motion-group state |
| Local motor/load dual-loop compensation | Cross-axis/end-effector coupling |
| Encoder electrical interface and decoding | Common-epoch validation and staleness policy |
| Over-current, temperature and voltage protection | Feed, path and process synchronisation |
| Drive following-error protection | Atomic group-command generation |
| STO input and drive-safe behavior | Group fault coordination and audited recovery |

The first Technosoft integration should connect the motor-side and load-side encoders to
each intelligent drive. Ti64 receives both feedback values cyclically if the selected
drive can PDO-map them. If it exposes only a locally fused result, that limitation is
recorded in the drive capability descriptor and must not be represented as 32 raw
feedback channels.

The physical e-stop and STO chain is always wired outside TinyOS. Software may observe
its state but may never be the mechanism that makes it effective.

---

## 3. Timing vocabulary

These quantities must never be collapsed into the word "fast":

| Quantity | Meaning |
|---|---|
| Cycle period | Time between successive motion epochs |
| Synchronisation skew | Difference between when selected drives sample or apply the same epoch |
| Feedback age | Time from physical sampling to the control calculation consuming it |
| Calculation WCET | Qualified upper bound for the group calculation on a named platform |
| Commit margin | Time remaining between completed staging and the selected apply epoch |
| End-to-end latency | Physical sample to physically applied corrective command |
| Jitter | Variation of any of the above across cycles |

EtherCAT Distributed Clocks provide a common device time and synchronous latch event.
They do not by themselves prove low end-to-end latency, low jitter, or adequate
calculation WCET.

### Epoch rule

The initial cyclic control law is deliberately one epoch delayed:

```text
sample N → validate N → calculate from N → stage N+1 → apply N+1
```

Every feedback and command record names its epoch. A command may never be silently
relabelled for a later epoch after missing its intended commit point.

Epoch wrap is a protocol event that must be handled explicitly. It must never make an
old frame appear current.

---

## 4. Required programming contracts

The following Rust-shaped declarations illustrate responsibilities and invariants. Exact
field encodings are decided test-first in the owning Story; this document does not
pre-commit an ABI.

### 4.1 Identity and time

```rust
pub struct MotionGroupId(/* bounded identity */);
pub struct AxisId(/* 0..16 */);
pub struct FeedbackId(/* 0..32 */);
pub struct Epoch(/* ordered, wrap-aware value */);
pub struct MotionTime(/* common time-domain value */);
```

Raw integer indices must not cross the public motion boundary. Axis, feedback, group and
epoch identity must not be accidentally interchangeable.

### 4.2 Feedback

```rust
pub enum FeedbackRole {
    MotorPosition,
    LoadPosition,
    Velocity,
    Auxiliary,
}

pub struct FeedbackSample {
    pub feedback_id: FeedbackId,
    pub axis_id: AxisId,
    pub role: FeedbackRole,
    pub position: Position,
    pub velocity: Velocity,
    pub quality: FeedbackQuality,
}

pub struct FeedbackFrame<const N: usize> {
    pub group: MotionGroupId,
    pub epoch: Epoch,
    pub sampled_at: MotionTime,
    pub valid_mask: FeedbackMask,
    pub samples: [FeedbackSample; N],
}
```

The frame, not an individual sample, is the input to control. The selected motion-group
profile declares which feedback bits are mandatory. A frame missing any mandatory bit
is invalid for active control.

`FeedbackQuality` must distinguish at least:

- valid;
- stale;
- missing;
- discontinuous;
- device fault;
- transport-invalid;
- identity/configuration mismatch.

### 4.3 Command

```rust
pub struct AxisCommand {
    pub axis_id: AxisId,
    pub mode: CommandMode,
    pub target_position: Position,
    pub target_velocity: Velocity,
    pub target_torque: Torque,
    pub limits: CommandLimits,
}

pub struct ActuationFrame<const N: usize> {
    pub group: MotionGroupId,
    pub based_on: Epoch,
    pub apply_epoch: Epoch,
    pub valid_mask: AxisMask,
    pub commands: [AxisCommand; N],
}
```

The selected profile declares the mandatory axis mask. An active-control commit with an
incomplete mask is forbidden. Position, velocity and torque fields may be used as
target, limit or feed-forward quantities according to the selected CiA-402 mode; their
presence does not imply that Ti64 owns every inner loop.

### 4.4 Transport

```rust
pub trait MotionGroupTransport {
    fn receive_epoch(&mut self) -> Result<FeedbackFrame<32>, TransportFault>;
    fn stage(&mut self, frame: ActuationFrame<16>)
        -> Result<CommitToken, TransportFault>;
    fn commit_at(
        &mut self,
        token: CommitToken,
        apply_epoch: Epoch,
    ) -> Result<(), TransportFault>;
}
```

Required invariants:

- `stage` accepts the entire frame or changes nothing;
- a `CommitToken` is single-use and tied to exactly one staged frame;
- `commit_at` cannot change the frame or its epoch;
- no axis has a separate public "write now" escape path;
- a late commit fails closed;
- feedback acquisition and command staging are bounded and non-blocking;
- an implementation cannot hide dropped or repeated process images.

EtherCAT is the first implementation of this interface. The deterministic simulator is
the first test implementation. The coupled control code depends only on the interface,
never on PDO indices, EtherCAT object numbers or a concrete NIC.

### 4.5 Coupled control

```rust
pub trait CoupledController {
    fn calculate(
        &mut self,
        reference: &TrajectoryReference<16>,
        feedback: &FeedbackFrame<32>,
    ) -> Result<ActuationFrame<16>, ControlFault>;
}
```

The full feedback frame is mandatory. An API that invokes one callback per axis would
reintroduce independent-axis control and is not conforming.

Implementations may include:

- trunnion-table RTCP/TCPC;
- gantry squaring;
- two drives contributing to one linear axis;
- master/follower gearing;
- mechanically coupled axes;
- robot-arm end-effector control;
- process axes such as feed or deposition rate correlated with path velocity.

All implementations run through one shared conformance suite.

---

## 5. Motion-group state and fault policy

The minimum state model is:

```text
Disabled → Initialising → Ready → Running → Holding
                                  │          │
                                  └──────────┴──→ Faulted
```

State transitions are group transitions. One axis may not remain actively commanded
while the rest of its declared group has faulted unless the deployment profile
explicitly defines a degraded group and proves that transition safe.

The following conditions must have typed, testable dispositions:

| Condition | Minimum disposition |
|---|---|
| Missing mandatory feedback | Reject epoch; enter declared hold/deceleration policy |
| Stale feedback | Reject epoch; never calculate from it |
| Out-of-order or repeated epoch | Reject without changing active command |
| Feedback discontinuity | Axis/group fault according to machine profile |
| Incomplete command frame | Refuse staging |
| Missed commit point | Do not retag; enter declared safe policy |
| EtherCAT working-counter mismatch | Reject process image; fault/hold group |
| Distributed Clock out of tolerance | Prevent or stop active synchronized control |
| Drive state leaves Operational/Enabled | Group transition, not a UI-only alarm |
| Calculation WCET/deadline breach | No new command commit; declared safe policy |
| Topology or PDO-layout change | Require re-admission; never adapt while Running |

"Safe hold" is a profile-defined family of behavior, not a universal synonym for
holding position. Depending on mechanics, safety may require controlled deceleration,
brake engagement, torque removal, or an independent STO action.

Recovery from `Faulted` requires an explicit, capability-gated action. Reappearance of
valid feedback is not by itself authority to resume motion.

---

## 6. Foundational feature sequence

The identifiers below are local work-package identifiers, not `goals/` IDs. Before
implementation starts, the selected package is promoted under the active Epic and gets
the mandatory Feature, Story, Test, assurance-contract and Report records.

| ID | Foundational feature | Deliverable | Depends on | Evidence gate |
|---|---|---|---|---|
| MFS-01 | Motion Group Data Contracts | Typed identity, units, masks, feedback/command frames, epoch ordering and compile-time capacities for 16/32 | Existing `hal::time` and fixed-capacity patterns | Host tests, layout checks, adversarial construction tests |
| MFS-02 | Periodic Phase-Aligned Release | Task period, phase, next release, absolute deadline and missed-release policy integrated with the scheduler | Existing preemption/WCET | Host tests plus Tier 0 timer-driven fixture |
| MFS-03 | Deterministic 16/32 Simulator | Fixed-capacity plant/drive simulator supporting lag, backlash, elasticity, noise, discontinuity and transport faults | MFS-01 | Shared transport conformance suite and repeatable Tier 0 traces |
| MFS-04 | Coherent Feedback Collector | Mandatory-mask, identity, sequence, age and quality validation producing one accepted or rejected group epoch | MFS-01, MFS-02, MFS-03 | Fault-injection tests for every rejection arm |
| MFS-05 | Coupled Group Executor | One trajectory/reference input plus one complete feedback frame produces one complete command frame | MFS-01–04 | Two-axis disturbance test and 16-axis WCET distribution |
| MFS-06 | Atomic Stage/Commit and Safe State | Single-use commit token, no partial output, missed-commit handling, group hold/fault transitions | MFS-04, MFS-05 | Positive and negative Tier 0 fixtures proving output did and did not occur |
| MFS-07 | Deterministic Process Image | Fixed, double-buffered or ownership-swapped process image between C2 transport driver and C3 motion task; no general message parsing in the cyclic path | MFS-01, existing protection-domain/shared-memory work | Tear/race/backpressure tests and bounded handoff measurement |
| MFS-08 | EtherCAT MainDevice Core | Frame/datagram engine, fixed PDO plan, working counter, state supervision, Distributed Clock correlation and SYNC0 scheduling behind `MotionGroupTransport` | MFS-06, MFS-07 | Host virtual-bus conformance plus malformed-frame tests |
| MFS-09 | ARM64 Interrupt and Ethernet Foundation | Pi 5/Orin timer interrupt, GIC path, selected NIC MMIO/IRQ/DMA driver, dedicated-port operation | Existing ARM64 boot/time work, MFS-07 | Target-compiled fixture, then board capture with fault injection |
| MFS-10 | CiA-402 Drive Adapter | Drive state machine, CSP first, explicit scaling, status/control words, selected feedback PDOs and static ESI-derived profile | MFS-08, selected drive documentation | Virtual drive tests, then one physical drive with two feedback inputs |
| MFS-11 | Synchronized HIL and Hardware Proof | 16/32 HIL load, two correlated physical axes, DC skew, age, latency, jitter, loss and safe-state evidence | MFS-01–10 | Dated Reports with raw captures on named, qualified platforms |

### Why this order

MFS-01 through MFS-06 prove the control semantics without a fieldbus. MFS-07 proves the
protection-domain handoff without tying it to one NIC. MFS-08 proves EtherCAT protocol
behavior without claiming hardware timing. MFS-09 and MFS-10 connect real hardware only
after both sides of the boundary have independent conformance tests. MFS-11 measures the
assembled system.

Starting with a NIC driver or PDO parser would make hardware progress visible sooner but
would leave the central synchronization invariant undefined. It would also make
EtherCAT-specific details leak upward into the coupled controller.

---

## 7. EtherCAT architecture

EtherCAT is split into two planes.

### 7.1 Commissioning plane — not in the servo loop

- bus scan and device identity;
- ESI ingestion and validation;
- PDO-layout selection;
- SDO configuration;
- drive parameterisation and mode selection;
- firmware/setup transfer where supported;
- topology admission;
- conversion of configuration into a fixed, signed runtime profile.

ESI XML and other variable-length vendor input are never parsed in the RT cycle. The
commissioning path produces a fixed runtime process-image plan. Changing that plan
requires leaving `Running` and re-admitting the group.

### 7.2 Cyclic plane — the servo transport

- prebuilt EtherCAT frame/datagram layout;
- fixed PDO offsets and scaling;
- cyclic input read and output write;
- working-counter validation;
- Distributed Clock observation/correction;
- SYNC0-aligned sampling and application;
- fixed topology and fixed memory;
- no allocation, blocking, retries, logging strings, XML, SDO or dynamic discovery.

The EtherCAT port is dedicated to motion. UI, remote control, telemetry, deployment and
ordinary IP traffic use another interface or remain outside the motion schedule.

### First supported drive profile

The first drive profile is:

- CoE / CiA-402;
- Cyclic Synchronous Position;
- one motor-side feedback input;
- one load-side absolute/position feedback input where both are PDO-visible;
- position actual, velocity actual, drive status and fault state;
- target position, target velocity/feed-forward and control word;
- Distributed Clock/SYNC0 operation;
- drive communication-loss behavior configured and verified;
- STO observed but never software-mediated.

PVT may be added as a buffered trajectory adapter for early movement demonstrations or
lower-bandwidth applications. It does not replace the cyclic motion-group contract and
must disclose its additional buffering horizon in end-to-end latency evidence.

---

## 8. Test-first acceptance suite

Each row becomes one or more formal `goals/tests/TEST-*` entries when its Story starts.

| Test | Given | Required result |
|---|---|---|
| Complete epoch | All mandatory 32 feedback bits, correct group/configuration and current epoch | Exactly one accepted `FeedbackFrame` |
| Missing feedback | Any mandatory bit absent | Whole epoch rejected; no controller call |
| Stale sample | Sample age exceeds profile limit | Whole epoch rejected and stale fault recorded |
| Repeated/out-of-order epoch | Previous or future-invalid sequence received | Rejected without changing the staged command |
| Identity mismatch | A feedback ID reports against the wrong axis or configuration hash | Rejected as configuration fault |
| Atomic staging | One of 16 commands invalid or absent | No command from the frame becomes staged |
| Single-use commit | A valid commit token is reused | Second commit refused |
| Late commit | Commit point has passed | Command not retagged or emitted |
| Coupled correction | Axis A disturbance changes common end-effector state | Controller produces the expected bounded correction for A and correlated axis B |
| WCET breach | Group calculation exceeds declared budget | No new frame committed; safe policy taken |
| Transport loss | Frame absent or working counter wrong | Group hold/fault within declared cycle count |
| DC excursion | Synchronisation error exceeds profile tolerance | Active synchronized operation prevented or stopped |
| Topology change | Drive disappears, appears or changes PDO layout while Running | No live reconfiguration; group leaves Running |
| 16/32 capacity | Maximum selected axes and feedback at declared cycle | No allocation/drop; distribution and margin reported |
| Hostile non-RT load | UI/network/storage/inference pressure at admitted limits | Motion distribution remains within the qualified bound or admission is refused |
| Safety independence | TinyOS is halted, wedged or disconnected | Hardware e-stop/STO remains effective |

Every safety detector requires a positive control showing it fires. A clean run alone is
not evidence that stale data, missed commits, working-counter errors or WCET breaches
would be detected.

---

## 9. Evidence and observability

Every motion run must identify:

- build/commit and selected deployment profile;
- board and platform-qualification record;
- NIC and driver version;
- drive model, firmware and configuration hash;
- ESI source hash and compiled PDO-layout hash;
- physical/virtual topology and selected axis/feedback masks;
- cycle period and apply-epoch rule;
- Distributed Clock offset/skew distribution;
- feedback-age distribution;
- calculation-time distribution;
- stage-to-commit margin distribution;
- end-to-end sample-to-applied-correction distribution where physically measurable;
- working-counter failures, dropped/repeated epochs and missed commits;
- every transition to Holding/Faulted and the reason;
- sample-buffer drops or measurement perturbation;
- whether the run is Host, Tier 0, HIL, Tier 1 or Tier 2.

The existing `kernel::measure` machinery should carry timing samples, and spoor records
should carry decisions/faults. Neither currently has enough fields to reconstruct a
motion epoch by itself; the motion Report schema must add group, epoch, configuration
hash and fault identity without putting variable-length logging in the RT path.

### Claim ladder

| Label | Minimum evidence |
|---|---|
| Designed | This document/specification only |
| Code-live | Implementation plus passing host conformance/adversarial tests |
| Tier-0-live | Target-compiled QEMU/Renode fixture and raw Report |
| HIL-live | 16/32 simulated-device transport at real cycle cadence |
| Hardware-live | Named board/NIC/drive capture with fault injection |
| Timing-qualified | Hardware-live plus platform qualification and admissible worst-case evidence |
| Production-ready | Timing-qualified plus machine safety, EMC, lifecycle and applicable certification evidence |

No lower label implies a higher one.

---

## 10. Assurance and containment mapping

When promoted, the parent Feature must map at least:

- Goals `G-PA-1`, `G-PA-2`, `G-PA-3`, `G-PA-4`, `G-PA-5`, `G-PA-6` and
  `G-PA-8`;
- `APP-04` Physical AI and motion-control suite;
- `LZ-01` bare-metal reflex and physical control;
- timing/performance domains for interrupt latency, dispatch, WCET, jitter, IPC/shared
  handoff, device I/O, safe state and loaded isolation;
- C2 for the EtherCAT/NIC device service and C3 for the signed motion/control task,
  without deriving scheduling priority from containment class;
- the selected `PD-*`, `SEC-*` and `BND-*` contracts for device DMA, bounded IPC,
  authority, hostile device/network input, temporal isolation, audit and teardown.

The C2/C3 split must not put a general IPC queue in the servo dependency chain. MFS-07
exists to provide a bounded, fixed-layout ownership handoff. The driver cannot block the
motion task, and the motion task cannot access arbitrary device MMIO/DMA.

Network-originated commands remain outside the servo loop. They may alter an admitted
trajectory, mode or override through ACI policy, but only the motion task produces the
cyclic `ActuationFrame`.

---

## 11. Repository integration checklist

Before MFS-01 starts:

- create the parent motion-foundation Feature under the active Epic;
- add its `goals/assurance/feature-contracts.tsv` row;
- create the first Story/Test pair before implementation;
- add `os/src/motion/` only with that Story's first real consumer and tests;
- update `os/Cargo.toml` in the same change;
- add the shared conformance-test harness before the second transport/controller
  implementor;
- decide an ADR stating **the motion group is the unit of control** and EtherCAT is a
  transport implementation;
- correct current documents/UI text that describe `PositionFeedback` and periodic motion
  tasks as live when they are not;
- extend `APP-04` and `LZ-01` to name coherent feedback epochs, atomic group commit and
  the EtherCAT motion fabric;
- register every remaining hardware/timing dependency as explicit debt rather than
  leaving it in prose.

At each completed work package:

- run formatting, linting, unit and conformance tests;
- run `cargo run -p xtask -- check-assurance-spine`;
- add the relevant Tier 0/HIL/hardware fixture;
- demonstrate the detector with a deliberate fault;
- publish a dated Report with raw evidence;
- update this document's status only to the evidence level actually reached.

---

## 12. Explicit non-goals of the foundation

This delivery does not:

- design a servo power stage;
- move FOC/current control into the Pi or Jetson;
- make GPIO a production 16-axis servo network;
- replace hardwired e-stop or STO;
- claim that one physical drive proves sixteen-drive timing;
- deliver the full G-code interpreter, canned-cycle library or operator UX;
- deliver every CNC kinematic geometry;
- make GPU/inference work part of an RT dependency;
- promise a 100 µs cycle because a selected drive advertises one;
- claim safety certification from software tests.

It delivers the stable synchronization and control foundation on which those higher
motion functions can be implemented and verified without changing the transport or
hardware boundary each time.

---

## 13. Immediate next delivery

The first promotable increment is **MFS-01 plus MFS-03's minimal conformance double**:

1. create the `motion` crate;
2. define typed group/axis/feedback/epoch identities;
3. define fixed `FeedbackFrame<32>` and `ActuationFrame<16>`;
4. define mandatory-mask and epoch-order validation;
5. define the narrow `MotionGroupTransport` contract;
6. implement a deterministic in-memory transport double;
7. write failing tests first for missing feedback, stale/repeated epochs, incomplete
   commands and partial commit;
8. make those tests pass without yet adding EtherCAT, interpolation or physical I/O.

That increment creates the first real programming asset for synchronization. MFS-02 then
places it on a periodic timeline; MFS-04 through MFS-06 turn it into a closed, safe
motion-group cycle; EtherCAT follows against a contract already proven independently.
