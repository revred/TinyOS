# Verification Reports

Status: **No reports filed yet — Phase 0 implementation has not started.**

## Purpose

A Report is the record that a Test actually ran, not just that it was specified. Every entry in [`goals/tests/`](../tests/) that has been implemented and executed at least once gets a corresponding Report here, linked back from the Test's "Reports" field.

## When a Report gets filed

- The first time a new Test passes in CI or on real hardware.
- Any time a previously-passing Test fails (regressions get a Report too — a Report is not only good news).
- At minimum once per Roadmap phase's Tier 1/2 hardware validation pass, per the [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix).

## Naming convention

`REPORT-YYYY-MM-DD-NN.md`, sequential within the date, mirroring the [`session/hand-YYYY-MM-DD/`](../../session/) dated-folder convention but for test execution records rather than design handovers.

## Report format

Each Report states:

- **Test(s) covered** — one or more `TEST-*` IDs.
- **Result** — pass / fail, with failure detail if applicable.
- **Environment/tier** — Tier 0 (QEMU/Renode), Tier 1 (Jetson Orin Nano Super), or Tier 2 (x86_64 mini-PC), per the Target Hardware & Test Matrix.
- **Toolchain/commit** — the exact Rust toolchain version and commit hash the test ran against, for reproducibility.
- **Linked session** — the `session/hand-*` handover (if any) this Report's finding was discussed in.

## Status

Empty. The first Report filed against this project will be for `TEST-P0-01-01-A` (QEMU boot-to-halt), once `FEAT-P0-01` implementation begins.
