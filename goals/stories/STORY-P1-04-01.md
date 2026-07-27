# STORY-P1-04-01 — Timer-Driven Preemption

Status: **Specified, not yet started**
Feature: [`FEAT-P1-04`](../features/FEAT-P1-04.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

The armed-but-unconsumed local-APIC timer finally gets its consumer: the tick ISR invokes a preemption decision (highest-priority-ready wins, per the existing `Scheduler` ordering), performing an interrupt-driven context switch instead of waiting for cooperative yield — converting `kernel::dispatch` from cooperative-only to genuinely preemptive. The priority-inheriting lock's behavioral half (boost actually preventing inversion under real preemption) becomes testable and tested here, closing `STORY-P0-02-03`'s host-only caveat.

## Depends on

`STORY-P1-02-01` (a preemption path that faults lands in a real handler); `STORY-P1-01-01` (tick-to-dispatch latency is measured, D03/D05).

## Acceptance criteria (draft — to be finalized when this Story starts)

1. A busy-looping low-priority task is preempted by a newly-ready high-priority task within a measured, bounded number of ticks under Tier 0 — no cooperative yield anywhere in the fixture.
2. Priority inversion demonstrably avoided: low holds lock, high blocks, medium spins — high proceeds within its bound because low is boosted (the classic three-task scenario, run for real).
3. Interrupt-context discipline: the ISR-side work is bounded and allocation-free, with the heavy lifting on the switch path, per `agent/CODING_STANDARDS.md`'s RT rules.

## Tests

Not yet written — deferred until this Story starts. Requires Tier 0 preemption and inversion fixtures.

## Goals verified

G-RT-1 (preemption + inversion avoidance, behaviorally).
