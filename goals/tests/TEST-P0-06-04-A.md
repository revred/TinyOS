# TEST-P0-06-04-A — WCET Overrun and Budget Reset Stamp Exactly the Boundary-Crossing Events, Nothing Else

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-06-04`](../stories/STORY-P0-06-04.md)
Tier: Host (`cargo test -p kernel --lib`), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — `kernel::wcet`/`kernel::spoor_journal` are pure logic with no target-specific dependency.
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D03`, `D11`, `D24`
Security controls: `SEC-12`, `SEC-14`, `SEC-16`, `SEC-20`
Containment classes: `C1`, `C2`, `C3`, `C4`
Boundary tests: `BND-17`
Protection Domain contracts: `PD-02`, `PD-08`, `PD-11`, `PD-13`
Code admission gates: `RCG-02`, `RCG-08`, `RCG-12`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

**Given** a `kernel::wcet::record_tick`/`reset_budget_window` pair and a caller-supplied `kernel::spoor_journal::SpoorJournal<N>`,
**when**:
- a tick actually crosses the task's declared WCET budget — **then** exactly one `Spoor` is appended: `Category::Wcet`, `Action::Overrun`, `Outcome::Failed`, `TARGET` the task's pool index, `COST` the total ticks consumed at the moment it crossed,
- `reset_budget_window` actually clears nonzero consumption — **then** exactly one `Spoor` is appended: `Action::ResetBudget`, `Outcome::Ok`, `TARGET` the task's pool index, `COST` the consumption cleared,
- a tick stays within budget, or a reset is called against an already-zero counter — **then** the journal stays empty; a call that doesn't cross a budget boundary stamps nothing,
- either function is called against an unknown task — **then** it fails closed (`WcetError::UnknownTask` / `None`) with no side effect and nothing stamped.

## Test type

Unit tests — the existing `kernel::wcet` test suite (`STORY-P0-02-04`) extended in place with journal-threading and spoor-emission assertions, plus two new test functions (`resetting_an_already_zero_counter_stamps_nothing`, `reset_budget_window_against_an_unknown_task_fails_closed`) covering `reset_budget_window`'s no-stamp and fail-closed paths, which had no dedicated assertion before this Story.

## Implementation location

`os/src/kernel/src/wcet.rs` (`record_tick`, `reset_budget_window`, its `#[cfg(test)]` module) — building on `os/src/kernel/src/spoor.rs` (`STORY-P0-06-01`) and `os/src/kernel/src/spoor_journal.rs` (`STORY-P0-06-02`), reusing the journal-as-parameter pattern `os/src/kernel/src/lock.rs` established in `STORY-P0-06-03`.

## Reports

[`REPORT-2026-07-26-18`](../reports/REPORT-2026-07-26-18.md) — Pass.
