# EPIC-P0 — Kernel Skeleton

Status: **Planned**
Roadmap phase: [Phase 0 — Kernel skeleton](../../README.md#roadmap)
Introduced in: [`session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md`](../../session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md)

## Goal

Boot, context switch, preemptive priority scheduler, static memory pools, and a minimal HAL for one x86_64 target — the walking skeleton described in [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md#delivery-strategy-walking-skeleton-first), proven end to end (`cargo build` → boot in QEMU → halt cleanly → CI green) before any further feature work begins.

## Goals verified (from `SeedMVP.md` §3)

- **G-RT-1** — Preemptive, priority-based scheduling with bounded interrupt latency and documented priority-inversion avoidance.
- **G-RT-2** — Deterministic memory behavior: no unbounded heap fragmentation or allocation-time variance on any RT-scheduled path.
- **G-RT-7** — 64-bit-only, two-architecture portability (this Epic establishes the x86_64 side; ARM64 HAL follows in `EPIC-P7`).
- **G-HW-4** — Unified hardware description: ACPI (x86_64) normalized into TinyOS's internal hardware topology model.

## Hardware & test tier

Per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix): Tier 0 (QEMU x86_64) is the primary environment for this Epic. Tier 2 (the x86_64 mini-PC) HIL validation follows once the walking skeleton is stable in Tier 0 and the hardware has been purchased.

## Features

| Feature | Summary | Status |
|---|---|---|
| [`FEAT-P0-01`](../features/FEAT-P0-01.md) | Workspace bootstrap & walking skeleton (build → QEMU boot → halt → CI green) | Planned |
| [`FEAT-P0-02`](../features/FEAT-P0-02.md) | Preemptive priority scheduler core | Planned, not yet decomposed into Stories |
| [`FEAT-P0-03`](../features/FEAT-P0-03.md) | Static/pool memory allocator | Planned, not yet decomposed into Stories |
| [`FEAT-P0-04`](../features/FEAT-P0-04.md) | x86_64 HAL backend & ACPI manifest normalization | Planned, not yet decomposed into Stories |

`FEAT-P0-01` is decomposed first and fully, since it's the literal next step (the walking skeleton itself). The other three features are scoped but intentionally left un-decomposed into Stories until work on each begins, per the just-in-time principle in the [goals dashboard](../index.html#jit-decomposition).

## Exit criteria for this Epic

- `FEAT-P0-01` through `FEAT-P0-04` all reach **Verified** status (passing tests, filed Reports, at minimum Tier 0).
- The crate-size ceiling and SOLID review checklist (per `agent/CODING_STANDARDS.md`) have been exercised on every PR in this Epic, not just claimed compliant retroactively.
- `EPIC-P1` (Determinism proof) can begin — it depends on this Epic's scheduler and timing infrastructure existing at all.
