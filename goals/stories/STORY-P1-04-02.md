# STORY-P1-04-02 — Deadline Monitor & WCET Watchdog on the Real Timer

Status: **Specified, not yet started**
Feature: [`FEAT-P1-04`](../features/FEAT-P1-04.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

`kernel::wcet::record_tick` stops being a bookkeeper waiting for a clock: the real timer drives per-task budget accounting, and an overrun triggers the task's declared fault policy — restart, degrade, or trip to safe state — through `FEAT-P1-02`'s fault machinery, with a spoor for every enforcement decision. This closes the "no timer, no watchdog" structural gap `STORY-P0-02-04` named and twice re-surfaced, and gives `G-RT-3` its enforcement half: budgets are held by the scheduler, not just declared.

## Depends on

`STORY-P1-04-01`; `STORY-P1-02-01` (the policy lands in real fault handling).

## Acceptance criteria (draft — to be finalized when this Story starts)

1. A task deliberately exceeding its declared WCET budget is caught within a bounded number of ticks and its declared policy fires — one fixture per policy arm (restart, degrade, trip), each observable under Tier 0.
2. Enforcement never punishes the innocent: a within-budget RT task on the same core keeps its deadlines while the offender is handled (measured, not asserted).
3. Every overrun decision is a spoor with class/actor/action/outcome; silent overruns are structurally impossible (the accounting path has no "ignore" branch).

## Tests

Not yet written — deferred until this Story starts. Requires Tier 0 overrun fixtures per policy arm.

## Goals verified

G-RT-3 (enforcement half), G-PA-1 (groundwork), G-SEC-14.
