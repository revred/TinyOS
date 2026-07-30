# REPORT-YYYY-MM-DD-NN — [Interval measured] on [platform]: [headline number and miss count]

<!--
Template implementing work/Derisk10usLatencyRequirement.md §10 (measurement
protocol) and §11 (timing gates), frozen by ADR 0011. Underscore-prefixed:
this file is a template, not a Report, and is deliberately outside the Report
register. Copy it to REPORT-YYYY-MM-DD-NN.md when filing a real run; delete
every guidance comment; every field below is mandatory — "unrecorded" is a
permitted value only where the field says so, and it is always a finding.
-->

**Test(s) covered:** `TEST-…` (`STORY-…`, `FEAT-…`)
**Date:** YYYY-MM-DD · **Operator:** … · **Tier:** Host | Tier 0 | HIL | Tier 1/2 hardware
**Claim-ladder level asserted (§12, use the lowest that fits):** Designed | Code-live host | Mechanism-live | Target measured | HIL-live | Timing-qualified

## Interval under measurement (exactly one per table row; never substituted)

| Property | Interval | This run |
|---|---|---|
| `LAT-CALC-10` | `t_frame_ready − t_state_ready` | measured? |
| `LAT-OS-10` | `t_hw_handoff − t_state_ready` | measured? |
| `LAT-PHYS-10` | `t_apply_last − t_sample_first` (strict `< 10,000 ns`) | measured? |
| `SYNC-1` | `t_sample_last − t_sample_first` and `t_apply_last − t_apply_first` | measured? |
| `PERIOD-N` / `JITTER-X` | cycle period / named-interval variation | measured? |

A Distributed-Clocks synchronization figure is not a row here and never will be.

## Provenance (all mandatory)

- Source commit · compiler/linker versions · binary SHA-256 hashes
- Board, silicon revision, firmware, boot configuration, power mode, thermal state
- FPGA/ESC/NIC/PHY firmware and configuration hashes
- Network/device order, cable lengths, link rates, process-image layout
- Drive, encoder, probe, coupler model / firmware / configuration
- Exact selected axis and feedback masks (list every channel)
- Cycle, phase, apply-epoch and drive-interpolation settings
- Task/IRQ affinity, cache policy, and every competing workload present
- Clock source, calibration method, timestamp resolution, **measured uncertainty**
- Platform-qualification state per `ADR 0005` (Q1–Q4; "unqualified" is a permitted
  value and caps the claim at *measurement*, never *bound*)

## Endpoint semantics (the §3 disclosure — all mandatory)

- Sensor value at `t_sample_*`: latched | sampled | decoded | merely delivered
- "Drive acceptance" at `t_apply_*`: electrical latch | ESC register event |
  set-point consumption | vendor-defined (cite the vendor evidence)
- Device/controller clock relationship and its measured uncertainty
- Consequence exercised for a missing / stale / duplicate / invalid / late sample
- Any device interpolation or buffering, and for how long

## Workload (the §10 evidence workload — state which of the nine were present)

1. Maximum 16-axis/32-feedback transport and controller load: yes/no
2. Actual Hexapod configuration: yes/no
3. Maximum admitted numerical conditioning and controller branches: yes/no
4. Concurrent UI/telemetry/storage/network/inference pressure: yes/no (list)
5. Hot and cold thermal states, every permitted power mode: yes/no
6. Duration and cycle count, justified against the claimed failure rate: N cycles, T
7. **Positive control — injected known delay > 10 µs reported as FAIL:** yes/no
8. **Positive control — clock-skew/timestamp corruption detected:** yes/no
9. At least one deadline miss demonstrating commanded group-safe behaviour: yes/no

A run without items 7 and 8 is not admissible evidence. A "faster" build with safety
checks disabled is not admissible evidence (`R11`).

## Results

- Raw per-cycle trace: `assets/…` (every scheduled epoch retained; a late or
  rejected epoch is counted, classified, and never deleted from the population)
- Minimum / median / p99 / p99.9 / **maximum** / **miss count**: …
- Misses classified (every one): …
- Instrumentation overhead quantified (`R12`): …
- Measurement uncertainty included in the margin arithmetic: …

## Verdict

- Gate arithmetic: `maximum + uncertainty` vs the allocation, strict comparison
- Claim made, at the ladder level asserted above, in one sentence
- Claims explicitly **not** made (at minimum: everything above the asserted level)

## Register interlock

- Loose ends raised/closed; open-debt rows touched; guardrail-evidence rows filed
  (only in domains this Story's contract selects)
