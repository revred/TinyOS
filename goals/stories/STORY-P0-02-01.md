# STORY-P0-02-01 — Task Creation and Priority Assignment

Status: **Verified**
Feature: [`FEAT-P0-02`](../features/FEAT-P0-02.md)
Introduced in: [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)

## Description

Define the kernel's task control block (TCB) and a fixed-priority task-creation API: a task is created with a static priority, a WCET budget placeholder (enforced fully in `STORY-P0-02-04`), and an entry point — stored in a `Pool<Tcb, N>` (`FEAT-P0-03`'s `STORY-P0-03-01`) rather than a heap-allocated list, so task creation itself carries no allocation-time variance.

## Depends on

`STORY-P0-03-01` (the `Pool<T, N>` type this Story stores task control blocks in) and `STORY-P0-01-01` (a booting kernel to run inside).

## Acceptance criteria

1. Creating a task returns a typed `TaskId`, never a bare integer (per the newtype style note in `agent/CODING_STANDARDS.md`). Implemented as `TaskId`, a newtype over `mem::PoolHandle`, only constructible by `Scheduler::create_task`.
2. Task creation against a full task pool fails closed (`Err(TaskCreateError::Exhausted)`, not a panic), mirroring `STORY-P0-03-03`'s contract, repeatedly and with recovery after a slot frees.
3. Priority is a bounded, statically-checked range (not an arbitrary `u8`), so an out-of-range priority is a compile-time or construction-time error, not a silent clamp. Implemented as `Priority::try_new(u8) -> Result<Priority, PriorityError>` over the range `PRIORITY_MIN..=PRIORITY_MAX` (`0..=31`, five bits — see `sched.rs` doc comment for why 31 was chosen).
4. A task's WCET budget (placeholder, not yet enforced — `STORY-P0-02-04`) and entry point (`extern "C" fn() -> !`, matching `main.rs`'s `kernel_main` idiom) are stored in the `Tcb` and retrievable, so no data is lost between creation and future enforcement/scheduling work.

## Tests

Written test-first (red before green) as `#[cfg(test)]` functions in `os/src/kernel/src/sched.rs`, run via `cargo test -p kernel --lib`. There is no separate `TEST-P0-02-01-*.md` doc (none existed when this Story was picked up); the test functions are the specification:

- `create_task_returns_distinguishable_task_ids` — acceptance criterion 1: two tasks created in the same scheduler get distinct `TaskId`s.
- `exhausted_scheduler_fails_closed_and_recovers_after_free` — acceptance criterion 2: `create_task` against a full `Scheduler<2>` returns `Err(TaskCreateError::Exhausted)` twice in a row (not just once), then succeeds again once a slot is freed. Mirrors `mem.rs::exhausted_pool_fails_closed_without_side_effects`.
- `priority_construction_rejects_out_of_range_values` — acceptance criterion 3: `Priority::try_new(PRIORITY_MAX + 1)` and `Priority::try_new(u8::MAX)` both return `Err(PriorityError::OutOfRange)`, not a panic or a clamp.
- `priority_construction_accepts_full_valid_range_including_boundaries` — acceptance criterion 3: every value in `PRIORITY_MIN..=PRIORITY_MAX`, including both boundaries, constructs successfully.
- `created_task_tcb_retains_priority_and_budget` — acceptance criterion 4: a created task's `Tcb` (recovered via `Pool::free`) reports back the exact priority, WCET budget, and entry-point function pointer it was created with.

See [`REPORT-2026-07-26-05`](../reports/REPORT-2026-07-26-05.md) for the verification run.

## Goals verified

G-RT-1 (preemptive, priority-based scheduling).
