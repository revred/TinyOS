# STORY-P0-02-04 — WCET Budget Enforcement

Status: **Verified**
Feature: [`FEAT-P0-02`](../features/FEAT-P0-02.md)
Introduced in: [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)
Implemented in: [`session/hand-2026-07-26/17-story-p0-02-04-wcet-enforcement-implementation.md`](../../session/hand-2026-07-26/17-story-p0-02-04-wcet-enforcement-implementation.md)

## Description

Every RT task declares a worst-case execution time (WCET) budget as part of its task definition (`agent/CODING_STANDARDS.md`'s Real-time discipline section); this Story is the scheduler-side enforcement of that budget — detecting and handing off to a documented failsafe path when a task overruns it, rather than letting an overrun silently degrade every other task's timing.

## Depends on

`STORY-P0-02-01` (the task definition this Story adds a budget to), `STORY-P0-02-02` (context switch, since enforcement acts at a switch/tick boundary).

**Scope resolution (2026-07-26):** mirroring `STORY-P0-02-03`'s own resolution (both Stories hit the identical gap). At the point this Story was picked up, this kernel had neither a periodic timer-tick source to call detection logic on its own, nor a real, documented watchdog/failsafe system implementation to hand an overrun off to (`README.md` Non-Negotiable #5 is a design commitment, not yet a built subsystem). This Story implements and verifies the *detection* half exhaustively (`kernel::wcet::record_tick`, checking cumulative consumption against budget synchronously on every attributed tick) via a standalone `OverrunHandler` trait standing in for the not-yet-built watchdog — the same Dependency Inversion pattern `exec::win32_shim::CapabilityPolicy` established for `aci`. It cannot verify that a live timer actually drives this detection, or that a real failsafe transition actually follows an overrun, without those two prerequisites. See `kernel::wcet`'s own doc comment and this Story's linked handover for the full rationale.

**Update (2026-07-26, same session):** [`STORY-P0-02-05`](STORY-P0-02-05.md) supplies a real cooperative dispatch loop, but this Story's two specific gaps are independent of it and remain open: a dispatch loop is not a *timer* (nothing yet periodically calls `record_tick` on its own tick boundary, cooperative or otherwise), and no real watchdog/failsafe implementation exists regardless of what drives detection. Unlike `STORY-P0-02-03`, this Story's gap is not closed by `STORY-P0-02-05` — restated explicitly so the two Stories' differing outcomes from the same dispatch-loop addition aren't conflated.

## Acceptance criteria

1. A task that exceeds its declared WCET budget is detected within one scheduler tick of the overrun, not retroactively after the fact. **Met**, at the detection-logic level per the scope resolution above: `record_tick` checks cumulative consumption against budget on every call, so an overrun trips on the exact tick that crosses the budget — verified both for several small ticks accumulating past it and for a single oversized tick (`kernel::wcet::tests::overrun_is_detected_on_the_exact_tick_that_crosses_the_budget`, `a_single_oversized_tick_overruns_immediately`).
2. The overrun path hands off to the documented watchdog/failsafe system (`README.md` Non-Negotiable #5), not a silent log-and-continue. **Partially met**: `record_tick` calls `OverrunHandler::on_overrun` exactly once per detected overrun, never silently continuing — but no real watchdog/failsafe implementation exists yet to wire in as the production `OverrunHandler`, per the scope resolution above. This is a genuine gap, not a completed criterion, restated in `TEST-P0-02-04-A`'s own scope note.
3. Per `agent/CODING_STANDARDS.md`'s "timing-sensitive code" rule, this Story's tests include a timing regression entry, not just a functional pass — deferred in full to Roadmap Phase 1's timing regression suite, but this Story's own test must at minimum prove the *detection* logic is correct under a synthetic overrun. **Met**: `kernel::wcet`'s 6 tests cover in-budget, at-exactly-budget (not an overrun), multi-tick accumulation, single-oversized-tick, unknown-task rejection, and budget-window-reset recovery.

## Tests

[`TEST-P0-02-04-A`](../tests/TEST-P0-02-04-A.md) — see [`REPORT-2026-07-26-12`](../reports/REPORT-2026-07-26-12.md) for the full pass record. Host-only (`cargo test -p kernel --lib`), per this Story's own scope — `kernel::wcet`/`kernel::sched` are pure logic with no target-specific dependency, mirroring `STORY-P0-02-01`'s and `STORY-P0-02-03`'s precedent.

## Goals verified

G-RT-1 (bounded interrupt latency — an unenforced WCET budget is not actually bounded; this Story's detection logic is the mechanism that bound would rely on once a real timer/watchdog exist to drive and consume it).
