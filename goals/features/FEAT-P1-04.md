# FEAT-P1-04 — Timer-Driven Preemption, Deadline Monitor & WCET Watchdog

Status: **Complete — 3 of 3 Stories Verified** (assurance `baseline-debt`; `LE-09` hardware debt open, and the Feature's title names a deadline monitor this Feature does not deliver — see below). Reopened 2026-07-28 for `STORY-P1-04-03`: the Feature's two Verified Stories described mechanisms the shipping image did not run, which is a gap in the Feature's own claim rather than a follow-on to it.
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
| [`STORY-P1-04-02`](../stories/STORY-P1-04-02.md) | Deadline monitor & WCET watchdog on the real timer; overrun → declared fault policy | **Verified** (Tier 0 + Host, 2026-07-28, `REPORT-2026-07-28-04`; assurance `baseline-debt`) |
| [`STORY-P1-04-03`](../stories/STORY-P1-04-03.md) | Preemption and WCET enforcement on the shipping `os` image; the workload's overrun declaration made a decision | **Verified** (Tier 0 + Host, 2026-07-28, `REPORT-2026-07-28-05`; assurance `baseline-debt`) |

`STORY-P1-04-01` closed `LE-01` (priority inheritance's behavioural proof, open since `EPIC-P0`) and `LE-14` (extended-state save/restore — in the ISR stub, so it covers every tick rather than only preempting ones). `STORY-P1-04-02` closed `LE-02`: `wcet::record_tick` is driven by the real timer, and an overrun trips the policy the task declared at creation. `STORY-P1-04-03` closed `LE-20`: all of it now runs in the binary this project ships.

**What this Feature does not deliver, stated rather than blurred.** Its title names a *deadline monitor* as well as a WCET watchdog. A declared deadline is a different quantity from a declared execution budget, and only the latter is enforced — see `STORY-P1-04-02`'s explicit scope. That gap stays named and unclosed. `LE-22` (degrade and priority inheritance have not been reconciled; a boosted holder that is degraded has the degrade undone by the subsequent unlock) is likewise open and is not this Feature's to close.

**Why a third Story was added after the Feature was called Complete.** `LE-20` was originally recorded as a follow-on: preemption and enforcement were Verified in fixtures, and wiring them into the shipping image was treated as separate work. That framing was wrong, and calling the Feature Complete on it overstated the evidence. This Feature's own exit criteria are about what the scheduler *does*, and a mechanism that is inert in the binary this project ships does not do it. `STORY-P1-04-03` closed that, and the Feature exits on the shipping image rather than on its fixtures.

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0/C1** · subject **C1/C2** · boundary tests **BND-15, -16, -17**.

Preemption is budget enforcement, not authority: a preempted or overrun task loses CPU, never gains anything; containment class never implies scheduling priority (`BND-16`); RT reserves are priority-safe so no lower-criticality storm can starve an RT task (`BND-15`); every overrun decision is a spoor with class/actor/action/outcome (`BND-17`, `PD-07`/`PD-08` temporal isolation and finite charged resources).

## Exit criteria

All three Stories **Verified** at Tier 0: a busy-looping low-priority task is provably preempted by a higher-priority one within the measured tick budget; a deliberately-overrunning task trips its declared policy through the fault path; the priority-inheritance lock's behavioral half (blocked-on-contention boost under real preemption) is finally verified, closing `STORY-P0-02-03`'s host-only caveat; **and the shipping `os` image does all of it**, rather than describing mechanisms only its fixtures run.
