# STORY-P1-03-01 — Per-Task `CR3` Switching in the Context Switch

Status: **Specified, not yet started**
Feature: [`FEAT-P1-03`](../features/FEAT-P1-03.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

Install each task's `exec::AddressSpace` on switch-in: the TCB gains an address-space handle, `context::switch` loads the incoming task's `CR3` (skipping the reload when unchanged — same-space switches must not pay the TLB cost), kernel mappings are present in every space so the switch path itself never faults, and the boot identity map is demoted to early-boot bootstrap only. **Hard precondition:** `FEAT-P1-02`'s fault handlers exist first, per the standing Handover 32/33/35 ordering — this Story does not start until they are Verified.

## Depends on

`STORY-P1-02-01`/`-02` (hard, see above); `STORY-P1-01-01` (D04 baseline absorbs the CR3 cost measurably).

## Acceptance criteria (draft — to be finalized when this Story starts)

1. Two tasks in distinct address spaces each run and switch under Tier 0; a cross-space memory probe from one *faults* and is contained by the `#PF` handler (the other task keeps running) — isolation proven adversarially, not inferred from the mapping tables.
2. Same-space switches skip the `CR3` reload, and the measured D04 delta between same-space and cross-space switches is recorded against the catalogue budget.

## Tests

Not yet written — deferred until this Story starts. Requires Tier 0 two-task isolation fixtures.

## Goals verified

G-SEC-2 (active address spaces), G-RT-1 (switch cost stays within budget).
