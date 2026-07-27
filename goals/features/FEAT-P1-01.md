# FEAT-P1-01 — Timing Measurement Harness & CI Timing-Regression Gate

Status: **Specified — no Story started**
Epic: [`EPIC-P1`](../epics/EPIC-P1.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

The ruler every other `EPIC-P1` Feature is measured with (Goals **G-RT-3**, **G-DX-7**): a reusable, cycle-calibrated Tier 0 timing-measurement harness (generalizing the pattern `EPIC-P0`'s pool-bench fixture prototyped — `rdtsc` overhead calibration, fixed-capacity sample buffers, percentile reporting over the COM1 serial protocol, harness-side parsing in `xtask`), plus committed per-metric baselines and a **CI gate that fails on timing regression the way `check-assurance-spine` already fails on structural drift**. First measured targets: the context switch (D04), ready-queue dispatch (D05), and pool allocation (D07) — the performance-catalogue domains whose code already exists and is stable.

QEMU cycle counts calibrate the harness and the regression *mechanism*; they are not hardware WCET evidence, and every artifact this Feature produces must say so (see `EPIC-P1`'s hardware-tier debt note).

## Crate(s) involved

`os/src/kernel/` (measurement fixtures), `os/src/hal-x86_64/` (serial/cycle primitives), `os/src/xtask/` (report parsing, baseline comparison, the new `check-timing-regression` command + CI wiring)

## Depends on

`EPIC-P0` complete. No intra-Epic dependency — this Feature is deliberately first.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P1-01-01`](../stories/STORY-P1-01-01.md) | Reusable cycle-calibrated measurement harness (kernel side + xtask parser) | Specified |
| [`STORY-P1-01-02`](../stories/STORY-P1-01-02.md) | Committed baselines + `check-timing-regression` CI gate, proven able to fail | Specified |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0/C1** · subject **C1/C2** · boundary tests **BND-15, -16, -17**.

The harness measures; it grants nothing. Measurement fixtures run with the same authority as the code they time (never more), sample buffers are fixed-capacity, serial reporting is bounded, and baseline files are data the gate parses defensively — a malformed baseline fails the gate closed, it does not skip the check. Timing instrumentation must never perturb the RT paths it measures beyond its own documented, calibrated overhead.

## Exit criteria

Both Stories **Verified**: the harness produces stable percentile evidence for D04/D05/D07 under Tier 0, baselines are committed, the CI gate runs on every PR, and a deliberately-introduced regression has been demonstrated to fail it.
