# STORY-P1-01-01 — Reusable Cycle-Calibrated Measurement Harness

Status: **Functionally Verified (Tier 0 + Host), 2026-07-27** — assurance state `baseline-debt`; no `PERF-D04`/`D05`/`D07` guardrail closed, hardware-tier timing evidence outstanding (`LE-09`)
Feature: [`FEAT-P1-01`](../features/FEAT-P1-01.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)
Implemented in: [`session/hand-2026-07-27/02-story-p1-01-01-measurement-harness.md`](../../session/hand-2026-07-27/02-story-p1-01-01-measurement-harness.md)

## Description

Generalize the pool-bench fixture's one-off pattern into the kernel's standing measurement primitive: cycle-source-overhead calibration, fixed-capacity no-heap sample buffers, warmup/sample-count discipline, percentile extraction (p50/p99/p99.9/max, matching the performance catalogue's budget columns), and a versioned serial reporting format `xtask` parses into structured results. First measured targets: context switch (D04), ready-queue dispatch (D05), pool allocation (D07).

## Depends on

`EPIC-P0` complete; the serial reporting path the pool-bench fixture introduced.

## Acceptance criteria (final)

1. **A reusable, arch-neutral measurement API** — fixed-capacity, no allocation, calibrated overhead subtracted, documented perturbation bound, usable from any Tier 0 fixture, with the cycle source behind a trait so no architecture is named in the interface. **Met**: `hal::time::CycleSource`/`Timebase` (+ a shared Liskov conformance suite), `kernel::measure::{Samples, Calibration, Summary, Stopwatch, Report}`, `hal_x86_64::tsc::Tsc`.
2. **pool-bench refactored onto it** rather than left as a divergent sibling. **Met**: `fixture_pool_bench.rs`'s private `rdtsc`, calibration, percentile and prose-reporting code is deleted; it now reports the same versioned envelope through the shared harness (closes `LE-06`).
3. **`xtask` parses the serial measurement stream into per-metric percentile records; malformed streams are a harness error (exit 2), never silently dropped samples.** **Met**: `xtask::timing` with one host test per specified failure mode (unknown version, missing/repeated `BEGIN`/`END`, count mismatch, missing/unknown/duplicate key, non-numeric value, duplicate metric, non-monotonic percentiles, `n=0`, unknown unit, no metrics, truncation), plus the new `xtask measure` command that captures COM1 and parses it.
4. **D04/D05/D07 each produce stable Tier 0 percentile evidence across repeated runs, with run-to-run variance recorded.** **Met with an important negative result**: three consecutive runs, all parsed, five metrics; run-to-run p99 CV is 1.2%–2.0% for the two dispatch metrics but **39%–61%** for the small-operation metrics — evidence that Tier 0 tail percentiles are too noisy to gate on, which directly shapes `STORY-P1-01-02`'s gate design. See [`REPORT-2026-07-27-02`](../reports/REPORT-2026-07-27-02.md).

## Beyond the original scope (both driven by measuring on target for the first time)

- **A guest-side microsecond timebase.** `hal_x86_64::tsc::calibrate_cycles_per_us` measures the timestamp counter against a 10 ms PIT channel-2 one-shot, reporting 2302–2307 cycles/µs across the three runs, or no timebase at all when the result is implausible. This closes `REPORT-2026-07-27-01`'s first named next step (cycle-denominated evidence could not previously be scored against microsecond-denominated guardrails).
- **Two real bugs found and fixed.** `Scheduler::highest_priority_ready` raised `#UD` → triple fault on the real target binary because LLVM emits SSE for 16-byte moves and `CR4.OSFXSR` was never set — a production scheduler function that could not execute on target, invisible to host-only verification (fixed in `hal_x86_64::boot`, see [`ADR 0003`](../../docs/adr/0003-enable-sse-in-the-boot-path.md)); and a measurement fixture switching into a task enables interrupts (`Context::new` seeds `IF`) against an IDT this boot path never installs (fixed by retiring the legacy PIC in the fixture; the underlying seam is loose end `LE-11`).

## Tests

[`TEST-P1-01-01-A`](../tests/TEST-P1-01-01-A.md) — written before implementation, Red run recorded (48 failing harness tests), then Green.

## Reports

- [`REPORT-2026-07-27-02`](../reports/REPORT-2026-07-27-02.md) — Red-then-Green evidence, three-run Tier 0 raw captures, the timebase measurement, per-guardrail scoring against `catalogue.tsv` (nothing closes, with the dev-profile and TCG-emulation reasons stated), and the gate-design implications.

## Goals verified

G-RT-3 (measurement half), G-DX-7.

## Named debt this Story leaves open

`LE-09` (no hardware tier — every number here is Tier 0), `LE-11` (`IF` set with no IDT), `LE-12` (CI never lints target-only fixture code), `LE-13` (measurement runs the dev profile, not release), `LE-14` (`context::switch` saves no SSE/x87 state now that SSE is enabled).
