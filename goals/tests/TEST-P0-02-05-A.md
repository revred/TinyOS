# TEST-P0-02-05-A — Dispatch Loop Actually Runs the Highest-Priority Ready Task, Including After a Priority Boost

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-02-05`](../stories/STORY-P0-02-05.md)
Tier: Host (`cargo test -p kernel --lib`), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — `context_switch_asm` is pinned to `sysv64` explicitly (not `extern "C"`, which follows the host OS's own convention), so the exact assembly this Story's dispatch loop calls runs identically on the host toolchain and the real target, mirroring `STORY-P0-02-02`'s own precedent for why this is meaningful and not just a Tier-0 stand-in.

## Specification

**Given** a `kernel::sched::Scheduler` with tasks at distinct priorities, real per-task `kernel::context::Context`s built via `Context::new`, and `kernel::dispatch::run_once`,
**when**:
- several tasks are `Ready` at once with no contention — **then** `run_once` selects and actually switches into the highest-*static*-priority one, and the switched-into task's own code visibly runs (recorded in a shared log the task writes to before yielding back), not merely identified as "would be selected,"
- a lower-priority task holds a `kernel::lock::PriorityInheritingLock` and a higher-priority task contends for it, boosting the holder's priority above an uninvolved, higher-static-priority Ready task's — **then** the *next* `run_once` call selects and switches into the now-boosted holder instead of the uninvolved task, proving the boost changes a real dispatch decision, not just a recorded priority value,
- the holder releases the lock, restoring its original priority — **then** a further `run_once` call reverts to selecting the uninvolved (now again higher-priority) task,
- no task is `Ready` — **then** `run_once` returns `None` without switching into anything.

## Scope note

This is a cooperative dispatch loop: task functions in this Test's fixture yield back to the dispatcher voluntarily (calling `context::switch` themselves, in an infinite loop, once per `run_once` call) — nothing preempts a task that doesn't yield, since this kernel has no timer interrupt / IDT yet (`STORY-P0-05-02`'s named gap). This Test proves `STORY-P0-02-03`'s own previously-unverifiable behavioral claim (a boosted holder is actually run ahead of an uninvolved task) for tasks that cooperate this way; it does not prove true involuntary preemption.

## Test type

Adversarial-style/integration unit test — constructs a full three-round scenario (baseline selection, post-boost selection, post-release selection) with real `context::switch` calls into real task functions, not a priority-value-only assertion, per `agent/CODING_STANDARDS.md`'s TDD mandate for scheduling-correctness paths.

## Implementation location

`os/src/kernel/src/dispatch.rs` (`run_once`, its `#[cfg(test)]` module) and `os/src/kernel/src/sched.rs` (`TaskState`, `Scheduler::state_of`/`set_state`/`iter_tasks`/`highest_priority_ready`, `TaskId::index`, the ready-queue selection logic `run_once` acts on) and `os/src/kernel/src/mem.rs` (`Pool::iter_occupied`, `PoolHandle::index`, the enumeration primitive `iter_tasks` is built on).

## Reports

[`REPORT-2026-07-26-13`](../reports/REPORT-2026-07-26-13.md) — Pass.
