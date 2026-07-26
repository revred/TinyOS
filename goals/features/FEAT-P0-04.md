# FEAT-P0-04 — x86_64 HAL Backend & ACPI Manifest Normalization

Status: **In progress — 1/3 Stories Verified**
Epic: [`EPIC-P0`](../epics/EPIC-P0.md)
Introduced in: [`session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md`](../../session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md); decomposed in [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)

## Description

The x86_64 side of the [Universal Driver Model](../../docs/universal-driver-model.md)'s unified hardware manifest (Goal **G-HW-4**): normalizing ACPI tables into TinyOS's internal hardware topology model, and the minimal x86_64-specific HAL backend (bus enumeration, interrupt routing) the kernel and scheduler need to run on real hardware, not just QEMU's default machine model.

## Crate(s) involved

`os/src/hal/`, `os/src/hal-x86_64/`

## Depends on

`FEAT-P0-01`.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P0-04-01`](../stories/STORY-P0-04-01.md) | ACPI table parsing into the canonical topology model | Verified |
| [`STORY-P0-04-02`](../stories/STORY-P0-04-02.md) | Interrupt controller (APIC) bring-up | Planned, not yet started |
| [`STORY-P0-04-03`](../stories/STORY-P0-04-03.md) | Minimal bus-enumeration pass | Planned, not yet started |

`STORY-P0-04-01` implemented and Verified in [`session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md`](../../session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md) once QEMU became available in the working environment (see that session's handover for what changed from the prior "no QEMU" constraint). `STORY-P0-04-02`/`-03` remain Planned — both also require QEMU-based Tier 0 verification and are natural next Stories now that the harness is confirmed working end to end, including a real ACPI-derived topology.

## Exit criteria

`STORY-P0-04-01` through `-03` all reach **Verified**.
