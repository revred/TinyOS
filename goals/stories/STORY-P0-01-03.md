# STORY-P0-01-03 — `xtask qemu-x86_64` Builds and Launches the Kernel Under QEMU

Status: **Verified** (locally; CI run pending)
Feature: [`FEAT-P0-01`](../features/FEAT-P0-01.md)
Introduced in: [`session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md`](../../session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md)

## Description

`os/src/xtask/` — an ordinary `std` Rust binary per [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md#why-xtask-not-shell-scripts) — implements a `qemu-x86_64` subcommand that builds `kernel` against the custom target spec and launches it under QEMU, so no contributor needs to remember the `cargo build --target ... -Z build-std` incantation by hand, and CI invokes the exact same command a local developer would.

## Acceptance criteria

1. `cargo run -p xtask -- qemu-x86_64` succeeds from a clean checkout with only the pinned toolchain and QEMU installed.
2. The command exits with a distinguishable, correct exit code on boot success vs. boot failure/panic, so CI can use it as a pass/fail gate directly.
3. `xtask` itself carries `#![forbid(unsafe_code)]` per its `std`/host-tool classification in the crate map.

## Tests

- [`TEST-P0-01-03-A`](../tests/TEST-P0-01-03-A.md) — `xtask qemu-x86_64` command smoke test.

## Goals verified

G-DX-3 (remote-first, secure development loop — this is the local half of that loop; the remote-deploy half is `EPIC-P1_5`).
