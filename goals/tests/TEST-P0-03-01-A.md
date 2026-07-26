# TEST-P0-03-01-A — Pool Alloc/Free Round-Trip and Invalid-Handle Rejection

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-03-01`](../stories/STORY-P0-03-01.md)
Tier: Host unit test (no QEMU/hardware dependency — pure allocator logic), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)

## Specification

Per the TDD mandate, this specification is written before the allocator code it verifies.

**Given** a `Pool<u32, 4>`,
**when** a value is `alloc`'d,
**then** the returned handle can be `free`'d to recover the exact value that was stored, the slot becomes reusable by a subsequent `alloc`, and:
- freeing the same handle twice returns `Err(PoolError::InvalidHandle)` on the second call, not a panic or a stale/aliased value,
- an out-of-range handle (constructed from an index the pool never issued) is rejected the same way.

## Test type

Host unit test (`#[cfg(test)]` in `os/src/kernel/src/mem.rs`, run via `cargo test -p kernel --lib`) — no QEMU dependency because the allocator's correctness doesn't depend on the target's boot environment, only on `core` semantics common to host and `no_std` builds alike.

## Implementation location

`os/src/kernel/src/mem.rs` (`Pool`, `PoolHandle`, `PoolError`).

## Reports

- [`REPORT-2026-07-26-04`](../reports/REPORT-2026-07-26-04.md) — Pass (local, host unit test).
