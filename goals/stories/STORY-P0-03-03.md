# STORY-P0-03-03 — Allocation-Failure Path Fails Closed

Status: **Verified** (locally; CI run pending)
Feature: [`FEAT-P0-03`](../features/FEAT-P0-03.md)
Introduced in: [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)

## Description

Per `SeedMVP.md`'s Non-Negotiable #5 (fail-safe over keep-trying) and `agent/CODING_STANDARDS.md`'s "no unbounded loops or unbounded blocking in RT-path code": when `Pool::<T, N>::alloc` is called against a full pool, it returns a typed error immediately — it never panics, never blocks waiting for a slot to free, and never silently wraps around and corrupts a live slot. Implemented in `os/src/kernel/src/mem.rs` alongside `STORY-P0-03-01`, since the exhaustion path is part of the same `alloc` method, but specified and tested as its own acceptance unit because it is the safety-relevant half of the allocator's contract.

## Acceptance criteria

1. Calling `alloc` on a pool with zero free slots returns `Err(PoolError::Exhausted)` on every call, with no side effects (no slot state changes, no panic, no infinite loop).
2. `PoolError::Exhausted` and `PoolError::InvalidHandle` are distinct variants — a caller can distinguish "pool is full" from "you handed me a bad handle" without string-matching an error message, per the Error handling policy's "no stringly-typed errors" rule.
3. A property-style test exhausts a small pool (`N = 2`) completely, confirms the third `alloc` fails closed, then frees one slot and confirms `alloc` succeeds again — proving the failure is transient state, not a poisoned pool.

## Tests

- [`TEST-P0-03-03-A`](../tests/TEST-P0-03-03-A.md) — pool exhaustion fails closed, then recovers after a free.

## Goals verified

G-RT-2 (deterministic memory behavior); indirectly serves Non-Negotiable #5 (fail-safe over keep-trying).
