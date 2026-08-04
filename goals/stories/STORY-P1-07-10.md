# STORY-P1-07-10 — Interrupt Masking Is a Scope, Not a Switch: the Defect That Made `STORY-P1-07-04` Criterion 1 Unreachable

Status: **In progress — host half Green 2026-08-04 (7 new tests in `hal::interrupts`, Red first against the defect's own behaviour: the recording gate logged `[Masked, BodyRan]` with no `Restored`, which is `LE-71` exactly). **Criteria 4 and 5 Green on silicon** — `BOARD VERDICT 7` (measure boot 2026-08-04, kernel `619f40b8c076`) read `TOS64-TICK/1 count=1816 rmin=999 rmax=1000`, the tick surviving the fixture and handing [`STORY-P1-07-04`](STORY-P1-07-04.md)'s ratio requirement its evidence. Criterion 5 confirmed by a test written to refute it: Verdict 6's lone `max=3519` outlier is **absent** at `max=376`, on the boot where the tick fires 100×/s for the whole park loop, and the distributions came back tighter rather than looser. `LE-71` closed. Not Verified.**
Feature: [`FEAT-P1-07`](../features/FEAT-P1-07.md)
Introduced in: `session/hand-2026-08-04/03A` delivery session, from `BOARD VERDICT 6`

## Description

`STORY-P1-07-06` masks interrupts around the measurement run, correctly: Tier 0 measures interrupt-free, and a board that does otherwise folds tick handlers into its samples without saying so. The defect is not the masking. It is that the mask had **no scope**.

`fixture_measure` called `mask_interrupts()` and nothing ever unmasked. `unmask_interrupts()` existed in `hal_arm64::boot` with no callers anywhere in the tree — written for this and never wired in. The comment above the call stated the reasoning plainly:

> *the park loop unmasks nothing — masking is one-way here; the park loop's channels never depended on it being undone, and the tick line having accumulated its pre-fixture intervals is itself evidence.*

The premise is that enough intervals accumulate before the fixture starts. **On silicon it is false, and not marginally.** The only window in which the tick can fire is between the `daifclr` that follows GIC bring-up and the fixture's mask — a window containing the 64-sample conformance run and the PMU probe. `BOARD VERDICT 6` measured what that window admits: `TOS64-TICK/1 COUNT=1`.

One tick is one timestamp. One timestamp is **zero** intervals. `tick::ratio_bounds_per_mille` returns `None` below two intervals by deliberate design — *"a ratio over silence would be a confident claim about nothing"* — so `RMIN=NONE RMAX=NONE` is the correct output of a correct function fed a truthful input.

Every layer behaved correctly and the criterion was still unreachable. `STORY-P1-07-04` criterion 1 asks for a tick verified by ratio between consecutive intervals; with the fixture on the boot image, that ratio could not form on this board or any other, and no amount of re-booting would have produced it. The defect was invisible to every host test because no host test owned the *ordering*.

It also contradicts the recorded design intent: the `03A` handover states *"the park loop's `wfe` waits wake 100× per second"*. That is the behaviour this Story restores.

## What changes

The region becomes a scope, and the ordering rule moves to where a host test can hold it:

- `hal::interrupts` (new, arch-neutral): `InterruptState`, the `InterruptGate` trait, and `with_interrupts_masked(gate, body)` — mask, run, restore-what-was-there. The restore is unconditional with respect to the body's control flow, which matters because `fixture_measure`'s emit path has a `return false` partway through; a restore written as a trailing statement would have been skipped exactly when a run failed.
- `hal_arm64::boot::PstateInterrupts`: the two register pokes and no policy — one `MRS` to learn what was there, one `MSR` to change it.
- `fixture_measure_arm64` wraps its run in that scope. Its body no longer masks, unmasks, or assumes which state it was entered with.

**The state is saved and restored, never unconditionally unmasked.** A boot whose tick was *refused* enters the fixture with interrupts already masked, and must leave that way — unmasking there would open the door with no timer behind it.

## Depends on

`STORY-P1-07-04` (there is no tick to preserve without it) and `STORY-P1-07-06` (the fixture that masks).

## Acceptance criteria

1. **The scope always closes**, whatever the body does — including an early return. Held by host test against a recording gate, Red first.
2. **A region entered already masked is left masked.** Restoring by unconditional unmask is a different and wrong behaviour, and the test distinguishes them.
3. **Only `DAIF.I` decides.** `D`, `A` and `F` are other exception classes; reading any of them as `I` restores the wrong thing.
4. **On silicon, the tick survives the fixture**: a measure boot shows `TOS64-TICK/1` with `COUNT` climbing past 2 and `RMIN`/`RMAX` populated, which is `STORY-P1-07-04` criterion 1 reached through this Story.
5. **The measurement is not contaminated.** The fixture's own samples are still taken with interrupts masked; the metric distributions stay comparable to `BOARD VERDICT 5`/`6`, and a tick-shaped outlier appearing *inside* the fixture would refute this Story rather than confirm it.

## Named debt this Story leaves open

- **No panic-safety guard.** The crate is built `panic = "abort"`, so a panicking body aborts rather than escaping past the restore. No drop guard exists and none is claimed; if unwinding is ever enabled this becomes a real hole.
- **The `MSR`/`MRS` half stays untested**, as `LE-66` predicts for any seam that ends in an instruction. The mitigation is that the untested half was made as small as it honestly can be — it decides nothing.
- **Single core.** `PSTATE` is per-core; nothing here reasons about masking on a core that does not exist yet.

## Tests

[`TEST-P1-07-10-A`](../tests/TEST-P1-07-10-A.md) — written before implementation, per the TDD mandate.
