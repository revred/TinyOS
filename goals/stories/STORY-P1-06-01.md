# STORY-P1-06-01 — Bounded Decision-to-Actuation Path, Scheduler-Enforced

Status: **Specified, not yet started**
Feature: [`FEAT-P1-06`](../features/FEAT-P1-06.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

One RT task computes an actuation command and emits it to a bounded output primitive (under Tier 0, a timestamped I/O-port/MMIO write standing in for an actuator line); the task's WCET budget and deadline are declared, the deadline monitor enforces them, and the decision-to-actuation latency distribution is measured end to end. `G-PA-1`'s own wording is the bar: the bound is *enforced by the scheduler, not merely observed in testing* — so a deliberate overrun tripping the declared policy is part of the proof, alongside clean-run distributions under idle and under `STORY-P1-05-01`'s hostile load.

## Depends on

`STORY-P1-04-02` (enforcement), `STORY-P1-01-02` (measurement + baselines), `STORY-P1-05-01` (the under-load half of the evidence).

## Acceptance criteria (draft — to be finalized when this Story starts)

1. Measured decision-to-actuation distribution (p50/p99/p99.9/max) inside the declared bound, under idle and under hostile load, with raw data in the Report.
2. A deliberately-overrunning actuation task trips its declared policy before emitting a late command — late actuation is *prevented*, not logged.
3. Only the declared actuation task can reach the output primitive (no ambient path), and every enforcement decision is a spoor.
4. The Report names Tier 1/Tier 2 hardware measurement as explicit open debt: this Story proves the mechanism under QEMU; the boards prove the product's numbers.

## Tests

Not yet written — deferred until this Story starts. Requires a Tier 0 end-to-end actuation fixture with overrun and under-load variants.

## Goals verified

G-PA-1; the primitive `G-PA-8`'s CNC flagship milestone later builds on.
