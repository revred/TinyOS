# TEST-P0-01-01-A — QEMU Boot-to-Halt Integration Test

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-01-01`](../stories/STORY-P0-01-01.md)
Tier: Tier 0 (QEMU x86_64), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`, `D25`
Security controls: `SEC-01`, `SEC-17`, `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`, `BND-18`
Protection Domain contracts: `PD-02`, `PD-12`, `PD-13`, `PD-14`
Code admission gates: `RCG-05`, `RCG-06`, `RCG-07`, `RCG-12`, `RCG-14`
Assurance state: `baseline-debt`

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

## Implementation location

Implemented via `os/src/xtask/src/main.rs`'s `qemu-x86_64` command (bounded-timeout QEMU launch, `isa-debug-exit`-based pass/fail detection), exercising `os/src/kernel/` boot code (`src/boot.rs`, `src/qemu_exit.rs`).

## Reports

- [`REPORT-2026-07-26-01`](../reports/REPORT-2026-07-26-01.md) — Pass (local, Tier 0/QEMU)
