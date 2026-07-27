# FEAT-P1-06 — Deterministic Actuation Proof (G-PA-1 Flagship Path)

Status: **Specified — no Story started**
Epic: [`EPIC-P1`](../epics/EPIC-P1.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

The Epic's integration exit (Goal **G-PA-1**): one end-to-end path from *decision* (an RT task computes an actuation command) to *actuation* (the command reaches an output boundary — under Tier 0, a measurable I/O port/MMIO write standing in for a real actuator line) with a **scheduler-enforced** worst-case latency bound: the actuation task's WCET budget and deadline are declared, the deadline monitor (`FEAT-P1-04`) enforces them, a deliberate overrun trips the declared fault policy, and the measured decision-to-actuation distribution (via `FEAT-P1-01`) sits inside the declared bound with the margin recorded. "Enforced by the scheduler, not merely observed in testing" is the goal's own wording — the proof must show the *enforcement* firing, not only clean runs.

This is the primitive the `G-PA-8` 5-axis CNC flagship milestone (a cross-`EPIC-P0`–`P3` checkpoint, per the backlog) eventually stacks G-code parsing, motion planning, and real I/O onto. Here it is one task, one output, one bound — deliberately minimal, so the determinism claim is attributable to the kernel rather than to application structure.

## Crate(s) involved

`os/src/kernel/` (the actuation task fixture, budget declaration), `os/src/hal-x86_64/` (the bounded output primitive), `os/src/xtask/` (end-to-end measurement)

## Depends on

`FEAT-P1-04` (deadline enforcement is the claim), `FEAT-P1-01` (the measurement), `FEAT-P1-02` (the overrun path). Composes with `FEAT-P1-03` when both are done (actuation from a task in its own address space) — worth demonstrating but not gating on.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P1-06-01`](../stories/STORY-P1-06-01.md) | Bounded decision-to-actuation path: declared budget, enforced deadline, measured distribution, demonstrated overrun trip | Specified |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0/C1** · subject **C1/C2** · boundary tests **BND-15, -16, -17**.

Actuation authority is explicit: only the declared actuation task can reach the output primitive (no ambient path to it), the enforcement decision on overrun is a spoor, and no load elsewhere in the system (see `FEAT-P1-05`) may widen the actuation latency distribution beyond its declared bound — that cross-Feature composition is part of the evidence, not an afterthought.

## Exit criteria

The Story **Verified** at Tier 0: measured distribution inside the declared bound under idle *and* under `FEAT-P1-05`'s hostile load, enforcement demonstrated by a deliberate overrun tripping its policy, and the standing hardware-tier debt named — a QEMU-measured bound is the mechanism's proof, the boards' numbers are the product's, and the Report must keep that distinction explicit.
