# FEAT-P0-03 — Static/Pool Memory Allocator

Status: **Planned, not yet decomposed into Stories**
Epic: [`EPIC-P0`](../epics/EPIC-P0.md)
Introduced in: [`session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md`](../../session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md)

## Description

The deterministic memory model required by Goal **G-RT-2**: static or pool-based allocation in real-time paths, no unbounded heap fragmentation — per [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md#real-time-discipline-kernel-and-driver-code)'s "no heap allocation in any scheduler, IPC, or interrupt-handling hot path" rule.

## Crate(s) involved

`os/src/kernel/`

## Depends on

`FEAT-P0-01`.

## Status

Not yet decomposed into Stories. When decomposed, Stories here should cover at minimum: a bounded-capacity pool allocator type, compile-time pool-size configuration, and an allocation-failure path that fails closed (per Non-Negotiable #5) rather than panicking silently.
