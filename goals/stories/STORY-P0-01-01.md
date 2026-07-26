# STORY-P0-01-01 — Empty Kernel Crate Boots in QEMU x86_64 and Halts Cleanly

Status: **Planned**
Feature: [`FEAT-P0-01`](../features/FEAT-P0-01.md)
Introduced in: [`session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md`](../../session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md)

## Description

An `os/src/kernel/` crate with no scheduler, no memory pools, and no drivers — just a boot entry point — builds against the custom `os/targets/x86_64-tinyos.json` target, boots under QEMU's `q35` machine type, and halts cleanly with no panic and no unexpected output.

## Acceptance criteria

1. `cargo build -p kernel --target ../../targets/x86_64-tinyos.json -Z build-std` succeeds with no warnings under `clippy -D warnings`.
2. Booting the resulting image in QEMU x86_64 reaches a halt instruction within a bounded startup time, with no panic message on the serial/console output.
3. The crate carries `#![no_std]` and stays within the crate-size ceiling trivially at this stage (a few hundred lines at most).

## Tests

- [`TEST-P0-01-01-A`](../tests/TEST-P0-01-01-A.md) — QEMU boot-to-halt integration test.

## Goals verified

G-RT-7 (64-bit-only portability, x86_64 side), G-HW-4 (indirectly — this Story is the prerequisite for ACPI manifest work in `FEAT-P0-04`).
