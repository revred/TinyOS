# FEAT-P1-03 — Active Per-Task Address Spaces, W^X & Teardown

Status: **Specified — no Story started**
Epic: [`EPIC-P1`](../epics/EPIC-P1.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

Make **G-SEC-2** ("process memory is private *by construction*") an active runtime fact instead of dormant machinery: `EPIC-P0` built `exec::AddressSpace` page tables that are constructed and verified but never installed — every task still runs on the boot identity map, all-RWX. This Feature switches `CR3` per task in the context switch, replaces the all-RWX identity map with W^X/NX mappings (executable memory never writable, writable never executable — the Security Charter's `PD-04` executable sealing, boundary test `BND-05`), and implements generation-safe teardown (`PD-13`: revoke, wipe, advance generation before any reuse). This closes three named items on the Security Charter's runtime-evidence gap list: active per-task address spaces, executable sealing, and teardown.

## Crate(s) involved

`os/src/hal-x86_64/` (CR3 install, kernel-mapping W^X split), `os/src/kernel/` (per-task address-space handle in the TCB, context-switch integration, teardown), `os/src/exec/` (`AddressSpace` becomes the *installed* space, not just a verified artifact)

## Depends on

`FEAT-P1-02` (**hard ordering** — no live CR3 switch before a real fault handler exists, per Handovers 32/33/35), `FEAT-P1-01` (context-switch-with-CR3 cost gets a measured baseline against the D04 budget).

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P1-03-01`](../stories/STORY-P1-03-01.md) | Per-task `CR3` switching wired into the context switch | Specified |
| [`STORY-P1-03-02`](../stories/STORY-P1-03-02.md) | W^X/NX kernel + task mappings; generation-safe address-space teardown | Specified |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0/C1** · subject **C1/C2/C3/C4** · boundary tests **BND-04, -05, -20**.

Cross-domain reads/writes/executes/remaps must *fault* (landing in `FEAT-P1-02`'s handlers), not merely be absent from the happy path — the adversarial fixtures actively attempt them. No mapping is ever writable and executable; no teardown-freed frame is reusable before wipe + generation advance; the boot identity map survives only as the early-boot bootstrap, never as a running task's view.

## Exit criteria

Both Stories **Verified** at Tier 0 with adversarial evidence: a task provably cannot touch another task's memory (the attempt faults and is contained by `FEAT-P1-02`), W^X violations fault, teardown-then-probe fixtures prove generation safety, and the D04/D08 timing baselines absorb the CR3-switch cost within budget.
