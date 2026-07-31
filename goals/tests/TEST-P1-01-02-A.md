# TEST-P1-01-02-A — Committed Timing Baselines and a Regression Gate That Fails Closed and Has Been Seen to Fail

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P1-01-02`](../stories/STORY-P1-01-02.md)
Tier: Host unit tests (baseline parsing, median/tolerance comparison, result-line parsing — no QEMU dependency) **plus** Tier 0 QEMU runs of the real measurement fixture, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D04`, `D05`, `D07`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-15`, `BND-16`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-08`
Code admission gates: `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

`STORY-P1-01-01` produced measurements and one negative result that governs this Story's whole design: Tier 0 run-to-run **p99 coefficient of variation is 39–61%** on small operations. A gate thresholding a tail on a single run would fail green code and pass real regressions, and would then be disabled by whoever it woke at 2am — which is how a gate stops gating. So the clauses below gate the *stable* statistics over *repeated* runs, report the tails without gating them, and say so in the gate's own output.

Two inputs from later work are folded in here rather than deferred, both because a gate that lacks them cannot be trusted:

- **`LE-13`** — measurement has run **dev-profile** binaries, so every baseline would bake in missing optimization. Baselines here are **release-profile**.
- **`LE-09` piece 4** — the pass/fail bit currently travels only as a QEMU `isa-debug-exit` code, and a gate that can only read a QEMU exit code can never gate a board. It has to travel over the UART too.

## Specification

### 1. A pass/fail bit that survives having no QEMU under it

**Given** a measurement fixture,
**then** it emits exactly one sentinel result line on the same UART as the envelope — `TOS64-RESULT/1 fixture=<name> ok=<true|false>` — carrying its own self-consistency verdict.

**And** the host side parses that line strictly: no result line, more than one, an unknown key, a missing key, or an `ok=` value that is not exactly `true` or `false` is a **harness error**, never a pass. A run whose evidence does not say whether it passed is not evidence.

**And** on Tier 0, where both signals exist, the UART verdict and the `isa-debug-exit` code must **agree**; disagreement is a harness error naming both. This is the cross-check that establishes the UART bit is trustworthy *before* a board arrives where it is the only bit there is.

### 2. Baselines are committed data with their provenance attached

**Given** a committed baseline file,
**then** each row carries `domain`, `metric`, `tier`, `arch`, `profile`, `cycle_source`, `runs`, `min_cycles`, `p50_cycles` and `recorded_on` — so a number can never be read without knowing what produced it, and a Tier 0 figure can never silently masquerade as a hardware figure.

**And** the gate **refuses to compare** a measurement whose `tier`, `arch`, `profile` or `cycle_source` disagrees with the baseline's. Comparing a release run against a dev baseline, or a T1 run against a T0 baseline, is a category error and is reported as one rather than absorbed into a tolerance.

**And** baselines are recorded from **release-profile** binaries (`LE-13`). A dev-profile baseline is not merely noisier — it is measuring different code.

### 3. Parsing a baseline fails closed on every malformed shape

**Given** the baseline parser,
**then** each of the following is an error, never a skipped row and never a default value: a wrong or missing header; a row with too few or too many fields; a non-numeric `min_cycles`/`p50_cycles`/`runs`; `min > p50`; `runs = 0`; a duplicated `domain`/`metric` key; an empty file; and a file that exists but contains no rows.

**And** a **missing** baseline file is a gate failure, not a skip — the single most important clause in this document, because "no baseline yet, pass" is how a regression gate silently becomes decoration.

### 4. The gate compares medians of repeated runs, on the stable statistics only

**Given** `N >= 3` parsed runs of the same metric set,
**then** the compared value for each metric is the **median across runs** of `min` and of `p50` — never a single run, and never `p99`/`p99.9`/`max`, whose Tier 0 variance `REPORT-2026-07-27-02` measured at 39–61%.

**And** the tails are still **reported** in the gate's output, explicitly labelled as reported-not-gated, so a reader never mistakes an ungated tail for a passing one.

**And** an even number of runs takes the lower-middle element rather than averaging: the compared figure stays an actually-observed cycle count, not an interpolation.

### 5. The tolerance model is stated, bounded, and derived from measured noise

**Given** a baseline value `b` and an observed median `o`,
**then** the metric fails when `o > b + max(absolute_floor, b * relative_percent / 100)`, with both constants named, committed and justified by the run-to-run variation actually measured on this harness — not chosen to make today's numbers pass.

**And** an **improvement is never a failure**, but an improvement beyond the same tolerance is **reported** as a baseline-drift notice, because a metric that got 40% faster usually means the workload stopped happening, not that the code got faster.

**And** the tolerance is applied per metric, and one failing metric fails the gate — no aggregate score, no "3 of 5 passed".

### 6. The gate fails closed on everything upstream of the comparison

**Given** the gate command,
**then** each of these is an exit-2 harness error, distinct from an exit-1 regression: a fixture that would not build; a run that produced no envelope; a malformed or truncated envelope (`BND-15`/`-16`/`-17`, already specified by `TEST-P1-01-01-A`); runs that measure different metric sets; a metric present in the baseline but absent from the runs; a metric present in the runs but absent from the baseline; and the result-line failures of clause 1.

**And** the three outcomes are distinguishable by exit code alone: **0** pass, **1** timing regression, **2** harness error — the same discipline `TEST-P0-01-03-A` established for boot.

### 7. The gate has been *seen* to fail (`fixture-broken-boot` discipline)

**Given** a deliberately-introduced regression in real measured code — not a doctored baseline file —
**when** the gate runs,
**then** it exits 1, names the regressed metric, and prints baseline, observed and tolerance.

The regression is introduced the way this repository already introduces provable-failure fixtures: a Cargo feature that is never enabled in a real image (`fixture-broken-boot`'s precedent), so the demonstration is reproducible by anyone on demand rather than a screenshot of a one-time edit. Its output is recorded in this Story's Report.

### 8. CI runs the gate on every PR

**Given** the CI workflow,
**then** the gate runs in the QEMU job and its failure fails the build, exactly as a functional test failure does — `README.md` Phase 1's own words. `LE-07`'s lesson applies: a gate nobody observes is not a gate.

### 9. What this test explicitly does **not** establish

- **No hardware tier.** Every baseline here is Tier 0 QEMU/TCG. The gate proves *regression detection* works; it does not close any `PERF-D04`/`D05`/`D07` microsecond guardrail, and `LE-09` stays open until a Pi 5 produces a measurement.
- **No claim that the baselines are budgets.** A committed baseline records what this code does today under emulation. It is not a WCET, not a contract with a caller, and not evidence that any catalogue budget is met.
- **No tail gating.** p99/p99.9/max are reported and deliberately ungated at Tier 0. Whether they can ever be gated is a hardware-tier question, and `LE-15` says the hardware tier's noise has a different shape (quantization, not jitter) so today's tolerances will not transfer.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/xtask/src/timing.rs`) for the result-line parser, baseline parser, median selection, tolerance arithmetic and every fail-closed case; plus Tier 0 QEMU runs of `fixture-measure` through `cargo run -p xtask -- check-timing-regression`, including the deliberate-regression demonstration.

## Implementation location

- `os/src/kernel/src/measure.rs` — the `TOS64-RESULT/1` emitter.
- `os/src/kernel/src/fixture_measure.rs`, `os/src/kernel/src/fixture_pool_bench.rs` — emit it.
- `os/src/xtask/src/timing.rs` — result-line parser, baseline model and parser, median/tolerance comparison.
- `os/src/xtask/src/main.rs` — `check-timing-regression`, and `--profile=release` for `measure`.
- `goals/performance/baselines/` — the committed baseline file.
- `.github/workflows/ci.yml` — the gate step.

## Reports

- [`REPORT-2026-07-27-04`](../reports/REPORT-2026-07-27-04.md) — Red run recorded then Green, the committed baselines with their run-to-run spread, the tolerance derivation, and the gate demonstrated failing on a real injected regression.
