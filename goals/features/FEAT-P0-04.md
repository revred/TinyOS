# FEAT-P0-04 — x86_64 HAL Backend & ACPI Manifest Normalization

Status: **Planned, not yet decomposed into Stories**
Epic: [`EPIC-P0`](../epics/EPIC-P0.md)
Introduced in: [`session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md`](../../session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md)

## Description

The x86_64 side of the [Universal Driver Model](../../docs/universal-driver-model.md)'s unified hardware manifest (Goal **G-HW-4**): normalizing ACPI tables into TinyOS's internal hardware topology model, and the minimal x86_64-specific HAL backend (bus enumeration, interrupt routing) the kernel and scheduler need to run on real hardware, not just QEMU's default machine model.

## Crate(s) involved

`os/src/hal/`, `os/src/hal-x86_64/`

## Depends on

`FEAT-P0-01`.

## Status

Not yet decomposed into Stories. When decomposed, Stories here should cover at minimum: ACPI table parsing into the canonical topology model, interrupt controller (APIC) bring-up, and a minimal bus-enumeration pass sufficient for the class drivers planned in `EPIC-P3`.
