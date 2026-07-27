# STORY-P0-06-03 — Wire `kernel::lock` to Emit Spoors on Boost/Restore

Status: **Verified**
Feature: [`FEAT-P0-06`](../features/FEAT-P0-06.md)
Introduced in: [`FEAT-P0-06`](../features/FEAT-P0-06.md)'s own exit criteria, as the named follow-up once `STORY-P0-06-01`/`-02` landed
Implemented in: [`session/hand-2026-07-26/23-story-p0-06-03-spoor-adoption-kernel-lock.md`](../../session/hand-2026-07-26/23-story-p0-06-03-spoor-adoption-kernel-lock.md)

## Description

`FEAT-P0-06`'s own exit criteria left one item open after `STORY-P0-06-01`/`-02` landed: at least one real Phase 0 subsystem had to actually call `SpoorJournal::append` through the real `Spoor` API, proving spoor is genuinely usable and not just a type that compiles. This Story closes that gap by wiring `kernel::lock::PriorityInheritingLock` (`STORY-P0-02-03`) — its `Category::Lock`/`Action::Boost`/`Action::Restore` vocabulary already existed in `kernel::spoor` for exactly this purpose, unused until now.

`try_lock` and `unlock` each take a `&mut SpoorJournal<J>` parameter (the same Dependency Inversion pattern `kernel::wcet::OverrunHandler` already established: `PriorityInheritingLock` picks no journal capacity of its own, so wiring in a real, sized journal once a production caller exists is additive, not a rewrite). A spoor is stamped only on the two events that actually mutate a task's priority — a boost (contention that raises the holder above its current priority) and a restore (release that returns the holder to its pre-contention priority) — not on every `try_lock`/`unlock` call. A contended acquire that doesn't boost (the contender is already outranked) and a release with nothing to restore stamp nothing, matching the journal's own "audit trail of what changed" scope.

## Depends on

`STORY-P0-02-03` (the lock this Story instruments), `STORY-P0-06-01` (`Spoor`), `STORY-P0-06-02` (`SpoorJournal<N>`).

## Acceptance criteria

1. A priority boost (`try_lock` contention that raises the holder's priority) stamps a `Spoor` with `Category::Lock`, `Action::Boost`, `Outcome::Ok`, `TARGET` the holder's task-pool index, and `COST` the boosted-to priority value. **Met**: `kernel::lock::tests::contention_boosts_the_holder_above_the_medium_priority_task`, `repeated_boosts_still_restore_the_original_priority_on_unlock` (two escalating boosts, each stamped with its own boosted-to value).
2. A priority restore (`unlock` that had a boost to undo) stamps a `Spoor` with `Category::Lock`, `Action::Restore`, `TARGET` the task's pool index, and `COST` the restored (pre-contention) priority value. **Met**: `unlock_restores_the_holders_original_priority`, `repeated_boosts_still_restore_the_original_priority_on_unlock` (the single restore names the *original* priority, never an intermediate boosted one).
3. A `try_lock`/`unlock` call that doesn't actually change a priority (contender already outranked; release with nothing to restore) stamps nothing — the journal isn't a call-count log. **Met**: `unlock_without_contention_leaves_priority_unchanged`, `a_lower_priority_contender_does_not_change_the_holders_priority` (both assert the journal stays empty).
4. Every existing `STORY-P0-02-03`/`STORY-P0-02-05` acceptance criterion (the priority-value bookkeeping and the real dispatch-round proof) still passes unchanged — this Story adds an audit side effect, not a behavior change. **Met**: `kernel::lock`'s full existing test suite and `kernel::dispatch::tests::dispatcher_runs_the_boosted_holder_ahead_of_an_uninvolved_ready_task_after_contention` all still pass with a journal threaded through.

## Tests

`os/src/kernel/src/lock.rs`'s `#[cfg(test)]` module (existing tests extended with spoor assertions, no new test functions — the audit trail is a property of the existing scenarios, not a separate one) and `os/src/kernel/src/dispatch.rs`'s dispatch-round test (updated to thread a journal through, unchanged assertions otherwise). Host-only, no target dependency. See [`REPORT-2026-07-26-17`](../reports/REPORT-2026-07-26-17.md).

## Goals verified

G-PA-6, G-AI-3 (as `FEAT-P0-06`); G-RT-1 (as `STORY-P0-02-03`, unaffected by this Story's audit-only change).
