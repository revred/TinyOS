# FEAT-P0-01 — Workspace Bootstrap & Walking Skeleton

Status: **Verified** (locally; CI run pending — see linked Reports on each Story/Test)
Epic: [`EPIC-P0`](../epics/EPIC-P0.md)
Introduced in: [`session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md`](../../session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md)

## Description

The first milestone, per [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md#delivery-strategy-walking-skeleton-first): the smallest possible end-to-end slice through the whole pipeline, proven before any real kernel logic exists. Success is `cargo run -p xtask -- qemu-x86_64` booting an empty `kernel` crate to a clean halt with no panic, with CI reporting green on format, lint, the crate-size ceiling check, and `missing_docs`.

## Crate(s) involved

`os/src/kernel/` (empty shell), `os/src/hal/` + `os/src/hal-x86_64/` (empty shells), `os/src/xtask/`, `os/targets/x86_64-tinyos.json`.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P0-01-01`](../stories/STORY-P0-01-01.md) | Empty kernel crate boots in QEMU x86_64 and halts cleanly | Verified |
| [`STORY-P0-01-02`](../stories/STORY-P0-01-02.md) | CI pipeline runs format/lint/crate-size-ceiling checks on every PR | Verified |
| [`STORY-P0-01-03`](../stories/STORY-P0-01-03.md) | `xtask qemu-x86_64` command builds and launches the kernel under QEMU | Verified |

## Exit criteria

All three Stories reach **Verified**. Met locally on 2026-07-26 (see `goals/reports/REPORT-2026-07-26-01` through `-03`); CI has not yet run against this work. This unblocks `FEAT-P0-02` (scheduler), since scheduler work needs a booting kernel to run inside.
