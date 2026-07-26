# STORY-P0-07-01 — Bounded, Capability-Scoped Message Channel Between Two Tasks

Status: **Planned, not yet started**
Feature: [`FEAT-P0-07`](../features/FEAT-P0-07.md)
Introduced in: [`FEAT-P0-07`](../features/FEAT-P0-07.md), this session (2026-07-26)

## Description

A bounded, fixed-capacity message channel between exactly two `TaskId`s — the "socket communication" half of `FEAT-P0-07`'s local-IPC scope, in the local (never network-addressable) sense. Modeled on the same `Pool<T, N>`-backed, no-heap, fail-closed-on-exhaustion discipline every other Phase 0 subsystem already follows, not a queue with unbounded growth.

## Depends on

`STORY-P0-02-01` (tasks to be channel endpoints), `STORY-P0-03-01` (the `Pool<T, N>` type this Story's message buffer reuses).

## Acceptance criteria (draft — to be finalized when this Story starts)

1. A channel is created bound to exactly two `TaskId`s (its only two endpoints) — no third task can send or receive on it, enforced by the type/API, not by caller discipline.
2. `send`/`receive` never allocate and never block the caller indefinitely — a full channel's `send` fails closed with a typed error (mirroring `PoolError::Exhausted`'s precedent) rather than growing the buffer or looping.
3. Every channel operation is capability-scoped the same way `exec::win32_shim`'s calls are (`STORY-P0-05-03`'s `CapabilityPolicy` precedent) — a task without the capability to communicate with the other endpoint can't create or use a channel to it, once a real capability model (`aci`) exists to check against; until then, this Story documents the same standalone-trait stand-in pattern `win32_shim`/`wcet` already established, per this Story's own scope note when picked up.
4. No network-addressable socket (loopback or otherwise) is ever created to implement this Story — a channel is purely an in-kernel data structure keyed by the two `TaskId`s, never a bound port.

## Tests

Not yet written — deferred until this Story is picked up. Expect host-testable pure logic (a bounded channel over two known task endpoints has no target dependency), mirroring `STORY-P0-02-01`'s own precedent, plus adversarial tests for a third task's rejected access attempt.

## Goals verified

`G-PC-3` (no privileged bypass — a channel's two-endpoint scoping is exactly the kind of "ambient access nobody explicitly granted" this goal exists to prevent), `G-AI-3` (identical mediation regardless of caller type, once a real capability check exists).
