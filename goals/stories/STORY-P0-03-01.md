# STORY-P0-03-01 — Bounded-Capacity Pool Allocator Type

Status: **Verified** (locally; CI run pending)
Feature: [`FEAT-P0-03`](../features/FEAT-P0-03.md)
Introduced in: [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)

## Description

A `Pool<T, const N: usize>` type in `os/src/kernel/src/mem.rs`: fixed-capacity, statically-sized storage for up to `N` live values of `T`, with no heap allocation anywhere in its implementation — the RT-path primitive `agent/CODING_STANDARDS.md`'s "no heap allocation in any scheduler, IPC, or interrupt-handling hot path" rule requires. This Story covers the allocator type and its ordinary alloc/free path; exhaustion behavior is specified separately in `STORY-P0-03-03` (fail-closed on a full pool) but implemented in the same type since the two are inseparable in practice.

## Acceptance criteria

1. `Pool::<T, N>::new()` is a `const fn` (no runtime initialization cost, usable in a `static`) and requires no heap allocation — backing storage is `[MaybeUninit<T>; N]` plus an `N`-bit-or-smaller occupancy bitmap, not a `Vec`/`Box`.
2. `alloc(&mut self, value: T) -> Result<PoolHandle, PoolError>` claims the first free slot, moves `value` into it without dropping the caller's copy twice, and returns a handle that identifies that slot. `&mut self` is deliberate at this stage — no concurrent-access story exists yet (that's `FEAT-P0-02`'s scheduler work), so this Story does not invent an interior-mutability/locking scheme speculatively; a `&self`, synchronized API is a follow-on Story once real concurrent callers exist.
3. `free(&mut self, handle: PoolHandle) -> Result<T, PoolError>` returns ownership of the stored value to the caller and marks the slot free again; freeing an already-free or out-of-range handle returns `Err(PoolError::InvalidHandle)` rather than panicking (`panic!` is not error handling on an RT path, per `agent/CODING_STANDARDS.md`).
4. No `unsafe` block lacks a `// SAFETY:` comment stating the invariant that makes it sound, per the Unsafe code policy.
5. `cargo test -p kernel --lib` passes on the host target with no `unsafe` beyond what's needed for the `MaybeUninit` slot storage itself.

## Tests

- [`TEST-P0-03-01-A`](../tests/TEST-P0-03-01-A.md) — pool alloc/free round-trip and double-free/invalid-handle rejection (host unit test).

## Goals verified

G-RT-2 (deterministic memory behavior — no unbounded heap fragmentation or allocation-time variance).
