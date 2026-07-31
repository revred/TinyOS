# TEST-P1-01-01-A — Cycle-Calibrated Measurement Harness: Percentile Extraction, Bounded Sampling, and Fail-Closed Stream Parsing

Status: **Functionally Verified — Red recorded then Green, passing locally 2026-07-27** (`REPORT-2026-07-27-02`: 197 host tests green, three consecutive Tier 0 QEMU runs parsed with run-to-run variance recorded). No `PERF-D04`/`D05`/`D07` guardrail is closed and no hardware-tier evidence exists, so the assurance state is `baseline-debt`, not `verified`.
Story: [`STORY-P1-01-01`](../stories/STORY-P1-01-01.md)
Tier: Host unit tests (percentile/summary/report-emission logic and the `xtask`-side stream parser — no QEMU dependency) **plus** a Tier 0 QEMU measurement fixture that exercises the same API against real target-compiled code, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D04`, `D05`, `D07`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-15`, `BND-16`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-08`
Code admission gates: `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

Per the TDD mandate (`agent/CODING_STANDARDS.md#test-driven-development`, reaffirmed as binding by Handover 37's directive 2), every clause below is written before the harness code that satisfies it.

### 1. Cycle source behind an arch-neutral trait

**Given** a cycle source implementing `hal::time::CycleSource` (one method, `read_cycles`),
**when** the measurement API is used,
**then** nothing in the kernel-side API mentions `rdtsc`, `x86_64`, or any other architecture — the same code compiles and runs against a host test double and against `hal_x86_64::tsc::Tsc`, so the future ARM64/Pi 5 slice (`LE-09`) supplies a third implementor without the harness changing.

Because `CycleSource` will have two or more implementors, per the Liskov rule in `agent/CODING_STANDARDS.md#l--liskov-substitution` it ships a **shared conformance suite** (`hal::time::conformance`) that every implementor must pass identically:

- a conformance run over any implementor observes non-decreasing readings across a bounded number of samples, and
- observes strictly positive forward progress at least once across that run (a source stuck at a constant is a conformance failure, not a fast machine).

### 2. Bounded, non-allocating sample buffers with explicit drop accounting

**Given** a `Samples<N>` fixed-capacity buffer,
**when** more than `N` samples are recorded,
**then** the surplus is counted in an explicit `dropped` counter and reported downstream — never silently discarded, never grown (no allocation exists to grow it), and never allowed to overwrite already-recorded samples.

**And** a buffer with zero recorded samples yields no summary at all (`None`), rather than a zero-filled or panicking one.

### 3. Calibrated overhead, saturating subtraction

**Given** a calibration pass over a cycle source,
**then** the reported overhead is the minimum observed back-to-back read delta, and every recorded sample has that overhead subtracted **saturating at zero** — a measurement shorter than the calibrated overhead records 0, never a wrapped `u64`.

### 4. Percentile extraction matching the catalogue's budget columns

**Given** a set of recorded samples,
**when** summarized,
**then** the summary carries `n`, `dropped`, `min`, `p50`, `p99`, `p99.9`, and `max`, computed by integer nearest-rank (`rank = (n-1) * num / den`) with no floating point anywhere (`#![no_std]`, no `libm`), and:

- for the sequence `1..=1000`, `p50 = 500`, `p99 = 990`, `p99.9 = 999`, `min = 1`, `max = 1000`;
- for a single sample, every percentile equals that sample;
- summarizing is order-independent (a shuffled input yields the identical summary);
- `min <= p50 <= p99 <= p99.9 <= max` holds for every possible input.

### 5. Versioned, machine-parseable report format

**Given** a summary set,
**when** emitted to any `core::fmt::Write` sink (COM1 in a fixture, a `String` in a host test),
**then** the stream is a versioned envelope: exactly one `TOS64-MEAS/1 BEGIN` line carrying the tier, arch, cycle-source name, calibrated overhead and timebase; one `TOS64-MEAS/1 METRIC` line per metric with a fixed key set (`domain`, `metric`, `n`, `dropped`, `warmup`, `min`, `p50`, `p99`, `p99_9`, `max`, `unit`); and exactly one `TOS64-MEAS/1 END` line whose `metrics=` count equals the number of METRIC lines actually emitted.

### 6. `xtask`-side parsing fails closed (`BND-15`/`BND-16`/`BND-17`)

**Given** the `xtask` parser for that stream,
**then** a well-formed stream parses into per-metric percentile records, and **every** one of the following is a *harness error* (`xtask` exit code 2), never a silently dropped sample, a zero-valued record, or a pass:

- an unknown envelope version (`TOS64-MEAS/2 ...`);
- a missing `BEGIN`, a missing `END`, or more than one of either;
- an `END` whose `metrics=` count disagrees with the number of METRIC lines seen;
- a METRIC line with a missing key, an unknown key, a duplicated key, or a non-numeric value;
- two METRIC lines with the same `domain`/`metric` pair;
- a METRIC line whose percentiles are not monotonically ordered, or whose `n` is zero;
- a stream containing no METRIC lines at all;
- truncated output (an envelope that stops mid-stream — the shape a guest that crashed or a UART that stalled actually produces).

Interleaved non-envelope lines (other fixture chatter on the same UART) are ignored rather than treated as errors; only lines carrying the `TOS64-MEAS` sentinel are parsed, and a sentinel-bearing line that is malformed is always an error.

### 7. Tier 0 evidence for D04/D05/D07, with run-to-run variance

**Given** the Tier 0 measurement fixture built for `x86_64-tinyos` and booted under QEMU `q35`,
**when** run repeatedly (at least three consecutive runs),
**then** each run emits a parseable envelope covering context switch (`D04`), ready-queue dispatch (`D05`), and pool allocation (`D07`); `xtask` parses every run; and the run-to-run p99 coefficient of variation is computed and recorded per metric — the number itself is evidence to be reported, not a threshold this Story enforces (thresholds and the gate are `STORY-P1-01-02`'s charge).

### 8. The measured code is not perturbed beyond a documented bound (`FEAT-P1-01` containment contract)

The harness grants nothing and runs with the authority of the code it times. Its own cost is measured and reported (the calibrated per-sample overhead), sample buffers are fixed-capacity `.bss` storage, serial output is bounded by the metric count, and no measurement path allocates. `SEC-20` (exhaustion) is addressed by construction: the buffer cannot grow, and the drop counter makes over-supply visible instead of unbounded.

### 9. What this test explicitly does **not** establish

QEMU/TCG cycle counts are emulation, not silicon. Even with the timebase conversion this harness reports, no clause above closes any `PERF-D04`/`D05`/`D07` microsecond-denominated guardrail at hardware tier. Hardware-tier timing evidence remains named, dated, release-blocking debt until measured on the Raspberry Pi 5 (`LE-09`), per `EPIC-P1`'s hardware-tier rule.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal/src/time.rs`, `os/src/kernel/src/measure.rs`, and `os/src/xtask/src/timing.rs`, run via `cargo test --workspace`) plus a Tier 0 QEMU fixture (`cargo run -p xtask -- measure`, which boots the fixture with COM1 captured to a file and parses the captured stream).

## Implementation location

- `os/src/hal/src/time.rs` — `CycleSource`/`Timebase` traits and their shared conformance suite.
- `os/src/hal-x86_64/src/tsc.rs` — the x86_64 `Tsc` implementor plus PIT-based timebase calibration.
- `os/src/kernel/src/measure.rs` — `Samples`, `Calibration`, `Summary`, `Stopwatch`, and the `TOS64-MEAS/1` emitter.
- `os/src/kernel/src/fixture_measure.rs` — the Tier 0 D04/D05/D07 fixture.
- `os/src/kernel/src/fixture_pool_bench.rs` — refactored onto the harness (its private percentile/rdtsc/report code deleted), closing `LE-06`'s divergent-sibling half.
- `os/src/xtask/src/timing.rs` — the fail-closed stream parser and run-to-run variance computation.

## Reports

- [`REPORT-2026-07-27-02`](../reports/REPORT-2026-07-27-02.md) — Red run recorded, then Pass (host tests + Tier 0 QEMU, three consecutive runs with run-to-run p99 CV per metric). Hardware-tier timing evidence remains explicit release-blocking debt.
