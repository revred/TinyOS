# TEST-P0-04-01-A — ACPI Topology Discovery Against Real QEMU `q35` Tables

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-04-01`](../stories/STORY-P0-04-01.md)
Tier: Tier 0 (QEMU x86_64), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)

## Specification

Per the TDD mandate, this specification is written before the kernel code it verifies is trusted to be correct against real firmware — the host-side unit tests in `hal-x86_64/src/acpi.rs` (hand-crafted fixtures) were themselves written before their implementation, but per `STORY-P0-04-01`'s acceptance criterion 3, a hand-crafted fixture is a useful additional test, not a replacement for testing against what QEMU actually emits.

**Given** the `kernel` crate built against `os/targets/x86_64-tinyos.json`, linking `hal-x86_64::acpi::discover_topology`,
**when** it is booted under QEMU x86_64 (`q35` machine type, PVH direct-kernel-boot) via `xtask qemu-x86_64`,
**then**:
- `kernel_main` locates the RSDP starting from the PVH `hvm_start_info.rsdp_paddr` field, falling back to the classic BIOS-era EBDA/ROM signature scan when that field reads as zero (QEMU's PVH loader does not populate it — discovered during this Test's own implementation pass, not assumed in advance),
- walks RSDP → XSDT (or RSDT) → MADT, validating every table's signature, declared length, and checksum,
- parses at least one Processor Local APIC entry into a `hal::topology::Topology`,
- and reaches the `isa-debug-exit` success code — a parsing failure at any step (bad checksum, table not found, zero CPUs discovered) instead reaches the failure code, so this Test cannot pass by accident (e.g. a discovery function that always returns `Ok` regardless of input would fail this Test, unlike a Test that only checks "did the kernel not crash").

## Test type

Integration test (Tier 0, QEMU-based), per [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md#test-driven-development-mandatory)'s requirement that every driver/kernel path targets at minimum a Tier 0 test. Complements (does not replace) the 19 host-side unit tests in `os/src/hal-x86_64/src/acpi.rs` and 4 in `os/src/hal/src/topology.rs`, which exercise the same parsing/validation logic against hand-crafted byte fixtures — including the raw-pointer `unsafe` boundary functions, using real (if fake-content) stack addresses, wherever that's memory-safe to do on a 64-bit host (the RSDT-only path, whose entries are inherently 32-bit physical addresses, is exercisable only under QEMU or real hardware — see the code comment in `acpi.rs`'s test module).

## Implementation location

- `os/src/hal/src/topology.rs` — the arch-neutral `Topology`/`CpuDescriptor` output type.
- `os/src/hal-x86_64/src/acpi.rs` — RSDP location (PVH `hvm_start_info` + BIOS-era EBDA/ROM scan fallback), XSDT/RSDT/MADT parsing, `discover_topology`.
- `os/src/kernel/src/boot.rs` — passes the PVH `hvm_start_info` physical address (originally in `EBX`) into `kernel_main` via `EDI`/`RDI`, per the SysV x86-64 calling convention.
- `os/src/kernel/src/main.rs` — `kernel_main` calls `discover_topology` and maps success/failure to the `isa-debug-exit` code, exercised by `os/src/xtask/src/main.rs`'s `qemu-x86_64` command.

## Reports

- [`REPORT-2026-07-26-06`](../reports/REPORT-2026-07-26-06.md) — Pass (local, Tier 0/QEMU, plus 23 host unit tests).
