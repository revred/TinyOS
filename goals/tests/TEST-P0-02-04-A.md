# TEST-P0-02-04-A — WCET Overrun Is Detected on the Exact Tick That Crosses the Budget

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-02-04`](../stories/STORY-P0-02-04.md)
Tier: Host (`cargo test -p kernel --lib`), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — `kernel::wcet`/`kernel::sched` are pure logic with no assembly, boot, timer, or target-specific dependency, mirroring `STORY-P0-02-01`'s and `STORY-P0-02-03`'s own host-only precedent.
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D03`, `D24`
Security controls: `SEC-12`, `SEC-16`, `SEC-20`
Containment classes: `C1`, `C2`, `C3`, `C4`
Boundary tests: `BND-04`, `BND-15`, `BND-16`, `BND-17`, `BND-20`
Protection Domain contracts: `PD-01`, `PD-02`, `PD-07`, `PD-08`, `PD-09`, `PD-12`, `PD-13`
Code admission gates: `RCG-10`, `RCG-12`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

**Given** a `kernel::sched::Scheduler` task with a declared WCET budget, and a `kernel::wcet::OverrunHandler`,
**when**:
- ticks are attributed to the task one at a time via `record_tick`, and cumulative consumption stays at or below the budget — **then** each call returns `Ok(())` and the handler is never invoked,
- a tick's attribution pushes cumulative consumption strictly past the budget — **then** that exact call (not a later one, not a retroactive scan) returns `Err(WcetError::BudgetExceeded)` and calls `handler.on_overrun` exactly once, whether the overrun comes from several in-budget-looking ticks accumulating past it or from a single tick larger than the whole budget (`STORY-P0-02-04` acceptance criterion 1: detected within one scheduler tick of the overrun, not retroactively),
- `record_tick` is called against a task that no longer exists — **then** it fails closed with `Err(WcetError::UnknownTask)` and the handler is never called,
- a task's budget window is reset after an overrun — **then** its consumed-ticks counter returns to 0 and a subsequent in-budget tick succeeds normally, proving the overrun state doesn't latch permanently.

## Scope note

This kernel has neither a periodic timer-tick source that calls `record_tick` on its own, nor the documented watchdog/failsafe system (`README.md` Non-Negotiable #5) `STORY-P0-02-04` acceptance criterion 2 calls for handing an overrun off to — both are concrete, still-open prerequisites (the first is the same missing ready-queue/priority-based dispatcher `STORY-P0-02-03`'s own scope note already named; the second doesn't exist anywhere in this codebase yet, independent of the dispatcher). This Test verifies the *detection* logic exhaustively — acceptance criterion 3's own minimum bar — via `OverrunHandler`, a standalone trait standing in for that not-yet-built watchdog (test-only `RecordingHandler` records calls; no production watchdog implementation is wired in yet). It does not, and currently cannot, verify that a real overrun actually triggers a real failsafe transition.

## Test type

Adversarial-style unit test, per `agent/CODING_STANDARDS.md`'s TDD mandate for timing-sensitive/safety-relevant code: constructs both the "several small ticks that accumulate past budget" and "one oversized tick" overrun shapes, an at-exactly-budget non-overrun edge case, an unknown-task rejection, and a window-reset recovery path — not just a single happy-path tick.

## Implementation location

`os/src/kernel/src/wcet.rs` (`record_tick`, `reset_budget_window`, `OverrunHandler`, its `#[cfg(test)]` module) and `os/src/kernel/src/sched.rs` (`Tcb::ticks_consumed`, `Scheduler::wcet_state`/`add_ticks_consumed`/`reset_ticks_consumed`, the bookkeeping `wcet.rs` reads and mutates).

## Reports

[`REPORT-2026-07-26-12`](../reports/REPORT-2026-07-26-12.md) — Pass.
