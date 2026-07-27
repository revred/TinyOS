# FEAT-P0-04 — x86_64 HAL Backend & ACPI Manifest Normalization

Status: **Complete — 3/3 Stories Verified**
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
| [`STORY-P0-04-02`](../stories/STORY-P0-04-02.md) | Interrupt controller (local APIC) bring-up | Verified |
| [`STORY-P0-04-03`](../stories/STORY-P0-04-03.md) | Minimal bus-enumeration pass | Verified |

`STORY-P0-04-01` implemented and Verified in [`session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md`](../../session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md) once QEMU became available in the working environment (see that session's handover for what changed from the prior "no QEMU" constraint). `STORY-P0-04-02` implemented and Verified in [`session/hand-2026-07-26/32-story-p0-04-02-idt-apic-bring-up.md`](../../session/hand-2026-07-26/32-story-p0-04-02-idt-apic-bring-up.md) — scoped to the local APIC only (I/O APIC device-IRQ routing deferred to `-03` or a later driver Story, see that Story's own scope-correction note). `STORY-P0-04-03` implemented and Verified in [`session/hand-2026-07-26/35-story-p0-04-03-pci-bus-enumeration.md`](../../session/hand-2026-07-26/35-story-p0-04-03-pci-bus-enumeration.md) — read-only legacy-CAM bus-0 enumeration into `hal::device::DeviceTable`; I/O APIC device-IRQ routing remains deferred to a later driver Story (per `-03`'s own *Named, not silently solved* list), so this Feature closes with discovery complete and device interrupt routing explicitly out of scope.

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0/C1** · subject **C2** · boundary tests **BND-02, -06, -07, -08, -18, -20**.

That row also selects this Feature’s [`PD-*`](../security/protection-domain-contracts.tsv) and [`RCG-*`](../security/code-admission-gates.tsv) Security Charter obligations. Every Test repeats the exact selections and CI rejects drift.

Firmware discovery describes hardware but cannot select a driver or grant DMA, IRQ, MMIO, priority, or process-memory authority. Complex device protocols and vendor drivers remain isolated C2 services; C1 accepts only normalized fixed-format results. Required evidence includes bounded hostile firmware parsing, zero implicit device grants, IOMMU/bounce-buffer confinement, crash/reset/hot-unplug revocation, negative unselected-driver surface, and malicious-device containment.

## Exit criteria

`STORY-P0-04-01` through `-03` all reach **Verified**.
