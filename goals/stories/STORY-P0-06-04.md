# STORY-P0-06-04 — Wire `kernel::wcet` to Emit Spoors on Overrun/Reset

Status: **Verified**
Feature: [`FEAT-P0-06`](../features/FEAT-P0-06.md)
Introduced in: `FEAT-P0-06.md`'s own named "unwired, available second candidate" (`kernel::wcet`'s `Action::Overrun`/`Action::ResetBudget` vocabulary), taken up by explicit user request once `STORY-P0-06-03` closed the Feature's own exit criteria
Implemented in: [`session/hand-2026-07-26/24-story-p0-06-04-spoor-adoption-kernel-wcet.md`](../../session/hand-2026-07-26/24-story-p0-06-04-spoor-adoption-kernel-wcet.md)

## Description

`FEAT-P0-06`'s exit criteria only required *at least one* real subsystem to emit spoors, which `STORY-P0-06-03` already satisfied via `kernel::lock`. This Story adopts spoor into a second subsystem — `kernel::wcet` (`STORY-P0-02-04`) — whose `Category::Wcet`/`Action::Overrun`/`Action::ResetBudget` vocabulary already existed in `kernel::spoor` unused, per that Story's own module doc comment naming it "a natural second candidate if wanted, not required by anything currently open."

`record_tick` and `reset_budget_window` each take a `&mut SpoorJournal<J>` parameter (the same Dependency Inversion pattern `kernel::lock`'s `STORY-P0-06-03` already applied, itself mirroring `kernel::wcet::OverrunHandler`'s own precedent: neither function picks a journal capacity of its own). A spoor is stamped only on the two *budget-boundary* events, not every call: an overrun (the tick that actually crosses budget) and a reset that actually clears nonzero consumption. A tick that stays within budget, or a reset against an already-zero counter, stamps nothing — matching `kernel::lock`'s own "audit trail of what changed" discipline rather than a call-count log.

## Depends on

`STORY-P0-02-04` (the WCET enforcement this Story instruments), `STORY-P0-06-01` (`Spoor`), `STORY-P0-06-02` (`SpoorJournal<N>`), `STORY-P0-06-03` (established the journal-as-parameter pattern this Story reuses).

## Acceptance criteria

1. A tick that crosses budget (`record_tick` returning `WcetError::BudgetExceeded`) stamps a `Spoor` with `Category::Wcet`, `Action::Overrun`, `Outcome::Failed`, `TARGET` the task's pool index, and `COST` the total ticks consumed at the moment it crossed. **Met**: `wcet::tests::overrun_is_detected_on_the_exact_tick_that_crosses_the_budget`, `a_single_oversized_tick_overruns_immediately`.
2. A reset (`reset_budget_window`) that actually clears nonzero consumption stamps a `Spoor` with `Action::ResetBudget`, `Outcome::Ok`, `TARGET` the task's pool index, and `COST` the consumption cleared. **Met**: `resetting_the_budget_window_clears_prior_consumption`.
3. A tick that stays within budget, or a reset against an already-zero counter, stamps nothing. **Met**: `a_tick_within_budget_does_not_overrun`, `consumption_exactly_at_the_budget_is_not_an_overrun`, `resetting_an_already_zero_counter_stamps_nothing` (all assert the journal stays empty).
4. An unknown task fails closed on both functions with no side effect and no spoor stamped. **Met**: `record_tick_against_an_unknown_task_fails_closed`, `reset_budget_window_against_an_unknown_task_fails_closed`.
5. Every existing `STORY-P0-02-04` acceptance criterion still passes unchanged — this Story adds an audit side effect, not a behavior change. **Met**: `kernel::wcet`'s full existing test suite passes with a journal threaded through, all prior assertions unchanged.

## Tests

`os/src/kernel/src/wcet.rs`'s `#[cfg(test)]` module — existing tests extended with journal-threading and spoor assertions, plus two new test functions covering `reset_budget_window`'s previously-unasserted no-stamp/fail-closed paths. Host-only, no target dependency. See [`REPORT-2026-07-26-18`](../reports/REPORT-2026-07-26-18.md).

## Goals verified

G-PA-6, G-AI-3 (as `FEAT-P0-06`); G-RT-1 (as `STORY-P0-02-04`, unaffected by this Story's audit-only change).
