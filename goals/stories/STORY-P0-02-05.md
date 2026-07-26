# STORY-P0-02-05 — Priority-Ordered Cooperative Dispatch Loop

Status: **Verified**
Feature: [`FEAT-P0-02`](../features/FEAT-P0-02.md)
Introduced in: [`session/hand-2026-07-26/18-story-p0-02-05-dispatch-loop-implementation.md`](../../session/hand-2026-07-26/18-story-p0-02-05-dispatch-loop-implementation.md) — not part of `FEAT-P0-02`'s original 4-Story decomposition; added when `STORY-P0-02-03` and `STORY-P0-02-04` both independently surfaced the same missing prerequisite (see "Depends on" below).
Implemented in: [`session/hand-2026-07-26/18-story-p0-02-05-dispatch-loop-implementation.md`](../../session/hand-2026-07-26/18-story-p0-02-05-dispatch-loop-implementation.md)

## Description

`STORY-P0-02-03` (priority inheritance) and `STORY-P0-02-04` (WCET enforcement) each implemented and verified their own bookkeeping exhaustively, but both explicitly named the same gap as unverifiable without it: this kernel had no mechanism that actually selects and runs tasks in priority order — only a raw two-context `context::switch` primitive (`STORY-P0-02-02`) and (as of those two Stories) priority/budget bookkeeping with nothing consuming it. This Story closes that gap: a priority-ordered ready-queue selection (`Scheduler::highest_priority_ready`) plus a cooperative dispatch loop (`kernel::dispatch::run_once`) that actually switches into the selected task via `context::switch`.

## Depends on

`STORY-P0-02-01` (tasks with priorities to select among), `STORY-P0-02-02` (the `context::switch` primitive this Story's dispatch loop calls). Named as a concrete prerequisite by both `STORY-P0-02-03`'s and `STORY-P0-02-04`'s own scope-resolution notes, independently, in the same session — strong enough a signal to implement it immediately after rather than deferring further.

## Scope note

This is a **cooperative** dispatcher, not a preemptive one: this kernel still has no timer interrupt / IDT (`STORY-P0-05-02`'s named gap), so nothing forces a running task to yield control back — it must call `context::switch` itself. This is sufficient to behaviorally close `STORY-P0-02-03`'s own gap (a boosted lock holder is actually *chosen and run* ahead of an uninvolved, higher-static-priority Ready task — proven by this Story's own test, a real `context::switch` into the holder's task function, not a priority-value assertion standing in for it) for a task that yields at controlled points. It does **not** prove true preemption of a task that never yields voluntarily, and does not (yet) drive `kernel::wcet::record_tick` from a real timer — `STORY-P0-02-04`'s own gap (no live timer source, no real watchdog) remains open; this Story only supplies the piece needed to *run* tasks by priority, not the piece needed to interrupt one against its will or to consume an overrun with a real failsafe.

## Acceptance criteria

1. Given several tasks in `TaskState::Ready`, the highest-priority one is always selected — ties broken deterministically (most-recently-created of the tied set). **Met**: `Scheduler::highest_priority_ready`, `sched::tests::highest_priority_ready_selects_the_highest_priority_among_several`.
2. A `Blocked` or `Finished` task is never selected, regardless of its priority relative to Ready tasks. **Met**: `sched::tests::blocked_and_finished_tasks_are_never_selected`.
3. `dispatch::run_once` actually runs the selected task via a real `context::switch` (not just identifies it) and resumes the caller the moment that task yields back, leaving a still-`Running` task `Ready` again for the next round (cooperative round-robin default). **Met**: `dispatch::tests::run_once_against_an_empty_scheduler_returns_none` (the trivial case) and, decisively, the Story's flagship behavioral test below.
4. A `PriorityInheritingLock` boost, once applied, actually changes which task the *next* dispatch round selects and runs — closing `STORY-P0-02-03`'s own named behavioral gap. **Met**: `dispatch::tests::dispatcher_runs_the_boosted_holder_ahead_of_an_uninvolved_ready_task_after_contention` — without contention, a real dispatch round picks a higher-*static*-priority medium task over a low-priority holder; after a third task contends for the holder's lock (boosting it), the *next* round picks the now-boosted holder instead, and after release, a further round returns to picking medium — three real `context::switch` calls, not simulated ones.

## Tests

`os/src/kernel/src/sched.rs`'s `#[cfg(test)]` module (`highest_priority_ready`/state-transition tests, 5 new) and `os/src/kernel/src/dispatch.rs`'s (`run_once`, 2 new, including the flagship behavioral one). Host-only (`cargo test -p kernel --lib`) — `context_switch_asm` is pinned to `sysv64` explicitly (not `extern "C"`, which would follow the host OS's own convention), so the same assembly this Story's dispatch loop calls runs identically on the host toolchain and the real target, mirroring `STORY-P0-02-02`'s own precedent for why its host tests are meaningful, not just a stand-in for Tier 0. See [`REPORT-2026-07-26-13`](../reports/REPORT-2026-07-26-13.md) for the full pass record.

## Goals verified

G-RT-1 (preemptive — well, cooperative for now — priority-based scheduling; this Story is the mechanism `STORY-P0-02-03`'s and `STORY-P0-02-04`'s own G-RT-1 claims were resting on without yet having it).
