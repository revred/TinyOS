# TEST-P0-06-03-A — Lock Contention Boosts and Restores Stamp Exactly the Priority-Mutating Events, Nothing Else

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-06-03`](../stories/STORY-P0-06-03.md)
Tier: Host (`cargo test -p kernel --lib`), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — `kernel::lock`/`kernel::spoor_journal` are pure logic with no target-specific dependency.
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D06`, `D11`
Security controls: `SEC-05`, `SEC-14`, `SEC-16`
Containment classes: `C1`, `C2`, `C3`, `C4`
Boundary tests: `BND-17`
Protection Domain contracts: `PD-02`, `PD-08`, `PD-11`, `PD-13`
Code admission gates: `RCG-02`, `RCG-08`, `RCG-12`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

**Given** a `kernel::lock::PriorityInheritingLock` and a caller-supplied `kernel::spoor_journal::SpoorJournal<N>`,
**when**:
- contention actually boosts the holder's priority — **then** exactly one `Spoor` is appended: `Category::Lock`, `Action::Boost`, `Outcome::Ok`, `TARGET` the holder's task-pool index, `COST` the boosted-to priority,
- `unlock` actually restores a previously-boosted priority — **then** exactly one `Spoor` is appended: `Action::Restore`, `TARGET` the task's pool index, `COST` the restored (pre-contention) priority — never an intermediate boosted value even after multiple escalating boosts,
- contention occurs but the contender is already outranked (no boost), or `unlock` is called with nothing to restore — **then** the journal stays empty; a call that doesn't mutate priority stamps nothing,
- the same scenario is replayed with a real `kernel::dispatch::run_once` dispatch round — **then** every existing `STORY-P0-02-03`/`-05` priority-value and dispatch-selection assertion still holds unchanged with a journal threaded through.

## Test type

Unit tests — the existing `kernel::lock` test suite (`STORY-P0-02-03`) extended in place with spoor-emission assertions (no new test functions: the audit trail is a property of the same scenarios, not a separate one), plus `kernel::dispatch`'s existing dispatch-round test updated to thread a journal through unchanged.

## Implementation location

`os/src/kernel/src/lock.rs` (`PriorityInheritingLock::try_lock`/`unlock`, its `#[cfg(test)]` module), `os/src/kernel/src/dispatch.rs`'s `#[cfg(test)]` module — building on `os/src/kernel/src/spoor.rs` (`STORY-P0-06-01`) and `os/src/kernel/src/spoor_journal.rs` (`STORY-P0-06-02`).

## Reports

[`REPORT-2026-07-26-17`](../reports/REPORT-2026-07-26-17.md) — Pass.
