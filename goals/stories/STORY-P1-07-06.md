# STORY-P1-07-06 — `fixture_measure` on Silicon: Batched Iteration and the First Hardware Report

Status: **Specified — not started; the Story that closes `LE-09`, and it needs `TEST-P1-07-06-A` Red first**
Feature: [`FEAT-P1-07`](../features/FEAT-P1-07.md)
Introduced in: [`session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md`](../../session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md) §5

## Description

The Story every other Story in this Feature exists to make possible: `fixture_measure` runs on a Raspberry Pi 5 and emits a `TINYOS-MEAS/2` envelope that the existing `xtask` parser reads.

Two deliverables, one of which is a change to how this project measures everything:

**Batched-iteration measurement.** Measure N iterations and divide, rather than one operation per sample. This is required for any coarse counter, and it **also closes `LE-24`**: `D07/pool_u64x64_alloc_free_round_trip` medians to 0 cycles on the Windows dev host precisely because a single operation costs less than the harness's own calibrated subtraction. One change, two loose ends — and the batched shape is host-independent, which is the property `LE-24`'s row asks for by name.

**The first hardware Report.** It states board revision, firmware version, clock policy and thermal state per the measurement protocol, so a third party can reproduce it. That metadata is not bureaucracy: "numbers arrive but are wrong" is the one risk in this Feature with **no local detection**, and reproducibility metadata is the only defence against it.

## Depends on

All of `-01` through `-05`. This Story is the collection point, not a new mechanism.

## Acceptance criteria

1. **A `TINYOS-MEAS/2` envelope parsed by the existing `xtask` parser with no changes to the parser.** That last clause is the point. It is the final test of the arch-neutrality claim `STORY-P1-01-03` made on the host and never got to check on silicon — and a parser change here would mean the seam was x86-shaped all along, which is a finding worth more than a clean run.
2. **Batched-iteration measurement, with the batch size recorded and justified per metric.** A batch large enough to beat quantisation and small enough not to hide the tail is a trade-off, and the Report states which way each metric was resolved rather than presenting one N as obvious.
3. **`LE-24` closes on the batched shape**, demonstrated by `D07` producing a non-zero, host-independent median.
4. **The Report states what the numbers are *not*.** Single core, no preemption, no address spaces, no `EL0`, no WCET enforcement — this is a hardware tier for the measured paths in this slice, not a hardware tier for `EPIC-P1`'s claims at large. The distinction between "the mechanism was demonstrated" and "the guardrail closed" that every Story in this Epic has drawn applies here too, and most sharply, because these are the first non-Tier-0 numbers and they will be quoted.
5. **Tier 0 remains green and unchanged.** No Tier 0 baseline is re-recorded, re-interpreted, or retired by this Story. This Feature adds a tier; it does not replace one.
6. **`LE-09` closes on this Report** — with `closed_in` populated in the register, and not one Story earlier.

## Named debt this Story leaves open

- **`LE-23` and `LE-18` are unaffected.** Both are about which *host* recorded the Tier 0 baseline; a hardware tier neither fixes nor worsens them.
- **No hardware CI.** Recorded decision (b); the gate stays Tier 0.
- **No comparative claim.** `G24`/`G25` comparisons run only after absolute release gates pass, on the same hardware and safety-equivalent configuration. Nothing here licenses a "faster than Linux" or "10× an RTOS" statement.
- **No worst-case bound — added 2026-07-28 by [`ADR 0005`](../../docs/adr/0005-arm64-real-time-tier-is-conditional-on-secure-world-qualification.md), registered as `LE-43`.** Criterion 4 already says the Report states what the numbers are *not*; `ADR 0005` adds one item to that list, and it is the item most likely to be quoted past. **These numbers establish the tier. They do not license a bound** — a worst-case latency bound, WCET claim or jitter envelope is quotable only from a platform holding a secure-world qualification record, and the Pi 5 holds none, because secure interrupt groups routed to `EL3` preempt NS-EL1 irrespective of `PSTATE.I` and nobody has yet looked at how this board's firmware configured that. **This is deliberately recorded as named debt rather than as a seventh acceptance criterion**: adding a clause here would extend `TEST-P1-07-06-A`, and that document is not amendable from this direction — the Red comes first, from the session that starts this Story. What that session must decide is whether criterion 4 absorbs the sentence or the qualification record becomes its own Story under `FEAT-P1-07` §6.

## Tests

[`TEST-P1-07-06-A`](../tests/TEST-P1-07-06-A.md) — written before implementation, per the TDD mandate.
