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
- **G-PC-1** through **G-PC-4** — Native (PE64) executable loading, a minimal explicitly-enumerated compatibility surface, no privileged bypass for a loaded executable, and a small auditable loader TCB — validated against the flagship `Sharc.Blue` `blue-sharc.exe` use case.
- **G-DX-8** — Bounded total OS image size (≤8MB, excluding drivers) — a standing constraint across this whole Epic, most directly exercised by `FEAT-P0-05`.
- **G-PA-6** (physical process auditability) and **G-AI-3** (identical audit trail for every caller type) — `FEAT-P0-06`'s spoor primitive is the foundational, no-heap audit mechanism these goals will build on once there's a physical process or an AI caller to audit; introduced here so every later subsystem has it available from the start rather than retrofitting audit trails in per-subsystem.

## Hardware & test tier

Per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix): Tier 0 (QEMU x86_64) is the primary environment for this Epic. Tier 2 (the x86_64 mini-PC) HIL validation follows once the walking skeleton is stable in Tier 0 and the hardware has been purchased.

## Features

| Feature | Summary | Status |
|---|---|---|
| [`FEAT-P0-01`](../features/FEAT-P0-01.md) | Workspace bootstrap & walking skeleton (build → QEMU boot → halt → CI green) | Verified (locally; CI run pending) |
| [`FEAT-P0-02`](../features/FEAT-P0-02.md) | Preemptive priority scheduler core | Verified locally — 5/5 Stories (one structural gap remains, see `FEAT-P0-02.md`) |
| [`FEAT-P0-03`](../features/FEAT-P0-03.md) | Static/pool memory allocator | Verified locally — 3/3 Stories |
| [`FEAT-P0-04`](../features/FEAT-P0-04.md) | x86_64 HAL backend & ACPI manifest normalization | In progress — `STORY-P0-04-01` Verified locally |
| [`FEAT-P0-05`](../features/FEAT-P0-05.md) | Executable loading & process compatibility (`Sharc.Blue` PE loader) | In progress — `STORY-P0-05-01`/`-02`/`-03` Verified locally |
| [`FEAT-P0-06`](../features/FEAT-P0-06.md) | Spoor: universal 64-bit audit atom | Verified locally — 2/2 Stories |
| [`FEAT-P0-07`](../features/FEAT-P0-07.md) | Local IPC (shared memory & message channels) | Not yet started |

`FEAT-P0-01` is decomposed first and fully, since it's the literal next step (the walking skeleton itself). `FEAT-P0-05` depends on `FEAT-P0-02` (a task to run a loaded executable as) and `FEAT-P0-03` (memory to map its sections into) reaching enough maturity to host a real process, not just a skeleton — see `FEAT-P0-05.md`'s own dependency note for exactly how much of each is needed before implementation can begin. `FEAT-P0-06` (added 2026-07-26, per a new strategic objective — spoor should be "universal," borrowed from `Sharc.Blue`) is deliberately added inside this Epic rather than deferred to the backlog: it's a foundational, no-heap, no-allocation observability primitive every other Phase 0 subsystem (scheduler, lock, exec, and the IPC work `FEAT-P0-07` adds) can and should use from as early as possible, the same reasoning that put `FEAT-P0-03`'s pool allocator in Phase 0 rather than later. `FEAT-P0-07` (added the same session, per the same batch of strategic objectives — local IPC/shared-memory/socket communication) is `docs/mvp-delivery-strategy.md`'s own crate-map line for `kernel` ("Scheduler, IPC, memory pools, deadline monitor") finally decomposed — scoped deliberately to *local* IPC only; a real network-facing TCP/IP stack is a separate, later Feature under `EPIC-P3` (Connectivity), not this Epic, per `FEAT-P0-07.md`'s own scope-boundary note.

## Exit criteria for this Epic

- `FEAT-P0-01` through `FEAT-P0-07` all reach **Verified** status (passing tests, filed Reports, at minimum Tier 0).
- The crate-size ceiling and SOLID review checklist (per `agent/CODING_STANDARDS.md`) have been exercised on every PR in this Epic, not just claimed compliant retroactively.
- `EPIC-P1` (Determinism proof) can begin — it depends on this Epic's scheduler and timing infrastructure existing at all.
