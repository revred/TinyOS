# STORY-P1-05-01 — Hostile-Load Campaign: Saturation, RT Reserves, Bounded Recovery

Status: **Specified, not yet started**
Feature: [`FEAT-P1-05`](../features/FEAT-P1-05.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

A flooding fixture playing a compromised C2 component saturates, in turn and in combination: pool allocation, task-slot creation, spoor-journal writes, IPC channels/grants, and ready-queue churn — while a declared RT task runs its deadline workload on the same core. The claim under test is `G-SEC-12`'s, verbatim: every bound holds, RT reserves are priority-safe, denial is bounded and attributable, recovery after the flood is bounded and complete. Property-based tests (new this phase) state the invariant over interleavings, not examples: no schedule of hostile allocations may starve an RT reserve.

## Depends on

`STORY-P1-04-01`/`-02` (real preemption and deadline enforcement are what "RT reserves survive" means); `STORY-P1-01-02` (degradation measured against committed baselines).

## Acceptance criteria (draft — to be finalized when this Story starts)

1. Under each saturation vector and their combination, the RT task's measured deadline-hit rate and latency distribution stay within its declared bound — raw distributions in the Report, idle-vs-flood side by side.
2. Every denial is `Err`, bounded in cost, charged to the offender, and spoor-attributed; no denial path allocates, amplifies, or blocks unboundedly.
3. When the flood stops, recovery to baseline is bounded and complete (no leaked slots, no stuck queues, no residual degradation) — measured, with the recovery-time bound recorded.
4. The property-based invariant runs in CI at host tier with a recorded seed policy; Tier 0 carries the behavioral campaign.

## Tests

Not yet written — deferred until this Story starts. Requires host property tests plus a Tier 0 campaign fixture.

## Goals verified

G-SEC-12; G-SEC-14 (attributable denial); first `baseline-debt` → `verified` conversion candidate for `SEC-20`.
