# STORY-P0-03-02 — Compile-Time Pool-Size Configuration

Status: **Verified**
Feature: [`FEAT-P0-03`](../features/FEAT-P0-03.md)
Introduced in: [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)
Implemented in: [`session/hand-2026-07-26/19-story-p0-03-02-capacity-configuration-implementation.md`](../../session/hand-2026-07-26/19-story-p0-03-02-capacity-configuration-implementation.md)

## Description

A single, kernel-wide place to declare each RT subsystem's pool capacities (task control blocks, IPC message slots, and — per the Phase 6 mmap-page-table direction recorded in `session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md` — the demand-paging metadata pool a future file-backed virtual-memory region will need) as `const` values checked at compile time, rather than magic numbers scattered per call site.

## Depends on

`STORY-P0-03-01` (the `Pool<T, N>` type this Story configures).

**Scope resolution (2026-07-26):** at the point this Story was picked up, no production `Scheduler`/task-control-block pool, IPC pool, or demand-paging pool actually existed with a concrete call site yet (`STORY-P0-02-05`'s dispatch loop is real, but nothing in `main.rs` wires a production `Scheduler` into the boot path). This Story consolidates the capacities that **do** have a real, concrete consumer today — `MAX_CPUS` (`kernel::main`'s boot-time ACPI topology discovery) and `EXEC_FRAME_POOL_CAPACITY` (previously a `FRAMES` constant duplicated across `exec`'s two Tier 0 fixture files) — into `kernel::capacities`, the single location this Story's acceptance criteria call for. It deliberately does **not** add placeholder constants for the task-control-block/IPC/demand-paging pools this Story's own description names, since none has a real call site yet — adding one would be speculative, ahead-of-need code `agent/CODING_STANDARDS.md` argues against. Add one alongside whichever future Story first wires a production consumer in.

## Acceptance criteria

1. Pool capacities for every kernel subsystem that uses `Pool<T, N>` (or the analogous fixed-capacity `Topology<N>`) **and has a real call site** are declared in one reviewable location, not inline at each call site. **Met**: `kernel::capacities::{MAX_CPUS, EXEC_FRAME_POOL_CAPACITY}`, replacing `kernel::main`'s own local `MAX_CPUS` const and the `FRAMES` constant previously duplicated in `exec/src/fixture_main.rs` and `exec/src/fixture_win32_shim_main.rs`.
2. A pool sized larger than available static memory for the target fails the build, not a runtime allocation-failure path — this is a compile-time capacity budget, not a runtime-tunable one. **Met**: `kernel::capacities::STATIC_MEMORY_BUDGET_BYTES` plus a `const _: () = assert!(committed_bytes() <= STATIC_MEMORY_BUDGET_BYTES, ...)` — a violated budget is a `const`-evaluation compile error (`error[E0080]`), proven generically by a new governance fixture (`fixture-capacity-budget`, mirroring `fixture-oversized`'s existing LOC-ceiling precedent) rather than asserted only in the abstract.

## Tests

[`TEST-P0-03-02-A`](../tests/TEST-P0-03-02-A.md) — see [`REPORT-2026-07-26-14`](../reports/REPORT-2026-07-26-14.md) for the full pass record.

## Goals verified

G-RT-2 (deterministic memory behavior).
