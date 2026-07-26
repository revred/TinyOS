# TEST-P0-06-02-A — Spoor Journal Retains the Most Recent N Entries in Order, Never Panicking on Overflow

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-06-02`](../stories/STORY-P0-06-02.md)
Tier: Host (`cargo test -p kernel --lib`), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — `kernel::spoor_journal` is a pure ring buffer with no target-specific dependency.

## Specification

**Given** a `kernel::spoor_journal::SpoorJournal<N>`,
**when**:
- spoors are appended and the journal is not yet full — **then** `iter()` returns exactly the appended spoors, oldest first, and `len()` reflects the count,
- more spoors are appended than the journal's capacity `N` (including the degenerate `N = 1` case, and many more appends than capacity) — **then** the oldest entries are silently overwritten, `len()` never exceeds `N`, and `iter()` always returns exactly the `N` most recently appended spoors in oldest-first order — never panicking, never rejecting the new append,
- the journal is empty — **then** `iter()` yields nothing and `is_empty()` is `true`,
- the journal's record format is inspected — **then** `JOURNAL_MAGIC` is the identical 8-byte magic (`b"SPOORJ01"`) `Sharc.Blue`'s own on-disk spoor journal format uses, and each entry is stored as exactly the `u64` `Spoor::to_bits()` produces (no intermediate representation).

## Test type

Unit test covering the ring buffer's full state space: not-yet-full, exactly-full, wrapped-once, wrapped-many-times, and the `N = 1` boundary — per `agent/CODING_STANDARDS.md`'s expectation that a fixed-capacity data structure's boundary conditions are exercised explicitly, not just a single happy-path append/read.

## Implementation location

`os/src/kernel/src/spoor_journal.rs` (`SpoorJournal`, `JOURNAL_MAGIC`, its `#[cfg(test)]` module), building on `os/src/kernel/src/spoor.rs`'s `Spoor` (`STORY-P0-06-01`).

## Reports

[`REPORT-2026-07-26-16`](../reports/REPORT-2026-07-26-16.md) — Pass.
