# TEST-P0-02-03-A — Priority Inheritance Boosts a Lock Holder and Restores It Deterministically

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-02-03`](../stories/STORY-P0-02-03.md)
Tier: Host (`cargo test -p kernel --lib`), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — `kernel::lock`/`kernel::sched` are pure logic with no assembly, boot, or target-specific dependency, mirroring `STORY-P0-02-01`'s own host-only scope.

## Specification

**Given** a `kernel::sched::Scheduler` with three tasks created at low, medium, and high priority, and a `kernel::lock::PriorityInheritingLock`,
**when**:
- the low-priority task locks it, then the high-priority task contends for it — **then** the lock reports contention (`Err(LockError::AlreadyLocked)`) and the low-priority task's current priority is boosted to the high-priority task's, now outranking the medium-priority task — the classic priority-inversion scenario's inversion is corrected at the bookkeeping level (`STORY-P0-02-03` acceptance criterion 1),
- the low-priority (now-boosted) task releases the lock — **then** its priority is restored to exactly its original, pre-contention value, never left at an intermediate or boosted value (`STORY-P0-02-03` acceptance criterion 2), including after repeated escalating contention from multiple waiters,
- a lower-priority task contends for an already-held lock — **then** the holder's priority is left unchanged (inheritance only ever raises priority, never lowers it),
- a task that isn't the current holder calls `unlock`, or the current holder attempts a reentrant `try_lock` — **then** both fail closed (`LockError::NotHeldByCaller`/`LockError::AlreadyLocked`) rather than releasing someone else's lock or granting/deadlocking a duplicate acquisition.

## Scope note

This kernel has no ready-queue/priority-based dispatch loop yet (`kernel::context::switch` is a raw two-context primitive invoked explicitly by its caller, not a scheduler that picks the next task to run by priority on its own — see `kernel::lock`'s own doc comment). This Test therefore verifies the *bookkeeping* half of priority inheritance exhaustively (boost-on-contention, restore-on-release, no-op for a non-boosting contender, rejection of invalid `lock`/`unlock` calls) but not the *behavioral* half (that a real, running medium-priority task is actually preempted in favor of the now-boosted holder) — proving that requires a real dispatcher, named as a concrete prerequisite in `STORY-P0-02-03`'s own "Immediate next steps," not silently assumed to already exist.

## Test type

Adversarial-style unit test, per `agent/CODING_STANDARDS.md`'s TDD mandate for safety-relevant scheduling paths — constructs the classic three-task inversion scenario plus edge cases (non-boosting contention, repeated escalating boosts, invalid unlock, reentrant lock) rather than only the happy path.

## Implementation location

`os/src/kernel/src/lock.rs` (`PriorityInheritingLock`, its `#[cfg(test)]` module) and `os/src/kernel/src/sched.rs` (`Scheduler::priority_of`/`set_priority`, the accessors `lock.rs` uses to read and boost a task's current priority).

## Reports

[`REPORT-2026-07-26-11`](../reports/REPORT-2026-07-26-11.md) — Pass.
