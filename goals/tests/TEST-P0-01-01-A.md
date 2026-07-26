# TEST-P0-01-01-A — QEMU Boot-to-Halt Integration Test

Status: **Planned (not yet implemented)**
Story: [`STORY-P0-01-01`](../stories/STORY-P0-01-01.md)
Tier: Tier 0 (QEMU x86_64), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)

## Specification

Per the TDD mandate, this specification is written before the kernel code it will verify.

**Given** the `kernel` crate built against `os/targets/x86_64-tinyos.json`,
**when** it is booted under QEMU x86_64 (`q35` machine type) via `xtask qemu-x86_64`,
**then** the boot process reaches a halt instruction within a bounded time budget (to be fixed once real boot timing is measured; tracked as an open value here, not a placeholder to forget), with:
- no panic message on the serial console,
- no unexpected output before the halt,
- a distinguishable success exit code from the `xtask` harness.

## Test type

Integration test (Tier 0, QEMU-based), per [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md#test-driven-development-mandatory)'s requirement that every driver/kernel path targets at minimum a Tier 0 test.

## Implementation location (once written)

`os/src/tests/` or co-located with `os/src/kernel/` — to be finalized when `FEAT-P0-01` work begins; this entry will be updated with the actual path.

## Reports

None yet — this test has not been implemented or run. The first Report filed against this Test will be linked here, most recent first.
