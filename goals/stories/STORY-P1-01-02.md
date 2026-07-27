# STORY-P1-01-02 — Committed Baselines & the `check-timing-regression` CI Gate

Status: **Specified, not yet started**
Feature: [`FEAT-P1-01`](../features/FEAT-P1-01.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

Turn `STORY-P1-01-01`'s measurements into a blocking gate: committed per-metric baseline files (with recorded environment/tier provenance), a new `cargo run -p xtask -- check-timing-regression` command comparing fresh Tier 0 runs against baselines with a documented tolerance model (absolute + relative headroom, chosen to survive QEMU jitter without absorbing real regressions), and CI wiring so a timing regression fails a PR exactly like a functional failure — `README.md` Phase 1's own words.

## Depends on

`STORY-P1-01-01`.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. Baselines are committed, dated, and carry their environment/tier so a QEMU number can never silently masquerade as a hardware number.
2. `check-timing-regression` fails closed: missing baseline, malformed baseline, or unparseable measurement stream is a failure, not a skip.
3. The gate has been demonstrated to fail on a deliberately-introduced regression (the `fixture-broken-boot` "prove the gate can fail" discipline), and that demonstration is recorded in the Story's Report.

## Tests

Not yet written — deferred until this Story starts. Requires host tests for comparison/tolerance logic and a CI-wired Tier 0 run.

## Goals verified

G-RT-3 (regression-suite half), G-DX-7.
