# STORY-P0-02-03 — Priority Inheritance on Lock Contention

Status: **Verified**
Feature: [`FEAT-P0-02`](../features/FEAT-P0-02.md)
Introduced in: [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)
Implemented in: [`session/hand-2026-07-26/16-story-p0-02-03-priority-inheritance-implementation.md`](../../session/hand-2026-07-26/16-story-p0-02-03-priority-inheritance-implementation.md)

## Description

Priority inheritance (or a documented priority-ceiling equivalent) so a low-priority task holding a lock a high-priority task needs is temporarily boosted to the waiter's priority — the classic priority-inversion mitigation `README.md` Design Pillar 1 and Goal G-RT-1 both name explicitly.

## Depends on

`STORY-P0-02-01` (tasks with priorities to inherit), `STORY-P0-02-02` (a working context switch to actually reschedule after a boost).

**Scope resolution (2026-07-26):** at the point this Story was picked up, no ready-queue/priority-based dispatch loop existed in this kernel (`STORY-P0-02-02`'s `context::switch` is a raw two-context primitive, not a scheduler that autonomously picks the next task to run by priority) — so "a working context switch to actually reschedule after a boost" is only partially satisfied. This Story implements and verifies the *bookkeeping* half of priority inheritance exhaustively (a new `kernel::lock::PriorityInheritingLock`) but explicitly cannot verify the *behavioral* half (a real medium-priority task actually failing to preempt a boosted holder) without that dispatcher, which doesn't exist yet. See `kernel::lock`'s own doc comment and this Story's linked handover for the full rationale — this is a scoping decision made deliberately, not a corner cut silently.

**Update (2026-07-26, same session):** [`STORY-P0-02-05`](STORY-P0-02-05.md) (added immediately after this Story, in the same session) supplies the missing dispatch loop and closes this gap for the cooperative case: `kernel::dispatch::tests::dispatcher_runs_the_boosted_holder_ahead_of_an_uninvolved_ready_task_after_contention` is a real `context::switch`-based dispatch round that selects and runs the boosted holder ahead of an uninvolved, higher-static-priority Ready task — the behavioral guarantee this Story's own scope resolution said couldn't be verified yet. What remains open (per `STORY-P0-02-05`'s own scope note) is true *preemption* of a task that never yields voluntarily, which still needs a timer interrupt / IDT this kernel doesn't have.

## Acceptance criteria

1. A documented, adversarial-style test constructs the classic three-task priority-inversion scenario (low holds lock, high contends for it, medium is a third, independent task) and confirms the low-priority holder's priority is boosted above the medium-priority task's the moment high contends — the bookkeeping precondition a real dispatcher would need to avoid starving high, verified at the priority-value level per the scope resolution above. **Met**: `kernel::lock::tests::contention_boosts_the_holder_above_the_medium_priority_task`.
2. Priority boosts are released deterministically (lock release restores the holder's original priority), with no path that leaves a task permanently boosted — including after multiple, escalating contentions from different waiters while the lock is held. **Met**: `kernel::lock::tests::unlock_restores_the_holders_original_priority` and `repeated_boosts_still_restore_the_original_priority_on_unlock`.

Also covered, beyond the original draft acceptance criteria (surfaced by this Story's own adversarial-test requirement): a lower-priority contender never lowers or otherwise disturbs the holder's priority; `unlock` by a non-holder is rejected (`LockError::NotHeldByCaller`); a reentrant `try_lock` by the current holder is rejected (`LockError::AlreadyLocked`) rather than silently granted or deadlocking.

## Tests

[`TEST-P0-02-03-A`](../tests/TEST-P0-02-03-A.md) — see [`REPORT-2026-07-26-11`](../reports/REPORT-2026-07-26-11.md) for the full pass record. Host-only (`cargo test -p kernel --lib`), per this Story's own scope — `kernel::lock`/`kernel::sched` are pure logic with no target-specific dependency, mirroring `STORY-P0-02-01`'s precedent.

## Goals verified

G-RT-1 (bounded interrupt latency and documented priority-inversion avoidance).
