# TEST-P1-07-10-A — A Masked Region Must Close, and Must Close Back to What It Found

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P1-07-10`](../stories/STORY-P1-07-10.md)
Tier: Host unit tests (`hal::interrupts` — ordering against a recording gate, `DAIF` decode) **plus** a Tier 1 hardware run on a Raspberry Pi 5 whose measure boot shows a tick surviving the fixture, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D02`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: `PERF-D02-G01`, `PERF-D03-G01`, `PERF-D03-G05` — interrupt entry latency, tick accounting latency and its jitter envelope. This Story is what makes them observable *after* a measurement run rather than only before one.

## What this test is for

A measurement region must run interrupt-free. The cheap way to get that is to mask on the way in and never think about it again, and that is precisely the defect `LE-71` records: the tick died at the fixture and `STORY-P1-07-04` criterion 1 became unreachable on every board, permanently, while every host test stayed green.

The lesson the test suite has to learn is that **the ordering was the defect**. Not the mask, not the register write, not the arithmetic — the question of *when* the second poke happens, which no test owned. So the ordering is extracted into arch-neutral code and pinned there, and the architecture keeps only the instructions.

This is `LE-66` applied honestly. That finding says a seam with zero tests is not thin, it is untested. The `MSR` at the bottom cannot be executed by a host, so the response is not to shrug at it — it is to move every decision *out* of it until what remains decides nothing.

## Clauses

**Clause 1 — the scope always closes.** A recording gate observes `mask`, then the body, then `restore`, in that order. Red first: run against the pre-Story behaviour and the log reads `[Masked, BodyRan]` with no `Restored` — `LE-71` reproduced as a unit test rather than as a photograph of a screen.

**Clause 2 — an early return still closes it.** `fixture_measure`'s emit path returns early on failure. A restore written as a trailing statement is skipped exactly when a run fails, which is the worst moment to leave interrupts off; the closure form makes the early return leave the *body*, not the region.

**Clause 3 — restore is handed what mask reported.** A region entered already masked is left masked. Unconditional unmask is a different behaviour and passes clause 1 while failing this one, which is why both exist: on a boot whose tick was refused, unmasking opens the door with no timer behind it.

**Clause 4 — only `DAIF.I` decides.** `D`, `A` and `F` are debug, SError and FIQ. A region that masked one of those says nothing about IRQ acceptance. The decode is tested against each of them set alone and against `I` set alongside them.

**Clause 5 — the body's value reaches the caller unchanged.** The wrapper is transparent; a fixture verdict must not be altered by the thing that masked interrupts around it.

**Clause 6 — on silicon, the tick survives the fixture.** A measure boot shows `TOS64-TICK/1` with `COUNT` past 2 and `RMIN`/`RMAX` populated near 1000. This is the clause that cannot be faked on a host, and it is the one that hands `STORY-P1-07-04` criterion 1 its evidence.

**Clause 7 — the measurement stays clean.** The fixture's samples are still taken masked. Distributions stay comparable to `BOARD VERDICT 5` and `6`; a tick-shaped outlier appearing *inside* the fixture refutes the Story rather than confirming it. Stated as a refutation condition on purpose — a criterion that can only be satisfied is not a criterion.

## What this test does not cover

- **Panic safety.** `panic = "abort"` means a panicking body aborts rather than escaping the region. No drop guard exists; none is claimed. This becomes a hole the day unwinding is enabled.
- **The instructions themselves.** No host executes `MSR daifset`. The claim is bounded to ordering and decode.
- **More than one core.** `PSTATE` is per-core and there is one core.
