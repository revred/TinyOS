# STORY-P1-01-03 — AArch64 Generic-Timer Cycle Source and Timebase (Host-Testable, No Board)

Status: **Functionally Verified (Host), 2026-07-27** — assurance state `baseline-debt`; no `PERF-D04`/`D05`/`D07` guardrail closed, no hardware-tier evidence, `LE-09` still open
Feature: [`FEAT-P1-01`](../features/FEAT-P1-01.md)
Introduced in: [`session/hand-2026-07-27/03-le-09-arm64-pi5-slice-proposal.md`](../../session/hand-2026-07-27/03-le-09-arm64-pi5-slice-proposal.md) — the carve-out of the user's 2026-07-27 **Option B with the carve-out** decision
Implemented in: [`session/hand-2026-07-27/04-story-p1-01-03-arm64-timebase.md`](../../session/hand-2026-07-27/04-story-p1-01-03-aarch64-timebase.md)

## Description

Piece 3 of the `LE-09` minimal Pi 5 slice, and the only piece that needs no board: an `hal::time::CycleSource` implementor reading `CNTVCT_EL0` and an `hal::time::Timebase` implementor deriving cycles-per-microsecond from `CNTFRQ_EL0` — one register read, rounded to nearest, with no PIT-style calibration, because the AArch64 generic timer reports its own frequency architecturally.

Its purpose is evidential rather than functional. `STORY-P1-01-01` asserted that the harness's cycle-source trait is a real architectural seam; a trait with one implementor cannot demonstrate that. This Story supplies the second implementor, runs the *shared* conformance suite against it, and drives the entire measurement path with it on the host — so the seam is either validated or exposed now, while it is still free to change.

## Depends on

`STORY-P1-01-01` (the harness, the traits and the shared conformance suite). Nothing else — deliberately not the boot path, the UART, or a target spec.

## Acceptance criteria

1. **A second `CycleSource` implementor that passes the shared conformance suite identically.** **Met**: `hal_arm64::timer::Cntvct<R>` passes `hal::time::conformance::check`, and fails it in exactly the documented ways when driven by a stuck or backwards-going counter — it launders nothing.
2. **A `Timebase` derived from `CNTFRQ_EL0`, with honest absence rather than a guess.** **Met**: `GenericTimerTimebase` reports rounded cycles-per-microsecond, and `None` for zero (unprogrammed firmware — a real ARM condition, and exactly the input a truncating divide would turn into a silent 0), for sub-MHz, and for absurdly high frequencies. The plausibility floor is deliberately 1 MHz rather than the x86_64 backend's 10 cycles/µs: a 1 MHz generic timer is ordinary hardware where a 1 cycle/µs TSC would be broken, so copying the other backend's bounds would have rejected valid boards.
3. **The harness accepts it with no change to the harness.** **Met, and this is the Story's real result**: a host test drives `Calibration`, `Samples`, `Stopwatch` and `Report` with the AArch64 source and produces a well-formed `TINYOS-MEAS/1` envelope (`arch=aarch64 cycle_source=cntvct_el0`) that the existing, unmodified `xtask` parser reads. No file under `os/src/kernel/`, `os/src/hal/` or `os/src/xtask/` was modified by this Story — the seam held.
4. **Everything except the two `mrs` instructions is host-tested.** **Met**: the register reads sit behind two one-method traits (`VirtualCounter`, `CounterFrequency` — segregated because a cycle source must not depend on a frequency register it never reads), the concrete `mrs` implementor is the only `cfg(target_arch = "aarch64")` item and the only `unsafe`, and every other item compiles, lints and tests on the x86_64 dev machine.

## Tests

[`TEST-P1-01-03-A`](../tests/TEST-P1-01-03-A.md) — written before implementation, Red run recorded, then Green.

## Reports

- [`REPORT-2026-07-27-03`](../reports/REPORT-2026-07-27-03.md) — Red-then-Green evidence, the unmodified-consumer diff, and the counter-resolution finding below.

## The finding this Story produced

The generic timer is a **fixed-frequency system counter, not a CPU cycle counter**. On a Pi 5 it runs at 54 MHz — one tick is ~18.5 ns, against an x86_64 TSC tick of ~0.43 ns at the 2.3 GHz `REPORT-2026-07-27-02` measured under QEMU. A ~100 ns context switch is therefore about **5 ticks** on the board this slice targets, where Tier 0 resolves it in hundreds of counts.

Two consequences, both recorded rather than resolved here:

- The harness must treat a **calibrated overhead of zero** as an ordinary reading, not an error — it does, and a clause now tests that, but nothing had previously forced the question.
- Hardware p50/p99 for the smallest operations will be **quantization-limited**, not noise-limited. That is a different statistical problem from the 39–61% Tier 0 tail variance `REPORT-2026-07-27-02` found, and `STORY-P1-01-02`'s gate design needs to know it before hardware arrives rather than after. The higher-resolution alternative — the PMU cycle counter `PMCCNTR_EL0` — is not architecturally guaranteed to be accessible and must be explicitly enabled at EL1, so it is a considered follow-up, not a substitution: loose end **`LE-15`**.

## Goals verified

G-RT-3 (measurement half, arch-neutrality), G-DX-7. Neither is *closed* by this Story.

## Named debt this Story leaves open

`LE-09` remains **open** — pieces 1, 2 and 5 (AArch64 boot + target spec, the PL011 UART, the host-side SD-card/serial run path) wait for `FEAT-P1-02` per the recorded decision, and the item only leaves the register when a Pi 5 has actually produced a measurement. `LE-15` (generic-timer resolution vs. `PMCCNTR_EL0`) is new and named above. The two `mrs` reads are compiled but never executed by any test in this repository, which is stated plainly in the Report rather than papered over.
