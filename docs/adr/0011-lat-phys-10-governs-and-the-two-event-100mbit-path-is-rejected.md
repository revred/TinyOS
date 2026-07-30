# ADR 0011 — `LAT-PHYS-10` Governs the Final Motion Platform; the Two-Event 100 Mbit/s EtherCAT Path Is Rejected for It; Option B Leads

Status: **Accepted** (2026-07-30) — the architecture *gate* of
[`work/Derisk10usLatencyRequirement.md`](../../work/Derisk10usLatencyRequirement.md) §5
remains **open**: this ADR records what is already decidable (the vocabulary, the
event contract, one physics-based rejection, and the default posture) and names
exactly what evidence closes the rest. It does not name a board, NIC, drive or
topology, because none has been selected or evidenced.
Date: 2026-07-30
Introduced in: [`work/Derisk10usLatencyRequirement.md`](../../work/Derisk10usLatencyRequirement.md)
(WP0 and §13 items 1–2), on the owner's order

## Context

The final motion-platform use case — the parallel-kinematic surface-metrology machine
in [`work/case-hexapod-metrology/`](../../work/case-hexapod-metrology/README.md) —
targets a strict end-to-end physical deadline the de-risking contract names
**`LAT-PHYS-10`**:

> less than 10,000 ns from a common-clock physical feedback latch to the physical
> latch or documented acceptance of the resulting complete corrective command by
> every selected drive in the synchronized group.

TinyOS does not satisfy this today and nothing in this ADR claims otherwise. The risk
this ADR retires is *architectural*: spending months optimizing software around a
transport whose physics already exceeds the deadline.

## Decision 1 — the timing vocabulary is adopted and non-substitutable

`LAT-PHYS-10`, `LAT-OS-10`, `LAT-CALC-10`, `SYNC-1`, `PERIOD-N` and `JITTER-X` are
adopted exactly as defined in the de-risking contract §2. "Under 10 µs" means
`< 10,000 ns`, strict. No property may be reported as another: a calculation latency,
an OS/process-image latency, or a Distributed-Clocks synchronization figure is never
permission to claim `LAT-PHYS-10`. The claim ladder of §12 governs every external
statement, and its "not permitted" list is binding on all marketing and Reports.

## Decision 2 — the event and endpoint contract is frozen

Every future latency result names these events on one traceable timebase (the §3
contract, frozen here as the one-page event diagram §13 item 2 requires):

```text
 physical world          │ acquisition        │ TinyOS / controller      │ output │ physical world
                         │                    │                          │        │
 t_sample_first ──► t_sample_last ──► t_state_ready ──► t_control_start  │        │
   first mandatory    last mandatory    epoch passed       controller     │        │
   channel latches    channel latches   identity/age/      begins         │        │
   epoch N            epoch N           quality/topology/                 │        │
                                        scaling/calibration               │        │
                                                     ──► t_frame_ready ──► t_hw_handoff ──► t_apply_first ──► t_apply_last
                                                          complete frame     ownership to      first drive       last drive
                                                          passed safety/     NIC/FPGA/equiv.   latches/accepts   latches/accepts
                                                          range checks       deterministic HW  command           command

 feedback_latch_skew = t_sample_last  - t_sample_first
 calculation_latency = t_frame_ready  - t_state_ready      (LAT-CALC-10)
 os_latency          = t_hw_handoff   - t_state_ready      (LAT-OS-10)
 physical_latency    = t_apply_last   - t_sample_first     (LAT-PHYS-10: < 10,000 ns)
 apply_skew          = t_apply_last   - t_apply_first      (SYNC-1 output half)
```

Frozen with it: no timestamp generated only after software receives a packet may be
described as the physical sampling time; the endpoint is the drive's documented
command latch/acceptance event, never detectable motion; every formal run states the
§3 disclosure list (latch semantics, drive-acceptance semantics, clock relationship
and uncertainty, masks, failure dispositions, apply epoch, device buffering).

## Decision 3 — Option A is rejected for strict `LAT-PHYS-10`, from physics

For minimum-size Ethernet traffic, one line interval is 8 (preamble/SFD) + 64
(minimum frame) + 12 (inter-packet gap) = 84 bytes = 672 bits; at 100 Mbit/s that is
**6.72 µs**. A feedback-frame → central software calculation → subsequent
command-frame architecture needs at least two such transmission events: **13.44 µs
before** forwarding, cable, DMA, task release, validation, calculation, staging or
drive-latch delay is counted.

Therefore **Option A (central two-event control over a 100 Mbit/s EtherCAT segment)
must never be represented as capable of `LAT-PHYS-10`.** This is a rejection of an
architecture *label*, not of EtherCAT: EtherCAT remains fully in scope for common
time, coherent process images, commissioning, supervisory cycles, drive coordination
and slower group-cycle control, exactly as `ADR 0010` and the motion delivery
contract place it. No measurement can un-reject Option A on a 100 Mbit/s segment;
only a completely specified alternative topology measured under the §10 protocol
could qualify a different disposition.

## Decision 4 — Option B leads; the gate stays open; closing evidence is named

Until the §5 gate is resolved with hardware in hand, **Option B (FPGA/edge closed
loop local to the feedback and drive interfaces, with TinyOS supplying trajectory
coefficients, limits, modes, supervision and evidence collection) is the leading test
architecture**, with Option C (shared-memory accelerator) and Option D (all-gigabit
deterministic fabric) as live candidates. In parallel, `LAT-CALC-10` is adopted as a
TinyOS software milestone — it is meaningful on the software timeline regardless of
which physical option wins.

The gate closes only when a decision record names: controller board, CPU and
accelerator; NIC/ESC, PHYs, couplers, topology and link rate; every drive, encoder
and probe interface; data widths; device latch and interpolation behaviour; cycle,
phase and apply-epoch model; fail-safe behaviour; and which controller owns each
loop. A vendor's nominal cycle time or an average benchmark cannot close it.
**`LE-63`** registers the three artifacts this repository cannot produce without
hardware — the bill of materials/topology, vendor latch/interpolation evidence, and
the initial hardware lower-bound capture — as explicit open debt with the §8 kill
rules attached.

## Consequences

- The motion information model must stop being axis-only *before* controller
  integration (`R4`): a probe, a workpiece-frame sensor and a group thermal channel
  must participate in an accepted epoch as typed end-effector and group/process
  feedback, never as "auxiliary axis" data. Delivered as `STORY-P1-08-02`.
- Every latency Report uses the frozen event contract and the §10 measurement
  protocol; the dated template is
  [`goals/reports/_lat-phys-10-report-template.md`](../../goals/reports/_lat-phys-10-report-template.md).
  A run without a positive control (a known injected delay proven to fail) is not
  evidence (`R13`, and the assurance spine's standing positive-control rule).
- `ADR 0005` is untouched and compounding: no worst-case claim on an unqualified
  platform, and zero platforms are qualified. `LAT-PHYS-10` evidence additionally
  requires the exact platform to pass Q1–Q4.
- The `MFS-08`–`MFS-10` EtherCAT packages remain scoped exactly as the motion
  delivery contract states, but their claims are bounded by Decision 3: they carry
  the machine fabric and the slower group cycle, not the sub-10-µs correction loop,
  unless a qualified Option D proof exists.
