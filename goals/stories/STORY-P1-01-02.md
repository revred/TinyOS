# STORY-P1-01-02 — Committed Baselines & the `check-timing-regression` CI Gate

Status: **Functionally Verified (Tier 0 + Host), 2026-07-27** — assurance state `baseline-debt`; no `PERF-D04`/`D05`/`D07` guardrail closed, hardware-tier timing evidence still outstanding (`LE-09`)
Feature: [`FEAT-P1-01`](../features/FEAT-P1-01.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)
Implemented in: [`session/hand-2026-07-27/05-story-p1-01-02-timing-gate.md`](../../session/hand-2026-07-27/05-story-p1-01-02-timing-gate.md)

## Description

Turn `STORY-P1-01-01`'s measurements into a blocking gate: committed per-metric baseline files (with recorded environment/tier provenance), a `cargo run -p xtask -- check-timing-regression` command comparing fresh Tier 0 runs against baselines with a documented tolerance model, and CI wiring so a timing regression fails a PR exactly like a functional failure — `README.md` Phase 1's own words.

## Depends on

`STORY-P1-01-01`.

## Acceptance criteria (final)

1. **Baselines are committed, dated, and carry their environment/tier so a QEMU number can never silently masquerade as a hardware number.** **Met**: [`goals/performance/baselines/tier0-x86_64.tsv`](../performance/baselines/tier0-x86_64.tsv), five metrics over five release-profile runs, each row carrying `tier`, `arch`, `profile`, `cycle_source`, `runs` and `recorded_on`. A comparison across any provenance disagreement — a T1 run against a T0 baseline, a release run against a dev baseline — is refused outright rather than absorbed into a tolerance.
2. **`check-timing-regression` fails closed: missing baseline, malformed baseline, or unparseable measurement stream is a failure, not a skip.** **Met**: one host test per malformed shape (bad header, field count, non-numeric column, `min > p50`, `runs=0`, empty field, duplicate key, header-only file), and a **missing** baseline file is an explicit gate failure with a message saying so — "no baseline yet, pass" is how a gate becomes decoration.
3. **The gate has been demonstrated to fail on a deliberately-introduced regression, and that demonstration is recorded in the Story's Report.** **Met**: `--inject-regression` builds the `fixture-measure-regression` Cargo feature (never enabled in a real image, `fixture-broken-boot`'s precedent), which performs seven extra selections *inside* the D05 timed region. The gate exits 1 naming `D05/dispatch_select_highest_priority_ready` at 76 → 1,086 cycles while the other four metrics pass. Recorded in [`REPORT-2026-07-27-04`](../reports/REPORT-2026-07-27-04.md).

## Beyond the original scope

- **`LE-13` closed — measurement is release-profile.** `--profile=release` was added to the build path and the gate always uses it. A dev-profile baseline would have gated a binary nobody ships; the release numbers are materially different (D04 p50 226 vs the dev profile's 246, with far tighter tails).
- **`LE-09` piece 4 closed — the pass/fail bit travels over the UART.** A `TINYOS-RESULT/1 fixture=<name> ok=<true|false>` sentinel line, emitted by both measurement fixtures and parsed strictly host-side. On Tier 0 it is **cross-checked against the QEMU `isa-debug-exit` code and must agree**, so the mechanism the Raspberry Pi 5 will depend on entirely is already validated against an independent signal rather than trusted for the first time on the day it becomes load-bearing.

## The finding this Story produced

**The gate's own sensitivity is poor, by measurement rather than by choice.** Two tolerance constants were falsified before one survived: 20% (the "stable statistics" from `REPORT-2026-07-27-02` in fact moved up to +28% run-to-run at release profile), then 40% (falsified by the gate's own first run — D07 alloc/free `min` came back +39% on unchanged code). The committed constant is **60% relative with a 24-cycle floor**, which means **Tier 0 can only catch regressions of roughly 1.6x or worse**.

That is honest and it is also the point: this gate is a tripwire for an accidental O(n) in a selection loop or a lock added to an RT path, not a defense against a 10% creep. No choice of constant makes TCG tighter. It is the most concrete argument this Epic has produced for `LE-09`'s hardware tier — the numbers a gate can actually defend need a board.

## Tests

[`TEST-P1-01-02-A`](../tests/TEST-P1-01-02-A.md) — written before implementation, Red run recorded (28 failing tests), then Green.

## Reports

- [`REPORT-2026-07-27-04`](../reports/REPORT-2026-07-27-04.md) — Red-then-Green evidence, the tolerance derivation with the two falsified drafts, the committed baselines, and the gate demonstrated both passing and failing.

## Goals verified

G-RT-3 (regression-suite half), G-DX-7. Neither is *closed*: both await hardware-tier evidence.

## Named debt this Story leaves open

`LE-09` remains **open** — pieces 1, 2 and 5 (AArch64 boot + target spec, PL011 UART, the SD-card/serial run path) wait for `FEAT-P1-02`, and the item only leaves the register when a Pi 5 has produced a measurement. `LE-15` (generic-timer resolution) is an input to any future hardware-tier tolerance, which cannot be extrapolated from the Tier 0 constants committed here. New: **`LE-16`** — the Tier 0 gate's ~1.6x sensitivity floor, which no Tier 0 work can improve.
