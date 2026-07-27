# STORY-P1-01-01 — Reusable Cycle-Calibrated Measurement Harness

Status: **Specified, not yet started**
Feature: [`FEAT-P1-01`](../features/FEAT-P1-01.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

Generalize the pool-bench fixture's one-off pattern into the kernel's standing measurement primitive: `rdtsc`-overhead calibration, fixed-capacity no-heap sample buffers, warmup/sample-count discipline, percentile extraction (p50/p99/p99.9/max, matching the performance catalogue's budget columns), and a versioned serial reporting format `xtask` parses into structured results. First measured targets: context switch (D04), ready-queue dispatch (D05), pool allocation (D07).

## Depends on

`EPIC-P0` complete; the serial reporting path the pool-bench fixture introduced.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. A reusable measurement API (fixed-capacity, no allocation, calibrated overhead subtracted, documented perturbation bound) usable from any Tier 0 fixture — pool-bench's fixture is refactored onto it rather than left as a divergent sibling.
2. `xtask` parses the serial measurement stream into per-metric percentile records; malformed streams are a harness error (exit 2), never silently dropped samples.
3. D04/D05/D07 each produce stable Tier 0 percentile evidence across repeated runs, with run-to-run variance recorded.

## Tests

Not yet written — deferred until this Story starts. Requires host tests for percentile/parsing logic and a Tier 0 measurement fixture.

## Goals verified

G-RT-3 (measurement half), G-DX-7.
