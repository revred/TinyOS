# TEST-P0-03-03-A — Pool Exhaustion Fails Closed, Then Recovers After a Free

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-03-03`](../stories/STORY-P0-03-03.md)
Tier: Host unit test (no QEMU/hardware dependency), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D07`
Security controls: `SEC-03`, `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`, `C3`, `C4`
Boundary tests: `BND-04`, `BND-15`, `BND-20`
Protection Domain contracts: `PD-01`, `PD-06`, `PD-08`, `PD-13`
Code admission gates: `RCG-09`, `RCG-10`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

Per the TDD mandate, this specification is written before the allocator code it verifies.

**Given** a `Pool<u32, 2>` with both slots filled,
**when** a third `alloc` is attempted,
**then** it returns `Err(PoolError::Exhausted)` with no panic and no change to either occupied slot's value, and:
- a repeated `alloc` on the still-full pool fails the same way every time (not just once),
- freeing one of the two occupied slots and retrying `alloc` succeeds — proving exhaustion is transient state tied to occupancy, not a poisoned/latched pool.

## Test type

Host unit test (`#[cfg(test)]` in `os/src/kernel/src/mem.rs`, run via `cargo test -p kernel --lib`).

## Implementation location

`os/src/kernel/src/mem.rs` (`Pool::alloc`'s exhaustion branch, `PoolError::Exhausted`).

## Reports

- [`REPORT-2026-07-26-04`](../reports/REPORT-2026-07-26-04.md) — Pass (local, host unit test; covers both `TEST-P0-03-01-A` and this Test in one `cargo test` run).
