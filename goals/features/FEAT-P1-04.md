# FEAT-P1-04 — Timer-Driven Preemption, Deadline Monitor & WCET Watchdog

Status: **In Progress — 1 of 2 Stories Verified**
Epic: [`EPIC-P1`](../epics/EPIC-P1.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

Give the scheduler its teeth (Goals **G-RT-1**, **G-RT-3**): `EPIC-P0` left a cooperative-only dispatch loop (`kernel::dispatch::run_once`), a WCET bookkeeper with no timer behind it (`kernel::wcet::record_tick` — the "no timer, no watchdog" structural gap `STORY-P0-02-04` named), and an armed local-APIC timer whose ticks nothing consumes. This Feature connects them: the timer tick preempts the running task through a real interrupt-driven dispatch path, `record_tick` is driven by the real timer so WCET budgets are enforced against real time, and an overrun triggers the declared fault policy (restart, degrade, or trip to safe state — via `FEAT-P1-02`'s fault machinery), never a silent log line. This is `README.md` Phase 1's "deadline monitor" made real, and the priority-inheritance lock finally gets behavioral verification under genuine preemption.

## Crate(s) involved

`os/src/kernel/` (preemptive dispatch, deadline monitor, WCET-watchdog wiring), `os/src/hal-x86_64/` (timer ISR → scheduler hook)

## Depends on

`FEAT-P1-02` (overrun handling lands in real fault machinery), `FEAT-P1-01` (preemption latency and tick-to-dispatch cost get measured baselines against D03/D05 budgets). Independent of `FEAT-P1-03` (preemption works on the identity map too; the two compose in `FEAT-P1-06`).

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P1-04-01`](../stories/STORY-P1-04-01.md) | Timer-driven preemption: tick → interrupt-driven dispatch, priority-inheritance under real preemption | **Verified** (Tier 0 + Host, 2026-07-28, `REPORT-2026-07-28-03`; assurance `baseline-debt`) |
| [`STORY-P1-04-02`](../stories/STORY-P1-04-02.md) | Deadline monitor & WCET watchdog on the real timer; overrun → declared fault policy | Specified |

`STORY-P1-04-01` closed `LE-01` (priority inheritance's behavioural proof, open since `EPIC-P0`) and `LE-14` (extended-state save/restore — in the ISR stub, so it covers every tick rather than only preempting ones). `LE-02` is untouched and is `-04-02`'s to close: `wcet::record_tick` is still not driven by the real timer, and no overrun trips a declared policy.

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0/C1** · subject **C1/C2** · boundary tests **BND-15, -16, -17**.

Preemption is budget enforcement, not authority: a preempted or overrun task loses CPU, never gains anything; containment class never implies scheduling priority (`BND-16`); RT reserves are priority-safe so no lower-criticality storm can starve an RT task (`BND-15`); every overrun decision is a spoor with class/actor/action/outcome (`BND-17`, `PD-07`/`PD-08` temporal isolation and finite charged resources).

## Exit criteria

Both Stories **Verified** at Tier 0: a busy-looping low-priority task is provably preempted by a higher-priority one within the measured tick budget; a deliberately-overrunning task trips its declared policy through the fault path; the priority-inheritance lock's behavioral half (blocked-on-contention boost under real preemption) is finally verified, closing `STORY-P0-02-03`'s host-only caveat.
