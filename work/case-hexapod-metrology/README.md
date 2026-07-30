# Use Case: Hexapod / Parallel-Kinematic Surface Metrology

Status: **working use case — final motion-platform demonstration; not a current
capability or timing claim**

Governing performance gate:
[De-risking the 10 µs latency requirement](../Derisk10usLatencyRequirement.md)

Foundation:
[Foundational motion synchronisation delivery
contract](../case-motion-controller/foundational-motion-synchronisation-delivery.md)

## 1. Outcome

TinyOS is to coordinate a parallel-kinematic measurement platform whose central
moving disc carries a Renishaw measurement probe. Multiple telescopic actuators move
the same end effector. Every actuator's position and velocity feedback, plus the
probe's calibrated deflection, must be accepted as one coherent epoch. The controller
then:

1. reconstructs the measured platform and probe pose;
2. compares it with a CAD-defined path and datum;
3. calculates the surface-following correction;
4. resolves that correction into synchronized actuator position/velocity/effort
   commands;
5. stages the complete group command for one atomic apply event;
6. moves to a declared group-safe state if the epoch, geometry, deadline, probe, or
   output is invalid.

The final use case targets a strict physical feedback-latch-to-drive-command-latch
latency below 10 µs. That result is not assumed from an operating-system benchmark or
an EtherCAT synchronization number; it is accepted only under the evidence contract
in the governing latency document.

## 2. Machine definition: resolve “three arms” and “Hexapod”

A machine's degrees of freedom come from independently actuated constraints, not from
the number of encoders.

### Variant A — the described three-arm platform

The session's machine has:

- three telescopic arms;
- one drive motor per arm;
- two encoder channels per arm;
- ball joints at the moving disc;
- a probe on the disc.

Three independently actuated leg lengths ordinarily command three degrees of freedom.
This design is valid as a 3-DOF translational parallel mechanism only if orientation
and the remaining freedoms are mechanically constrained by a guide, linkage, flexure,
or equivalent structure. Otherwise the pose is underconstrained. Adding a second
encoder to an arm improves observability, redundancy, backlash/load estimation, and
fault detection; it does not add an independently controllable degree of freedom.

Capacity:

```text
3 drive axes
6 axis-feedback channels (2 per arm)
3 probe deflection channels (P/Q/R)
-----------------------------------
3 axes / 9 feedback channels
```

This fits the TinyOS maximum of 16 axes and 32 feedback channels.

### Variant B — a true six-leg Hexapod

A general 6-DOF platform requires six independently controlled leg lengths in a
non-singular geometry. With two encoder channels per leg and a three-channel probe:

```text
6 drive axes
12 axis-feedback channels (2 per leg)
3 probe deflection channels (P/Q/R)
------------------------------------
6 axes / 15 feedback channels
```

This also fits the TinyOS maximum. The same contracts and control cycle apply, with a
six-component platform twist instead of the three-axis fixed-orientation reduction.

### Relationship to Renishaw Equator-X

The user's mechanism is a project concept, not a claim about the internal design of a
Renishaw product. Renishaw describes Equator-X as a Hexapod structure with six linear
motors, independent drive and metrology frames, and a RESOLUTE absolute encoder on
each metrology strut. It uses an SP25M scanning probe. See Renishaw's [Equator-X
product description](https://www.renishaw.com/en/equator-x-dual-method-gauge-for-shop-floor-inspection--49752).

That separation is an important design lesson: a motor-side encoder and a load-side
encoder can reveal transmission error, but two sensors attached to the same deforming
structure do not create an independent metrology frame. Structural bending that
neither sensor observes cannot be corrected in software.

The first mechanical architecture gate must therefore freeze:

- three constrained translational DOF or six general pose DOF;
- the constraint mechanism and permitted orientation range;
- base, drive, metrology and moving-platform load paths;
- where each encoder measures and what deformation lies outside it;
- actuator travel, velocity, acceleration, jerk and force limits;
- admitted workspace, payload and probe/stylus configurations.

## 3. Hardware and control partition

### Per actuator

The preferred dual-feedback interpretation is:

1. **motor-side encoder** — commutation/rotor or screw-side measurement used by the
   drive's fastest current and velocity loops;
2. **load/metrology absolute encoder** — physical telescopic-leg length, independent
   of motor-to-load backlash and compliance where practical.

Velocity may arrive as a drive-produced value, but the metrology velocity should also
be derived from equidistant, common-clock absolute position samples and filtered by a
bounded estimator. A GPIO edge counter is suitable only when its electrical,
frequency, timestamp and loss behavior are proven. Raw analog probe signals require
deterministic simultaneous-sampling ADC hardware; a general-purpose DAC board is an
output device and is not a substitute for encoder capture or ADC acquisition.

Each physical channel requires:

- differential receiver/line-driver compatibility and isolation as appropriate;
- deterministic latch/capture, not user-space polling;
- hardware timestamp or known relationship to the common epoch;
- static scale, sign, offset, wrap and quality metadata;
- open/short, illegal transition, signal-loss and overrange detection;
- shield, grounding, EMC and machine-safety design.

### Probe

A touch-trigger probe is suitable for discrete point capture. Continuous “hug the
surface” control requires a scanning probe that continuously reports deflection.
Renishaw describes SP25M as producing continuously varying analogue deflection signals
to which the controller responds in real time. See [SP25M
operation](https://www.renishaw.com/cmmsupport/knowledgebase/en/sp25m-operation--11445).

Its raw P/Q/R channels are not assumed to be Cartesian XYZ. Renishaw documents
non-linear, non-orthogonal behavior and a third-order calibration model. See [SP25M
calibration](https://www.renishaw.com/cmmsupport/knowledgebase/en/sp25m-calibration--11444).
The controller must also distinguish return-to-zero, minimum usable deflection,
normal operating range, overrange and lost contact. See [SP25M technical
terms](https://www.renishaw.com/cmmsupport/knowledgebase/en/sp25m-technical-terms--11447).

### Responsibility split

| Layer | Owns |
|---|---|
| Drive hardware | Commutation, current/torque loop, fastest local velocity loop, power-stage protection |
| Edge/FPGA when required | Common-clock input latch, fastest bounded correction, deterministic physical output latch |
| TinyOS motion service | Trajectory, platform observer, cross-axis coupling, inverse/differential kinematics, group state, safe commit |
| Metrology application | CAD feature/path, datum selection, inspection strategy, result interpretation |
| Independent safety chain | Emergency stop, STO, hard limits and safety-rated functions required by the machine risk assessment |

For a physical sub-10-µs coupled correction, the first two rows may share an
accelerator. TinyOS still defines the admitted trajectory, calibrated model, limits,
mode, synchronization contract and evidence; the location of the arithmetic is an
architectural choice, not a reason to move the safety or group semantics out of the
system.

## 4. Coordinate and calibration model

At least these frames are required:

| Frame | Meaning |
|---|---|
| `B` | machine/base frame containing fixed leg anchors |
| `P` | moving-platform/disc frame containing moving leg anchors |
| `S` | stylus/probe frame |
| `W` | workpiece/CAD datum frame |
| `M` | independent metrology frame, if the machine has one |

Notation `T_A_from_C` means the transform that expresses a point from frame `C` in
frame `A`. The established workpiece datum supplies `T_B_from_W`. Probe qualification
supplies `T_P_from_S` and the calibrated stylus-tip offset.

The disc centre is only an intermediate state. The position that matters to
measurement is the calibrated contact point:

```text
p_tip_B =
    p_platform_B
  + R_platform_B * s_tip_P
  + calibrated_probe_deflection_B

p_surface_B =
    p_tip_B
  - stylus_ball_radius * contact_normal_B
```

The full measurement model must include:

- base and platform anchor calibration;
- encoder scale, zero and cyclic error;
- probe head, module, stylus and tip qualification;
- P/Q/R-to-Cartesian deflection calibration;
- workpiece datum transform;
- thermal compensation and validity range;
- compliance and dynamic compensation only where observable and evidenced.

A transform or calibration version is immutable during an active motion group. A
change creates a new admitted configuration and requires an explicit stopped-state
transition.

## 5. Kinematic model

Let:

- `b_i ∈ R³` be fixed base anchor `i`, expressed in `B`;
- `a_i ∈ R³` be moving-platform anchor `i`, expressed in `P`;
- `p ∈ R³` be platform origin position in `B`;
- `R ∈ SO(3)` be platform orientation in `B`;
- `l_i ∈ R³` be leg vector;
- `q_i` be actuator/metrology leg length;
- `u_i` be the unit leg vector.

For each leg:

```text
l_i = p + R a_i - b_i
q_i = ||l_i||
u_i = l_i / q_i
```

These equations are the position-level inverse kinematics: given a desired platform
pose `(p_d, R_d)`, calculate every desired leg length `q_i,d`.

If the desired probe-tip pose is specified first:

```text
p_platform,d = p_probe,d - R_d s_tip_P - deflection_compensation
q_i,d = ||p_platform,d + R_d a_i - b_i||
```

This is calculated for the complete group, not independently by six unrelated axis
loops.

### 5.1 Differential inverse kinematics

Let platform linear velocity be `v` and angular velocity be `ω`. Differentiating each
leg length gives:

```text
qdot_i =
    u_iᵀ v
  + (R a_i × u_i)ᵀ ω
```

Stacking all legs:

```text
qdot = J(x) [v; ω]
```

Each row of the general six-leg Jacobian is:

```text
J_i = [ u_iᵀ   (R a_i × u_i)ᵀ ]
```

For the mechanically constrained three-arm translation-only variant, `R` is fixed and
`ω = 0`, so:

```text
qdot = J_translation(p) v
J_translation row i = u_iᵀ
```

Acceleration feed-forward is:

```text
qddot = J xddot + Jdot xdot
```

Velocity and acceleration commands must therefore be generated from the desired
platform path as well as corrected by feedback. Sending only target leg positions
would discard the requested feed/speed profile and increase following error.

### 5.2 Forward kinematics and state estimation

Measured leg lengths do not directly report the probe pose. The controller solves:

```text
r_i(x) = ||p + R a_i - b_i|| - q_i,measured = 0
```

For a general Hexapod, use the previous accepted pose as the seed and a bounded
damped-Newton step:

```text
Δx = -(Jᵀ W J + λ² I)⁻¹ Jᵀ W r
x_next = x + Δx
```

Production constraints:

- fixed maximum iteration count;
- no heap allocation;
- preallocated matrices with checked finite arithmetic;
- previous accepted pose and admitted assembly branch as the seed;
- residual threshold and explicit non-convergence result;
- bounded damping schedule;
- condition-number and determinant/singular-value guard;
- no continuation with a guessed pose after failure.

The three-arm fixed-orientation case reduces to a bounded three-variable solve.
Velocity estimation fuses common-clock positions with the selected drive velocity:

```text
qdot_load,i = bounded_filter((q_load,i[N] - q_load,i[N-1]) / Δt)
transmission_error_i = q_load,i - scale(q_motor,i)
```

The observer may estimate backlash, compliance or bias only inside an identified and
admitted model. An estimator residual outside its envelope faults or degrades the
group; it is not silently absorbed as a correction.

### 5.3 Singularity and branch protection

Inverse kinematics returning finite lengths does not prove the pose is controllable.
Before and during motion, check:

- Jacobian rank and configured condition threshold;
- leg minimum/maximum length;
- joint angular limits;
- actuator velocity, acceleration, jerk, force and following error;
- collision between legs, joints, disc, probe, workpiece and fixtures;
- permitted assembly branch;
- forward-kinematic residual;
- calibrated workspace and thermal envelope.

The CAD path is admitted offline against these constraints. Runtime probe correction
is confined to a bounded path tube that was included in admission. Approaching a
conditioning or workspace limit causes controlled deceleration/hold before numerical
failure.

## 6. Worked three-arm calculation

This numerical example makes the mapping concrete. It is a geometry test vector, not
a proposed production machine.

Assume:

- orientation is mechanically fixed;
- base anchors form an equilateral triangle of radius `300 mm`;
- platform anchors form an aligned equilateral triangle of radius `100 mm`;
- platform origin begins at `p = [0, 0, 400] mm`.

Anchors:

```text
b1 = [ 300.0000,    0.0000, 0] mm    a1 = [100.0000,   0.0000, 0] mm
b2 = [-150.0000,  259.8076, 0] mm    a2 = [-50.0000,  86.6025, 0] mm
b3 = [-150.0000, -259.8076, 0] mm    a3 = [-50.0000, -86.6025, 0] mm
```

Initial inverse kinematics:

```text
l1 = [-200.0000,    0.0000, 400.0000] mm
l2 = [ 100.0000, -173.2051, 400.0000] mm
l3 = [ 100.0000,  173.2051, 400.0000] mm

q1 = q2 = q3 = 447.213595 mm
```

Request a probe/platform translation:

```text
Δp = [1.0, -0.5, 0.2] mm
```

The resulting leg targets are:

| Leg | Initial `q` (mm) | Target `q` (mm) | `Δq` (mm) |
|---:|---:|---:|---:|
| 1 | 447.213595 | 446.946630 | -0.266966 |
| 2 | 447.213595 | 447.810780 | +0.597185 |
| 3 | 447.213595 | 447.423831 | +0.210236 |

The end effector therefore does not move by sending the same increment to every leg.
All three commands are correlated outputs of one desired Cartesian change.

At the initial pose, the unit-leg rows are:

```text
u1 = [-0.44721360,  0.00000000, 0.89442719]
u2 = [ 0.22360680, -0.38729833, 0.89442719]
u3 = [ 0.22360680,  0.38729833, 0.89442719]
```

For a desired platform velocity:

```text
v = [20, -10, 4] mm/s
```

the differential inverse kinematics produce:

| Leg | `qdot_i = u_iᵀv` (mm/s) |
|---:|---:|
| 1 | -5.366563 |
| 2 | +11.922828 |
| 3 | +4.176861 |

The trajectory generator supplies position, velocity and preferably acceleration/jerk
references. Feedback closes the error dynamically:

```text
e_pose       = pose_desired - pose_estimated
e_twist      = twist_desired - twist_estimated
twist_cmd    = twist_feedforward + Kp_pose e_pose + Kd_pose e_twist
qdot_cmd     = J(pose_estimated) twist_cmd
q_cmd        = IK(pose_corrected)
```

The axis command for each leg may then include load-position, motor/load
transmission-error and effort terms:

```text
u_i =
    feedforward_i
  + Kq_i    (q_i,d    - q_i,load)
  + Kqd_i  (qdot_i,d - qdot_i,load)
  + Kt_i   transmission_error_i
  + coupled_group_correction_i
```

Limits, anti-windup and drive mode are explicit configuration. The complete vector
`u = [u_1 ... u_n]` is accepted or rejected atomically.

## 7. CAD path and probe surface following

The metrology application provides at each path parameter `s`:

- desired surface/contact point `p_CAD(s)` in `W`;
- unit surface normal `n_CAD(s)`;
- tangential feed velocity `v_CAD(s)`;
- allowed path tube, curvature and speed/acceleration limits;
- inspection feature and datum identity.

After transforming into `B`, let:

- `δ ∈ R³` be calibrated Cartesian probe deflection;
- `δ*` be desired normal deflection;
- `n` be the current unit surface normal;
- `P_t = I - n nᵀ` be the tangential projector.

Then:

```text
δ_n = nᵀ δ
e_δ = δ* - δ_n

v_normal =
    clamp(
        Kp e_δ
      + Ki integral(e_δ)
      - Kd derivative(δ_n),
        -v_normal,max,
        +v_normal,max)

v_probe,cmd = P_t v_CAD + n v_normal
```

This is hybrid control:

- tangential position/velocity follows the admitted CAD path;
- normal admittance/deflection control maintains measuring contact;
- inverse/differential kinematics convert the corrected probe motion into one
  synchronized leg command.

Required state logic:

| Probe state | Meaning | Motion response |
|---|---|---|
| Clear | Below contact threshold; intentional approach state | Bounded approach only |
| Contact valid | Above minimum deflection and inside calibrated working range | Enable surface following |
| Return-to-zero transition | Signal is unloading toward rest | Do not treat as a new contact |
| Lost contact | Deflection falls below hysteretic threshold during scan | Bounded reacquire if admitted; otherwise decelerate/hold |
| Overrange | Outside calibrated/mechanical range | Immediate configured group-safe action |
| Invalid/stale | Missing channel, calibration, quality, or epoch | Reject whole epoch; no controller call |

The integral term is frozen or unwound during saturation, contact loss, hold and
mode transitions. Surface-normal discontinuities in tessellated CAD are smoothed or
rejected offline; an unbounded normal jump cannot enter the real-time path.

## 8. One synchronized control epoch

The logical cycle is:

```text
Distributed/common clock event for epoch N
        |
        +-- latch all motor-side encoders
        +-- latch all load/metrology encoders
        +-- latch probe P/Q/R and quality
        +-- latch drive status / safety-relevant process state
        |
        v
Validate complete epoch N
  identity + mandatory masks + sequence + age + quality + topology
        |
        v
Scale and calibrate
  raw counts -> leg lengths/velocities + probe Cartesian deflection
        |
        v
Estimate machine state
  forward kinematics + observer + residual/conditioning checks
        |
        v
Evaluate admitted CAD reference and surface controller
        |
        v
Calculate differential IK and coupled leg correction
        |
        v
Apply group limits, safety policy and command validation
        |
        v
Build one complete ActuationFrame for declared apply epoch
        |
        v
Atomic ownership transfer / timed hardware latch at every selected drive
```

Every input that can change the command is either part of epoch `N` or immutable
configuration. There is no mixture of one leg from `N`, another from `N-1`, and a
probe sample with an unrelated time. There is no partial per-axis output.

The apply-epoch policy must be explicit. If commands computed from epoch `N` are
applied at `N+1`, the cycle period is part of physical latency. If an edge controller
applies a correction inside epoch `N`, its hardware dependency and safety semantics
must be modeled and tested.

## 9. TinyOS programming assets and required corrections

### Foundation already present

The current `os/src/motion` work provides:

- compile-time capacity for 16 axes and 32 feedback samples;
- typed axis/feedback identity;
- complete `FeedbackFrame` and `ActuationFrame` concepts;
- whole-epoch validation;
- deterministic in-memory transport/conformance behavior;
- `no_std`, fixed-capacity, no-heap foundations.

This is a useful base for the case. It is not yet the physical controller.

### Required contract correction

The current model makes every `FeedbackSample` axis-owned, and the existing
“auxiliary” feedback role is not coupled by control. That cannot honestly represent a
probe whose P/Q/R signals change the command for every leg.

Add a typed source/ownership model, or equivalently separate fixed-capacity sections:

```text
FeedbackEpoch
  header: epoch, capture time, sequence, calibration/configuration generation
  axis_feedback[]:
    axis_id, source_id, role(motor/load/velocity/effort), value, quality
  end_effector_feedback[]:
    effector_id, source_id, role(probe_P/probe_Q/probe_R/force), value, quality
  group_feedback[]:
    group_id, source_id, role(metrology/thermal/process), value, quality
```

All selected sections are validated and accepted atomically. The motion task receives
only a `ValidatedMachineEpoch`, never a partially trustworthy raw frame.

Raw transport units remain explicit integer counts. A second bounded layer creates:

```text
CalibratedMachineState
  axis positions and velocities in physical units
  platform pose and twist with validity/residual
  calibrated probe deflection and state
  datum/calibration/configuration generation
  Jacobian condition and active limits
```

Transport validation, calibration/state estimation, control calculation and
actuation commit remain distinct so each can be tested and timed.

### Required implementation assets

| Asset | Purpose |
|---|---|
| `MachineGeometry` | Immutable anchors, frame transforms, travel/joint/workspace limits |
| `CalibrationSet` | Encoder/probe/datum/metrology coefficients and validity |
| `ValidatedMachineEpoch` | Complete coherent feedback capability |
| `PlatformState` | Pose, twist, residual, conditioning and observer validity |
| `AdmittedSurfaceSegment` | Bounded CAD path coefficients, normal, path tube and limits |
| `Kinematics` trait | Fixed-capacity FK, IK, Jacobian and fault result |
| `SurfaceController` trait | Probe/contact state and corrected Cartesian command |
| `CoupledGroupController` | Complete machine state to complete actuation frame |
| `TimedCommit` | Single-use command token for one declared physical apply event |
| `MotionEvidenceRecord` | Per-epoch timing, validity, state and fault outcome |

The real-time implementations must avoid parsing CAD, ESI, configuration, dynamic
allocation, unbounded iteration, logging formatting and general-purpose IPC in the
cyclic dependency chain. Commissioning builds immutable admitted artifacts before
motion starts.

## 10. Fault and recovery contract

| Detection | Same-cycle controller action | Group transition |
|---|---|---|
| Mandatory axis/probe sample absent, stale, duplicate or invalid | Do not call coupled controller; do not emit normal command | Configured hold/deceleration/fault |
| Motor/load encoder disagreement beyond admitted envelope | Bound/zero normal command as configured | Decelerate/hold, then fault if persistent |
| Forward-kinematic non-convergence or residual too large | Reject state | Controlled hold/fault |
| Jacobian condition threshold or workspace/path-tube breach | Clamp only inside admitted guard band | Decelerate before boundary; hold |
| Probe lost contact | Stop normal advance; bounded reacquire only if admitted | Reacquire state or hold |
| Probe overrange or invalid calibration | No normal scan command | Immediate configured safe action |
| Axis following error, drive fault or hard-limit indication | No partial compensation by other legs | Whole-group safe action |
| Missed release, calculation deadline or output commit | Discard late command | Whole-group safe action |
| EtherCAT/process-image count, sequence, DC or ownership error | Reject epoch/commit | Whole-group safe action |
| E-stop/STO | Software cannot inhibit | Independent power-safe action |

Recovery is an explicit state transition requiring:

- stopped/safe physical condition;
- valid complete feedback;
- resolved fault source;
- configuration/calibration generation match;
- authorized recovery capability;
- deliberate re-arm and, where needed, re-home/re-datum.

The arrival of valid feedback alone never restarts movement.

## 11. Latency architecture for this use case

The final requirement is `LAT-PHYS-10` from the governing document. Standard
100 Mbit/s EtherCAT remains a critical synchronization and machine-fabric building
block, but a normal feedback-frame/software/next-command-frame route has a minimum
serialization interval greater than the final deadline before computation.

The leading architecture to test is:

```text
                         non-critical configuration / supervision
TinyOS trajectory + ----------------------------------------------+
admitted CAD segment                                               |
       |                                                           v
       | coefficients, limits, mode, generation          EtherCAT machine fabric
       v
local deterministic edge/FPGA
  common-clock encoder + probe latch
  calibrated bounded state/control workload
  synchronized local drive command latch
       |
       +--> raw trace and state evidence back to TinyOS
```

An alternative shared-memory/PCIe accelerator can leave the coupled arithmetic in a
strictly isolated TinyOS task if measured handoff and computation fit the budget.
An all-gigabit EtherCAT G topology is a candidate only when every critical segment and
device supports it and the complete topology passes the physical test.

The first hardware experiment must capture the real start and end events. Selecting a
Raspberry Pi, Jetson-class board, FPGA, or drive by headline CPU rate is not evidence.
The exact CPU, secure firmware, interrupt path, memory, NIC/ESC and power/thermal
configuration must qualify.

## 12. Verification ladder

### Stage 1 — pure kinematic reference

- generated reachable poses round-trip through IK then FK;
- the numerical vector in §6 matches within declared tolerance;
- velocity finite differences match `J xdot`;
- analytic and numerical Jacobians correlate;
- singular, ill-conditioned, wrong-branch and unreachable cases are rejected;
- every solver has a fixed iteration bound.

### Stage 2 — probe and surface controller

- recorded P/Q/R calibration vectors map to expected Cartesian deflection;
- clear/contact/return-to-zero/lost-contact/overrange transitions pass;
- flat, curved and normal-discontinuity paths exercise the hybrid controller;
- saturation and anti-windup are deterministic;
- invalid probe data rejects the complete epoch.

### Stage 3 — deterministic plant simulation

- three-arm and six-leg plants;
- motor/load lag, backlash, compliance, noise and bias;
- one-leg disturbance produces coordinated corrections in the group;
- partial/stale epochs, sequence loss and command-commit loss;
- maximum 16-axis/32-feedback capacity even though the use case uses fewer channels.

### Stage 4 — synchronized HIL

- real process-image cadence and timestamp correlation;
- encoder/probe signal generators sharing the declared epoch;
- virtual/physical drives confirming command-latch events;
- injected working-counter, DC, DMA, IRQ, stale-signal and overrange faults;
- known-delay positive control proving the 10 µs detector trips.

### Stage 5 — low-energy machine commissioning

- STO and hardwired limits proven first;
- one actuator, then coupled group at reduced force/speed;
- geometry, polarity, scale, home and branch verification;
- probe approach/contact/retract over a reference artefact;
- safe-state and explicit recovery demonstrations.

### Stage 6 — final evidence run

- full admitted workspace/path workload;
- hostile concurrent system load;
- hot/cold and all permitted power modes;
- exact binaries/configurations/topology retained;
- raw common-clock `t_sample` and `t_apply` for every accepted cycle;
- zero unexplained misses and positive controls;
- separate accuracy, repeatability and traceable metrology results.

## 13. Definition of delivered

This use case is delivered only when:

1. the physical mechanism and controlled DOF are unambiguous;
2. all axis, end-effector and required group feedback share one validated epoch;
3. two encoder channels per actuator have explicit physical meaning and fault limits;
4. calibrated forward kinematics reconstruct platform/probe state;
5. inverse/differential kinematics preserve path, feed, speed, location and
   orientation where controlled;
6. the scanning-probe loop follows the CAD surface inside its admitted deflection/path
   tube;
7. the entire actuator command is staged and applied atomically;
8. every invalid-data, numerical, deadline, probe, drive and commit fault has measured
   group-safe behavior;
9. the exact platform passes the repository's real-time qualification policy;
10. every accepted cycle satisfies `LAT-PHYS-10` under the published evidence
    envelope;
11. metrology accuracy and repeatability pass their own traceable acceptance tests;
12. the evidence is reproducible from retained source, binary, configuration,
    topology and raw trace artifacts.

Until then, progress should be reported by the claim ladder in
[Derisk10usLatencyRequirement.md](../Derisk10usLatencyRequirement.md), not as a
finished sub-10-µs motion controller.

## 14. Source notes

Primary vendor references used to sanity-check the use case:

- Renishaw, [Equator-X dual-method gauge for shop-floor
  inspection](https://www.renishaw.com/en/equator-x-dual-method-gauge-for-shop-floor-inspection--49752)
  — six linear motors, independent drive/metrology frames, per-strut RESOLUTE
  encoders, SP25M and published traverse/measurement-mode descriptions.
- Renishaw, [SP25M
  operation](https://www.renishaw.com/cmmsupport/knowledgebase/en/sp25m-operation--11445)
  — continuously varying analogue deflection and real-time controller response.
- Renishaw, [SP25M
  calibration](https://www.renishaw.com/cmmsupport/knowledgebase/en/sp25m-calibration--11444)
  — non-linear/non-orthogonal behavior and calibration model.
- Renishaw, [SP25M technical
  terms](https://www.renishaw.com/cmmsupport/knowledgebase/en/sp25m-technical-terms--11447)
  — return-to-zero, minimum deflection, working range and overrange concepts.
- EtherCAT Technology Group, [EtherCAT
  technology](https://www.ethercat.org/en/technology.html) and [EtherCAT
  G](https://www.ethercat.org/en/ethercat-g.html) — process-on-the-fly,
  Distributed Clocks, standard link rate and gigabit extension.

These references validate product/interface facts only. They do not demonstrate
TinyOS timing, safety, accuracy, or compatibility with any named vendor device.
