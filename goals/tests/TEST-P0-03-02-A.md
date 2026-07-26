# TEST-P0-03-02-A — Capacity Budget Check Fails the Build When Exceeded

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-03-02`](../stories/STORY-P0-03-02.md)
Tier: Host (`cargo test -p kernel --lib`, plus `cargo run -p xtask -- governance-fixture-test`), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — `kernel::capacities` is pure `const` configuration with no target-specific dependency, and the build-time property this Test cares about (a violated budget fails compilation) is provable identically on the host toolchain.

## Specification

**Given** `kernel::capacities`'s consolidated pool/array capacities and documented `STATIC_MEMORY_BUDGET_BYTES`,
**when**:
- the crate builds normally — **then** `committed_bytes() <= STATIC_MEMORY_BUDGET_BYTES` holds, both as a compile-time `const` assertion (a build that succeeded at all is already proof) and as a runtime-visible `#[test]`,
- a capacity/budget pair is deliberately configured so the analogous check would fail — **then** `cargo build`/`cargo clippy` both fail to compile at all (a `const`-evaluation error, `error[E0080]`), not a runtime allocation-failure path — proven generically by a governance fixture (`fixture-capacity-budget`, `xtask`'s `governance-fixture-test`) mirroring `fixture-oversized`'s existing precedent for the LOC-ceiling gate.

## Test type

Mixed: a host unit test (the budget holds today) plus a governance smoke-test fixture (the check-style *would* fail a build if violated) — the same "prove the gate actually gates, not just that it runs" discipline `TEST-P0-01-02-A` already established for the crate-size ceiling, applied to this Story's own `const`-assertion-based budget check.

## Implementation location

`os/src/kernel/src/capacities.rs` (`MAX_CPUS`, `EXEC_FRAME_POOL_CAPACITY`, `STATIC_MEMORY_BUDGET_BYTES`, `committed_bytes`, the `const _: () = assert!(...)` check, its `#[cfg(test)]` module) and `os/src/xtask/src/governance.rs` (`check_capacity_budget_fixture`, extending `run_fixture_smoke_test` from four fixtures to five).

## Reports

[`REPORT-2026-07-26-14`](../reports/REPORT-2026-07-26-14.md) — Pass.
