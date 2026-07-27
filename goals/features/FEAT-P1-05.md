# FEAT-P1-05 — Hostile-Load & Exhaustion-Containment Proof

Status: **Specified — no Story started**
Epic: [`EPIC-P1`](../epics/EPIC-P1.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

The first *adversarial-by-design* Feature (Goal **G-SEC-12**; `SeedMVP.md` §7's "hostile-load" test type, introduced this phase): prove — with dated raw evidence, not architecture prose — that saturating every bounded thing `EPIC-P0` built (pools, queues, spoor journal, IPC channels, task slots, the scheduler's ready queue) degrades the *attacker's* service and nobody else's. Exhaustion must be contained to the offender's declared budget: RT tasks keep their reserves and deadlines under a C2-class flood, denial responses are themselves bounded (no amplification), recovery after the flood stops is bounded and complete, and every denial is attributable via spoor. This Feature is also where property-based tests (SeedMVP §7, this phase) enter: "no interleaving of hostile allocations can starve an RT reserve" is a property, not an example.

## Crate(s) involved

`os/src/kernel/` (hostile-load fixtures, any budget-accounting gaps they expose), `os/src/xtask/` (campaign harness), potentially `os/src/exec/` (loader-facing exhaustion probes)

## Depends on

`FEAT-P1-04` (RT reserves under load are only meaningful with real preemption and deadline enforcement), `FEAT-P1-01` (degradation is measured against baselines, not eyeballed).

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P1-05-01`](../stories/STORY-P1-05-01.md) | Hostile-load campaign: saturation, RT-reserve preservation, bounded recovery, attributable denial | Specified |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0/C1** · subject **C1/C2/C3/C4** · boundary tests **BND-15, -16, -20**.

The load generator plays a compromised C2 component and must prove single-compromise insufficiency (`BND-20`) for the exhaustion vector specifically: one flooding domain cannot consume another class's budget or any RT reserve (`BND-15`), cannot buy priority with class (`BND-16`), and cannot make the kernel's own denial path unbounded (`PD-08`/`PD-09` — denial work is charged to the caller).

## Exit criteria

The Story **Verified** at Tier 0 with a dated Report containing raw campaign evidence — the first Report in the repository whose primary content is adversarial-load data rather than functional pass/fail — and `SEC-20`'s state on this Story converting from `baseline-debt` to `verified` for the Tier 0 scope (hardware-tier debt stays named).
