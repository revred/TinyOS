# FEAT-P0-03 — Static/Pool Memory Allocator

Status: **Verified — 3/3 Stories Verified**
Epic: [`EPIC-P0`](../epics/EPIC-P0.md)
Introduced in: [`session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md`](../../session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md); decomposed in [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)

## Description

The deterministic memory model required by Goal **G-RT-2**: static or pool-based allocation in real-time paths, no unbounded heap fragmentation — per [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md#real-time-discipline-kernel-and-driver-code)'s "no heap allocation in any scheduler, IPC, or interrupt-handling hot path" rule.

## Crate(s) involved

`os/src/kernel/`

## Depends on

`FEAT-P0-01`.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P0-03-01`](../stories/STORY-P0-03-01.md) | Bounded-capacity pool allocator type (`Pool<T, N>`) | Verified |
| [`STORY-P0-03-02`](../stories/STORY-P0-03-02.md) | Compile-time pool-size configuration | Verified |
| [`STORY-P0-03-03`](../stories/STORY-P0-03-03.md) | Allocation-failure path fails closed | Verified |

`STORY-P0-03-02` implemented and Verified in [`session/hand-2026-07-26/19-story-p0-03-02-capacity-configuration-implementation.md`](../../session/hand-2026-07-26/19-story-p0-03-02-capacity-configuration-implementation.md) — a new `kernel::capacities` module consolidating `MAX_CPUS` and `EXEC_FRAME_POOL_CAPACITY` (previously scattered/duplicated), plus a `const`-evaluated `STATIC_MEMORY_BUDGET_BYTES` check proven to actually fail a build when violated (`fixture-capacity-budget`, a new governance-fixture-test case). Per that Story's own scope-resolution note, only capacities with a real, concrete call site today are declared — the task-control-block/IPC/demand-paging pools this Feature's own "Note on scope beyond Phase 0" below anticipates have none yet, so adding placeholders for them was deliberately deferred.

## Note on scope beyond Phase 0

Per [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md), this Feature is also the direct prerequisite for Phase 6's mmap/pointer-based model-file access (TinyOS's own kernel virtual-memory manager, demand-paging a file-backed region) — whoever extends `Pool<T, N>` toward a virtual-memory allocator should keep that consumer in mind, not just the RT task/IPC pool use case this Feature's own title suggests. This is also recorded in project memory (`inference-mmap-model-loading`).

## Exit criteria

`STORY-P0-03-01` through `-03` all reach **Verified**. **Met**, as of 2026-07-26 (`-01`/`-03`: `REPORT-2026-07-26-04`; `-02`: `REPORT-2026-07-26-14`).
