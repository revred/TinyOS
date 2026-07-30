# De-risking the 10 µs Latency Requirement

Status: **governing working risk-retirement contract — the requirement is preserved; the capability is not yet proven. Promotion 2026-07-30: WP0's software half and §13 items 1, 2, 6 and 8 are delivered — [`ADR 0011`](../docs/adr/0011-lat-phys-10-governs-and-the-two-event-100mbit-path-is-rejected.md) (vocabulary adopted, event contract frozen, Option A rejected on serialization physics, Option B leading, gate open), [`STORY-P1-08-02`](../goals/stories/STORY-P1-08-02.md) (typed axis/end-effector/group feedback ownership, `R4` closed in code, host tier), and the [§10 report template](../goals/reports/_lat-phys-10-report-template.md). Items 3, 4, 5 and 7 require hardware or the Hexapod solver and are registered open debt (`LE-63`). No timing has been measured; every claim remains capped at Code-live.**

Priority: **release-critical for the final motion-platform use case**

Related implementation work:

- [Hexapod / parallel-kinematic surface-metrology worked use case](case-hexapod-metrology/README.md)
- [Foundational motion synchronisation delivery contract](case-motion-controller/foundational-motion-synchronisation-delivery.md)
- [ARM64 real-time qualification decision](../docs/adr/0005-arm64-real-time-tier-is-conditional-on-secure-world-qualification.md)
- [Current motion feature status](../goals/features/FEAT-P1-08.md)

## 1. Executive decision

The final motion-platform use case is to target a strict:

> **less than 10,000 ns from a common-clock physical feedback latch to the
> physical latch or documented acceptance of the resulting complete corrective
> command by every selected drive in the synchronized group.**

This document calls that requirement **LAT-PHYS-10**. “Under 10 µs” means `< 10,000
ns`, not `≤ 10,000 ns`. It is an end-to-end deadline, not an average, a network
synchronization figure, or controller execution time measured in isolation.

TinyOS does **not** satisfy this requirement today. The repository has the beginning of
a deterministic, allocation-free 16-axis/32-feedback motion-group contract and host
tests. It does not yet have physical acquisition, periodic target execution, a real
motion process image, EtherCAT, kinematics, a probe controller, timed drive commit, HIL,
or target timing evidence.

One feasibility result must govern the architecture immediately:

- ordinary EtherCAT uses 100 Mbit/s Ethernet;
- the minimum line interval for one Ethernet frame is approximately **6.72 µs**;
- a controller that first receives newly latched feedback and then sends a newly
  computed command normally needs at least two transmission events;
- those two minimum intervals alone total **13.44 µs**, before forwarding, cable,
  DMA, task release, validation, calculation, output staging, and drive-latch delay.

Therefore a conventional two-event 100 Mbit/s EtherCAT software-control path must not
be represented as capable of LAT-PHYS-10. EtherCAT remains highly useful for common
time, coherent process images, commissioning, slower supervisory cycles, and drive
coordination. A genuinely sub-10-µs physical correction loop requires a different
latency-critical path, such as an FPGA/edge controller located with the feedback and
drive interfaces, unless measurement on a completely specified alternative topology
proves otherwise.

This finding does not weaken the requirement. It prevents the project from spending
months optimizing software around a physical lower bound that cannot meet it.

## 2. What the requirement means

The four timing properties below must not be substituted for one another.

| ID | Property | Start | End | Status |
|---|---|---|---|---|
| `SYNC-1` | Feedback/actuation synchronization skew | Earliest device latch | Latest device latch for the same epoch | Required; target to be allocated |
| `LAT-CALC-10` | Coupled-control calculation latency | Complete validated/calibrated machine state available | Complete accepted actuation frame staged | `< 10 µs` candidate software milestone |
| `LAT-OS-10` | OS/process-image latency | Device input image made available to TinyOS | Resulting output image handed to the hardware boundary | `< 10 µs` candidate OS milestone |
| `LAT-PHYS-10` | Physical closed-loop command latency | Common-clock physical feedback latch | Resulting command physically latched or accepted by all selected drives | **Final use-case requirement: `< 10 µs`** |
| `PERIOD-N` | Group cycle period | Epoch `N` latch | Epoch `N+1` latch | Separately configured and evidenced |
| `JITTER-X` | Variation | Minimum observed value | Maximum observed value for the named interval | Report distribution and maximum |

Passing `LAT-CALC-10` or `LAT-OS-10` is valuable, but neither is permission to claim
`LAT-PHYS-10`. EtherCAT Distributed Clocks synchronization below 1 µs is likewise not
an end-to-end loop-latency result.

Mechanical response is outside `LAT-PHYS-10`: the endpoint is the drive's documented
command latch/acceptance event, not detectable motor or end-effector movement.
Mechanical response, following error, and metrology accuracy receive separate
acceptance limits.

## 3. Clock and endpoint contract

Every result must identify these events on one traceable timebase:

| Symbol | Event |
|---|---|
| `t_sample_first` | First mandatory physical feedback channel latches epoch `N` |
| `t_sample_last` | Last mandatory physical feedback channel latches epoch `N` |
| `t_state_ready` | The complete epoch has passed identity, age, quality, topology, scaling, and calibration checks |
| `t_control_start` | Coupled controller begins for epoch `N` |
| `t_frame_ready` | Complete actuation frame for the selected apply epoch has passed safety and range checks |
| `t_hw_handoff` | Output ownership passes to NIC, FPGA, or equivalent deterministic hardware |
| `t_apply_first` | First selected drive latches/accepts the resulting command |
| `t_apply_last` | Last selected drive latches/accepts the resulting command |

The normative measurements are:

```text
feedback_latch_skew = t_sample_last - t_sample_first
calculation_latency = t_frame_ready - t_state_ready
os_latency          = t_hw_handoff - t_state_ready
physical_latency    = t_apply_last - t_sample_first
apply_skew          = t_apply_last - t_apply_first
```

For a passing sample:

```text
physical_latency < 10,000 ns
```

The measurement contract must also state:

- whether the sensor value is latched, sampled, decoded, or merely delivered;
- whether “drive acceptance” is an electrical latch, ESC register event, set-point
  consumption event, or vendor-defined internal event;
- the device and controller clock relationship and measured uncertainty;
- the selected group size and every channel in its mandatory masks;
- the exact consequence of a missing, stale, duplicate, invalid, or late sample;
- the configured command apply epoch;
- whether any device interpolates or buffers commands, and for how long.

No timestamp generated only after software receives a packet may be described as the
physical sampling time.

## 4. First-principles EtherCAT feasibility bound

EtherCAT's published architecture uses standard Ethernet frames at 100 Mbit/s and
processes data on the fly. The MainDevice is the only active sender. Distributed
Clocks provide precise synchronization; they do not remove serialization time or turn
a response received after a frame into a new command in that already transmitted
frame. See the [EtherCAT Technology Group's technology
description](https://www.ethercat.org/en/technology.html).

For minimum-size Ethernet traffic, the line interval comprises:

```text
8 bytes   preamble and start-frame delimiter
64 bytes  minimum Ethernet frame
12 bytes  inter-packet gap
--------
84 bytes = 672 bits
```

At 100 Mbit/s:

```text
672 bits / 100,000,000 bits/s = 6.72 µs
```

The frame and gap accounting is also documented in Intel's [small-packet Ethernet
performance application
note](https://www.intel.com/content/dam/doc/application-note/8255x-8254x-ethernet-controllers-small-packet-traffic-performance-appl-note.pdf).

For a feedback-dependent command calculated centrally, a conservative topology
lower-bound starts with:

```text
minimum feedback-carrying transmission interval  6.72 µs
minimum subsequent command transmission interval 6.72 µs
                                                  --------
                                                  13.44 µs
```

That does not yet include:

- cable propagation or physical-layer delay;
- EtherCAT Slave Controller forwarding;
- topology length and device count;
- MainDevice/NIC DMA and ownership transfer;
- interrupt, polling, or periodic-task release;
- epoch validation, calibration, state estimation, surface control, or kinematics;
- actuation-frame validation and staging;
- output forwarding and device set-point consumption.

Consequently:

> **A normal feedback-frame → software calculation → subsequent command-frame
> architecture on a 100 Mbit/s EtherCAT segment fails the LAT-PHYS-10 architecture
> gate before TinyOS execution time is considered.**

EtherCAT G offers gigabit operation and compatibility mechanisms, but 100 Mbit/s
devices remain on 100 Mbit/s segments. It helps only if the complete latency-critical
path—including feedback acquisition, couplers, drives, and controller interface—is
specified accordingly. See [EtherCAT
G](https://www.ethercat.org/en/ethercat-g.html).

This is an engineering lower bound, not a claim that EtherCAT is unsuitable for the
machine. EtherCAT may remain the machine fabric while the fastest correction is closed
at an edge controller or in each drive.

## 5. Architecture decision gate

The project must select and document one latency architecture before implementing the
physical transport.

| Option | Latency-critical path | TinyOS role | LAT-PHYS-10 disposition |
|---|---|---|---|
| A. Central 100 Mbit/s EtherCAT | Sensors → EtherCAT → TinyOS → EtherCAT → drives | Full group controller; drives own current/velocity inner loops | **Rejected for strict physical `<10 µs`**; valid for a slower group cycle |
| B. FPGA/edge closed loop | Local encoder/probe latch → bounded hardware controller → local drive command | Supplies trajectory coefficients, limits, mode, supervision, evidence collection | Candidate |
| C. Shared-memory accelerator | FPGA/ESC process image ↔ PCIe/shared memory ↔ isolated TinyOS task | Runs coupled calculation while hardware owns deterministic I/O/latches | Candidate, subject to measured handoff and compute WCET |
| D. All-gigabit deterministic fabric | Entire critical chain supports the selected high-speed EtherCAT G or alternative profile | Central or partitioned control | Candidate only after topology-level proof; no hidden 100 Mbit/s segment |
| E. Drive-local inner correction | Each drive closes fastest local feedback; TinyOS closes platform/cross-axis loop at a slower period | Trajectory, platform observer, cross-axis coupling, probe surface-following supervisor | Does not pass LAT-PHYS-10 for central cross-axis correction unless the local controllers share the coupled state |

### Default until the gate is resolved

Treat `LAT-PHYS-10` as governing and Option B as the leading test architecture. In
parallel, make `LAT-CALC-10` a TinyOS software milestone. This preserves TinyOS as the
motion-platform control foundation while putting the irreducible sub-10-µs mechanism
where physics permits it.

The decision record must name:

- controller board, CPU and accelerator;
- NIC/ESC, PHYs, couplers, topology and link rate;
- every drive, encoder interface and probe interface;
- feedback and command data widths;
- device latch and interpolation behavior;
- cycle, phase and apply-epoch model;
- fail-safe and recovery behavior;
- which controller owns current, velocity, position, platform and surface-contact
  loops.

Marketing preference, a vendor's nominal cycle time, or an average benchmark cannot
close this gate.

## 6. Candidate workload and timing budget

This worksheet is deliberately unallocated until the physical architecture is chosen.
Inventing microsecond allocations before the I/O path is known would create false
precision.

| Segment | Required evidence | Allocation | Observed maximum | Margin |
|---|---|---:|---:|---:|
| Physical feedback latch skew | Common-clock capture at all mandatory inputs | TBD | — | — |
| Inbound transfer/handoff | Hardware trace from latch to readable coherent epoch | TBD | — | — |
| Release/dispatch | Target trace under hostile interrupt and system load | TBD | — | — |
| Epoch validation and calibration | Target microbenchmark at maximum 16/32 capacity | TBD | — | — |
| Forward kinematics/state observer | Bounded implementation; worst valid conditioning | TBD | — | — |
| CAD/probe surface controller | Maximum admitted path and correction workload | TBD | — | — |
| Differential IK/coupled command | Maximum selected axes and constraints | TBD | — | — |
| Safety/range checks and frame construction | Complete group, including negative paths | TBD | — | — |
| Outbound handoff and physical apply | Common-clock capture at every selected drive | TBD | — | — |
| Measurement uncertainty and engineering reserve | Calibration and explicit reserve | TBD | — | — |
| **Physical total** | `t_apply_last - t_sample_first` | **< 10,000 ns** | — | — |

At the stated Equator-X traverse rates, an end effector moves:

```text
250 mm/s × 10 µs = 2.5 µm
500 mm/s × 10 µs = 5.0 µm
```

This explains why latency matters, but it does not establish accuracy. Metrology error
also includes frame calibration, encoder accuracy, probe calibration, structural
deflection, thermal effects, servo following error, interpolation and dynamic model
error.

## 7. Current TinyOS readiness

| Capability | Current truth | Required next evidence |
|---|---|---|
| Fixed capacities | Code-live for 16 axes and 32 feedback samples | Layout and target-capacity tests |
| Whole-group feedback/command contract | Code-live host implementation | Tier 0 and target conformance |
| Allocation-free motion path | Present in the initial `no_std` crate | Target audit and measurement |
| Periodic phase-aligned release | Not implemented for motion | `MFS-02` |
| Coherent physical acquisition | Not implemented | Process-image and hardware latch proof |
| Atomic timed physical command commit | Not implemented | `MFS-06`/`MFS-07` plus scope capture |
| EtherCAT MainDevice/NIC/DMA | Not implemented | `MFS-08`–`MFS-10` |
| Kinematics/state estimation | Not implemented | Bounded Hexapod solver and tests |
| Probe calibration/surface following | Not implemented | Sensor model, controller and fault tests |
| End-effector/group feedback model | **Contract gap:** feedback is presently axis-owned | Add typed end-effector and group/process feedback |
| HIL/hardware timing | Not implemented | `MFS-11` and this document's protocol |
| Qualified ARM64 hard-RT platform | None | ADR 0005 Q1–Q4 qualification |

The current feedback model binds every sample to an axis. A metrology probe, a
workpiece-frame sensor, and a group thermal sensor are not naturally “auxiliary axis”
feedback. The control contract must represent, in one atomic epoch:

1. axis-owned motor/load feedback;
2. end-effector-owned probe/force/deflection feedback;
3. group/process-owned metrology and environment feedback.

Using an “auxiliary” role that the controller does not couple on would make the
Hexapod implementation look complete while excluding the probe signal that closes the
surface-following loop.

## 8. Risk register

| Risk | Failure mode | Retirement action | Kill/decision rule |
|---|---|---|---|
| R1 Ambiguous endpoint | A fast internal function is reported as end-to-end latency | Freeze the event/clock contract in §3 | No endpoint contract, no latency claim |
| R2 Fieldbus lower bound | 100 Mbit/s serialization already exceeds the deadline | Measure topology; select edge/accelerated path | Reject the architecture, not the measurement |
| R3 Clock/trace error | Unsynchronized software timestamps hide transfer delay | Common-clock hardware markers and calibrated trace uncertainty | Uncertainty must be stated and fit inside margin |
| R4 Axis-only feedback schema | Probe/process feedback cannot participate in an accepted epoch | Extend the typed epoch before controller integration | No casting probe data into an unrelated axis |
| R5 Numerical WCET | Forward/inverse kinematics iteration runs long near singularity | Fixed iterations, admitted condition range, offline path checks | Reject/hold before solver becomes unbounded |
| R6 CPU jitter | Cache, FPU, DMA, IRQ or scheduler interference creates outliers | Isolation, bounded memory, hostile-load measurement | Any unexplained miss fails the run |
| R7 Secure-world interruption | Firmware/SMI-like event breaks the bound | Complete ADR 0005 qualification | No worst-case claim on an unqualified board |
| R8 Probe non-linearity | Raw P/Q/R values are treated as Cartesian contact | Third-order calibration and overrange/return-to-zero logic | Invalid calibration or state causes group-safe action |
| R9 Mechanical observability | Three-arm geometry is underconstrained or structurally unobserved | Freeze DOF and independent metrology architecture | No 6-DOF claim from a constrained 3-arm mechanism |
| R10 Drive semantics | “Command received” differs from set-point consumed | Obtain vendor latch/interpolation evidence and instrument it | Unspecified buffering remains inside the latency budget |
| R11 Safety shortcut | Checks are bypassed to win benchmark time | Measure production-equivalent safety path | Faster unsafe build is inadmissible evidence |
| R12 Observer effect | Logging changes the timing being measured | Hardware trace plus bounded telemetry comparison | Instrumentation overhead must be quantified |
| R13 Tail blindness | Average or percentile hides a deadline miss | Preserve every sample; report maximum and miss count | One unclassified deadline miss fails strict acceptance |

## 9. Delivery sequence

### WP0 — Freeze semantics and prove the lower bound

- adopt `LAT-PHYS-10`, `LAT-OS-10`, `LAT-CALC-10`, `SYNC-1`, and `PERIOD-N`;
- freeze physical start/end events and strict comparison;
- bench the selected link/topology before writing its driver;
- record an ADR selecting the latency-critical architecture.

Exit: the requirement has one unambiguous equation and the candidate architecture is
not already disqualified by transport physics.

### WP1 — Correct the motion information model

- add typed ownership for axis, end-effector, and group/process feedback;
- add calibrated physical quantities and frame transforms above raw encoder counts;
- preserve whole-epoch validation and mandatory masks;
- reject incomplete or invalid probe/axis epochs before calling control.

Exit: the full Hexapod sensor set can be represented and validated without semantic
aliases.

### WP2 — Build the bounded control workload

- implement forward kinematics/state observation;
- implement inverse and differential kinematics;
- implement Jacobian conditioning, workspace and actuator-limit checks;
- implement calibrated probe deflection and hybrid surface following;
- use fixed-capacity storage and a bounded number of solver iterations.

Exit: deterministic host vectors, singularity cases and fault cases pass at maximum
configured capacity.

### WP3 — Put the workload on a deterministic TinyOS timeline

- complete `MFS-02` through `MFS-07`;
- bind one admitted motion task to a phase-aligned periodic release;
- add explicit missed-release and missed-commit policy;
- measure the production-equivalent validation/control/safety path.

Exit: `LAT-CALC-10` and `LAT-OS-10` have target-board evidence or a documented,
actionable budget failure.

### WP4 — Close the physical loop

- implement the selected FPGA/ESC/NIC and drive interfaces;
- latch all axis and probe feedback on the declared epoch;
- apply one complete command atomically at the declared event;
- inject stale, missing, corrupt, late, overrange and working-counter failures.

Exit: synchronized HIL proves event meaning and safe behavior before motors are
energized.

### WP5 — Qualify and demonstrate

- qualify the exact ARM64/accelerator platform under ADR 0005;
- run the final Hexapod surface-following workload under hostile load and thermal
  conditions;
- publish raw traces, hashes, topology and positive-control results;
- run metrology correlation and safety validation separately from the latency proof.

Exit: every gate in §11 is satisfied.

## 10. Measurement protocol

Every formal run must record:

- date, operator and report identifier;
- source commit, compiler/linker versions and binary hashes;
- board, silicon revision, firmware, boot configuration and power mode;
- FPGA/ESC/NIC/PHY firmware and configuration hashes;
- network/device order, cable lengths, link rates and process-image layout;
- drive, encoder, probe and coupler model/firmware/configuration;
- exact selected axis and feedback masks;
- cycle, phase, apply epoch and drive interpolation settings;
- task/IRQ affinity, cache policy and all competing workloads;
- clock source, calibration, timestamp resolution and uncertainty;
- raw per-cycle timestamps, not only aggregates;
- minimum, median, percentile distribution, maximum and miss count;
- safety/fault outcomes for every injected invalid condition.

The evidence workload must include:

1. the maximum supported 16-axis/32-feedback transport and controller load;
2. the actual Hexapod configuration;
3. maximum admitted numerical conditioning and controller branches;
4. concurrent UI, telemetry, storage, network and inference pressure representative of
   the deployed system;
5. hot and cold thermal states and every permitted power mode;
6. a duration and cycle count justified against the claimed failure rate;
7. a **positive control** that injects a known delay beyond 10 µs and proves the
   detector reports failure;
8. a clock-skew or timestamp-corruption positive control;
9. at least one deadline miss demonstrating the commanded group-safe behavior.

A long soak can show that no miss was observed in the tested population. It does not
convert an empirical maximum into a mathematical bound. A worst-case claim additionally
requires the platform, scheduling, interrupt, DMA, memory and algorithm assumptions to
be bounded and qualified.

## 11. Final acceptance gates

The final use case is delivered only when all gates pass.

### Functional

- the complete Hexapod/parallel-kinematic worked case is implemented;
- all selected drive axes, dual feedback channels and probe channels share one epoch;
- forward kinematics reconstruct the end-effector state;
- inverse/differential kinematics produce the complete coupled command;
- surface following controls the calibrated probe deflection against a CAD datum;
- the controller never emits a partial group command.

### Safety

- emergency stop and STO are independent of the software control loop;
- stale, missing, invalid or late mandatory input rejects the entire epoch;
- singularity, actuator limit, probe overrange, loss of contact, deadline miss and
  command-commit failure enter the declared group hold/deceleration/fault state;
- recovery requires an explicit capability and valid state; valid feedback alone never
  resumes motion;
- safety checks are enabled in the timed build.

### Timing

- `physical_latency < 10,000 ns` for every accepted measured cycle;
- every scheduled epoch is retained in the trace; a late or rejected epoch cannot be
  deleted from the population to make the maximum pass;
- all mandatory channels and drives are included;
- measurement uncertainty is included in the margin;
- feedback and actuation skew meet their separately allocated limits;
- zero unexplained deadline misses occur;
- raw traces and the known-delay positive control are retained.

### Platform qualification

- the exact platform passes ADR 0005 Q1–Q4;
- firmware and secure-world behavior are frozen or bounded;
- the evidence applies only to the named hardware/configuration envelope.

### Metrology

- calibrated volumetric/end-effector accuracy is verified independently;
- following error and contact-deflection limits pass across the admitted workspace,
  speed and acceleration envelope;
- a traceable reference artefact correlates the machine result with an external
  measurement.

## 12. Claim ladder for investors and users

Use the strongest statement whose evidence level is complete:

| Level | Permitted statement |
|---|---|
| Designed | “TinyOS has a decomposed, falsifiable sub-10-µs architecture and evidence plan.” |
| Code-live host | “The bounded group contracts/control algorithms pass deterministic host tests.” |
| Mechanism-live | “Periodic group execution and atomic commit operate on the TinyOS Tier 0 fixture.” |
| Target measured | “The named target measured `LAT-CALC-10`/`LAT-OS-10` under the published load.” |
| HIL-live | “The named synchronized I/O architecture closes the complete HIL loop with the published maximum.” |
| Timing-qualified | “The exact qualified platform met `LAT-PHYS-10` for every accepted cycle in the published evidence envelope.” |
| Production-ready | Timing-qualified plus machine safety, metrology, EMC, lifecycle and applicable certification evidence. |

Statements that are **not** permitted:

- “sub-microsecond EtherCAT synchronization” as proof of loop latency;
- “100 µs cycle capable” as proof of a 10 µs response;
- an average, median or percentile described as a hard maximum;
- a function microbenchmark described as sensor-to-drive latency;
- a Raspberry Pi, Jetson or other ARM64 board described as hard real-time before its
  exact platform qualifies;
- a simulated or HIL result described as a production-machine result.

## 13. Immediate decisions and artifacts

Before the next physical-control implementation increment, produce:

1. an ADR selecting Option B, C, D, or an equivalently evidenced architecture;
2. a one-page event diagram with `t_sample_first` through `t_apply_last`;
3. a bill of materials and topology for the latency-critical path;
4. vendor evidence for encoder latching, probe acquisition, drive command acceptance
   and interpolation;
5. an initial hardware lower-bound capture;
6. the feedback-ownership contract correction;
7. bounded Hexapod controller reference vectors and WCET harness;
8. a dated report template implementing §10.

Until those exist, the correct project statement is:

> TinyOS has the capacity and atomic group semantics needed to begin the use case, and
> it now has a concrete route to retire the 10 µs risk. The full physical latency
> capability remains an unproven release gate.
