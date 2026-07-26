# STORY-P0-04-01 — ACPI Table Parsing into the Canonical Topology Model

Status: **Verified**
Feature: [`FEAT-P0-04`](../features/FEAT-P0-04.md)
Introduced in: [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)

## Description

Locate and parse the ACPI RSDP/RSDT/XSDT/MADT tables QEMU's `q35` machine model exposes, normalizing them into the [Universal Driver Model](../../docs/universal-driver-model.md)'s internal hardware topology model (Goal G-HW-4) — the x86_64 side of a manifest format the ARM64 HAL (`EPIC-P7`) will populate differently but expose identically to the rest of the kernel.

## Depends on

`STORY-P0-01-01` (a booting kernel to run inside); genuinely independent of `FEAT-P0-02`/`FEAT-P0-03` otherwise, per the parallel-work note in `session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`.

## Acceptance criteria

1. Parsing rejects a malformed/truncated table with a typed error (`hal_x86_64::acpi::AcpiError`) rather than reading past the table's declared length — every SDT's signature, declared length, and checksum are validated before any of its body is trusted; a declared length beyond a sanity ceiling (`MAX_TABLE_LEN`, 64KiB) is rejected as `TableTooLarge` rather than trusted verbatim. `unsafe` is used (raw physical-memory reads are unavoidable for this Story), but is kept to the smallest possible operation per the Unsafe code policy — every parsing/validation function beyond the raw read itself is safe Rust operating on an already-obtained `&[u8]`.
2. The resulting topology model (`hal::topology::Topology<N>`/`CpuDescriptor`) is arch-neutral — it carries no ACPI-specific fields or types, so a future device-tree backend (ARM64, `EPIC-P7`) can produce the same output type, per the Dependency Inversion principle.
3. Verified against QEMU's real `q35` ACPI tables (Tier 0), not a hand-crafted fixture only — see [`TEST-P0-04-01-A`](../tests/TEST-P0-04-01-A.md). This surfaced a real discrepancy a fixture-only test would have missed: QEMU's PVH direct-kernel-boot loader leaves `hvm_start_info.rsdp_paddr` zeroed, so `discover_topology` falls back to the classic BIOS-era EBDA/ROM RSDP scan whenever that field is zero.

## Tests

Written test-first (red before green):

- 4 host unit tests in `os/src/hal/src/topology.rs` (`#[cfg(test)]`) covering `Topology`'s fixed-capacity push/iterate/overflow/disabled-entry behavior.
- 19 host unit tests in `os/src/hal-x86_64/src/acpi.rs` (`#[cfg(test)]`) covering the classic BIOS-era RSDP scan, RSDP v1/v2 parsing and checksum validation, SDT header/checksum/length validation, XSDT/RSDT entry parsing, MADT Processor Local APIC entry parsing (including truncated-entry and overflow rejection), and two end-to-end `discover_topology` runs against hand-crafted, address-linked stack fixtures.
- [`TEST-P0-04-01-A`](../tests/TEST-P0-04-01-A.md) — Tier 0 QEMU integration test: `kernel_main` calls `discover_topology` against QEMU's own `q35`-generated ACPI tables and only reaches the `isa-debug-exit` success code if real parsing succeeds and at least one CPU is discovered.

See [`REPORT-2026-07-26-06`](../reports/REPORT-2026-07-26-06.md) for the full verification run.

## Goals verified

G-HW-4 (unified hardware description).
