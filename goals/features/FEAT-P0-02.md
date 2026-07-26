# FEAT-P0-02 — Preemptive Priority Scheduler Core

Status: **Planned, not yet decomposed into Stories**
Epic: [`EPIC-P0`](../epics/EPIC-P0.md)
Introduced in: [`session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md`](../../session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md)

## Description

The preemptive, priority-based scheduler with bounded interrupt latency and priority inheritance/ceiling protocols described in [`README.md`](../../README.md#1-a-real-multitasking-rtos-core) Design Pillar 1, and required by Goal **G-RT-1**.

## Crate(s) involved

`os/src/kernel/`

## Depends on

`FEAT-P0-01` (needs a booting kernel to run the scheduler inside).

## Status

Not yet decomposed into Stories — per the [goals dashboard](../index.html#jit-decomposition), decomposition happens when work on this Feature is about to start (immediately after `FEAT-P0-01` reaches Verified). When decomposed, Stories here should cover at minimum: task creation/priority assignment, context switch, priority inheritance on lock contention, and WCET budget enforcement.
