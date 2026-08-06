# STORY-P1-12-01 — A Task Has a Class, and the Pool Has a Floor It Cannot Cross

Status: **Specified, not yet started — and deliberately unallocated.** This Story exists because [`FEAT-P1-12`](../features/FEAT-P1-12.md) cannot be filed without one; it is not a queue entry a session should pick up opportunistically. The Feature is the one Feature-sized item among `STORY-P1-05-01`'s four prerequisites, and the standing do-not-start rule names it
Feature: [`FEAT-P1-12`](../features/FEAT-P1-12.md)
Introduced in: [`session/hand-2026-08-06/09C-the-guards-are-in-and-the-canvas-stops-painting-on-faith.md`](../../session/hand-2026-08-06/09C-the-guards-are-in-and-the-canvas-stops-painting-on-faith.md)

## Description

Two contracts are asserted in this repository against a mechanism that does not exist.
`BND-15` says one flooding domain cannot consume another class's budget or any RT
reserve; `RCG-08` rests on the same floor. The audit behind that is
`STORY-P1-05-01`'s, dated 2026-08-06: **`Tcb` carries no containment class**, its fields
being `base_priority`, `inherited_priority`, `wcet_budget`, `overrun_policy`, `entry`,
`ticks_consumed` and `state`; **the pool is one flat capacity** with no class tag and no
reservation floor; and a repository-wide search for a scheduling or allocation reserve
finds none.

This Story builds the two things that make those contracts checkable: **a class on the
task, and a floor on the pool.** Nothing above them — no flood, no campaign, no property
tests. Those are `FEAT-P1-05`'s and they are what *exercises* this; keeping them apart is
what stops a clean run being mistaken for a proof (`ADR 0005`).

## Why this Story is written and then left alone

`FEAT-P1-12`'s own scope section argues the split. The part that belongs here is the
consequence for whoever reads this file next: **a Story marked `Specified` is not an
invitation.** This one is the head of a Feature-sized job, and starting it in a session
that has room for a Story produces a half-built reserve — a class field with no allocator
honouring it, or a floor with no attributable refusal — which is worse than nothing
because the contracts would then *appear* to have a mechanism.

## Depends on

`STORY-P1-04-01`/`-02` — a reserve that survives is only meaningful where something can
be preempted away from it. Nothing in `FEAT-P1-05`: this precedes that Feature's
criterion 1 rather than following it.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. **A task's class is a declared field set at creation, and no path mutates it
   afterwards** — asserted over the whole crate, because a class that can change at run
   time is a budget that can be bought, which is `BND-16` exactly.
2. **The pool enforces a per-class floor**, and an allocation that would cross it fails
   with an `Err` that is bounded in cost and names the offending class. A floor enforced
   by convention rather than by the allocator is not a floor.
3. **The refusal is attributable.** It reaches the spoor stream identifying *which class*
   was denied — which is where this Story meets `STORY-P1-05-01`'s item 2 and `LE-82`:
   all three are "the spoor vocabulary cannot say the thing the contract requires", and
   the encoding change should be made once for all of them.
4. **`BND-15` and `RCG-08` have a mechanism behind them**, selected by
   `FEAT-P1-12`'s contract row rather than inherited from `FEAT-P1-05`'s.

## What this Story cannot establish, stated before it starts

**That the floor holds under attack.** That needs the flood, which is
`STORY-P1-05-01`'s, and a reserve demonstrated only by construction is the clean run
`ADR 0005` warns about. This Story's exit is the mechanism; the proof is the other
Feature's, and the two are separate documents so neither is quoted as the other's
evidence.

**No `G04` row comes out of it.** Bound-class gates are refused at `T0`, on `x86_64`, and
from any platform absent from `qualified-platforms.tsv`, which holds zero
(`ADR 0005`, `LE-09`, `LE-94`).

## The measurement that should precede this Story

`PERF-D05-G19` asks for `loaded_degradation` — p99 ≤ 5%, max ≤ 10%, hard deadline misses
= 0 — and that is a **measurement under load, not a guarantee**. It can be taken today
with `STORY-P1-05-01`'s items 3 and 4 and no reserve in the tree. **If it fails, the
failing row is what turns this Story from architecture prose into a number**, which is
the order every other claim in this repository is held to.

## Tests

Not yet written — deferred until this Story starts.

## Goals verified

`G-SEC-12` (the containment half); `G-SEC-14` (attributable denial, jointly with
`STORY-P1-05-01` item 2 and `LE-82`).
