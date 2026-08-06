# FEAT-P1-12 — The RT Reserve and the Per-Class Budget: the Floor `BND-15` Is Asserted Against

Status: **Specified — no Story started, and deliberately unallocated.** Split out of
[`STORY-P1-05-01`](../stories/STORY-P1-05-01.md) on 2026-08-06 at that Story's own
recommendation. It is the one part of the hostile-load work that is genuinely
Feature-sized, and separating it is what lets `FEAT-P1-05` proceed on the other three
without being blocked by association
Epic: [`EPIC-P1`](../epics/EPIC-P1.md) — Determinism Proof
Introduced in: [`session/hand-2026-08-06/09C-the-guards-are-in-and-the-canvas-stops-painting-on-faith.md`](../../session/hand-2026-08-06/09C-the-guards-are-in-and-the-canvas-stops-painting-on-faith.md)

## Description

**Two contracts in this repository are asserted against a mechanism that does not
exist.** `BND-15` says one flooding domain cannot consume another class's budget or any
RT reserve. `RCG-08` rests on the same floor. Neither has anything behind it, and the
audit that established that is dated and specific — `STORY-P1-05-01`'s scope section,
2026-08-06:

- **`Tcb` carries no containment class.** Its fields are `base_priority`,
  `inherited_priority`, `wcet_budget`, `overrun_policy`, `entry`, `ticks_consumed` and
  `state`. A task cannot be asked which class it belongs to, so no allocation decision
  can be made on the basis of one.
- **The pool is one flat capacity** with no class tag and no reservation floor. Every
  caller draws from the same free list in arrival order.
- **A repository-wide search for a scheduling or allocation reserve finds none.**

This Feature builds the floor: a declared reserve that hostile allocation cannot cross,
and a per-class budget that makes "charged to the offender" a thing the kernel can
actually do rather than a thing a Report can claim.

## Why this is its own Feature and not a Story of `FEAT-P1-05`

`STORY-P1-05-01` costed its own prerequisites and said of this one, verbatim: *"This is
Feature-sized design work and probably its own Feature rather than a Story of this
one."* Acting on that is all this document does.

The consequence of **not** splitting it is the one worth recording. `FEAT-P1-05` has four
prerequisites; three of them are session-sized and one is not, and while they sat in one
Story the standing do-not-start rule — *"`FEAT-P1-05`'s RT reserve (Feature-sized, files
nothing in a session)"* — was read as covering all four. It never said that. But a rule
that names one item inside a Story that contains four will be applied to the Story, and
four consecutive handovers carried it that way while `FEAT-P1-06`'s half 3 waited behind
it. **Splitting the item is what makes the rule land where it was aimed.**

**This adds no design surface, and the distinction matters because the hardware-evidence
sprint rule from 2026-07-30 has not been lifted.** Every sentence of scope here was
already written inside `STORY-P1-05-01` on 2026-08-06. This document *moves* it; it does
not invent it. No new capability is proposed, no new subsystem is named, and the
containment contract below is a subset of `FEAT-P1-05`'s existing row rather than an
extension of it.

## Crate(s) involved

`os/src/kernel/` — the `Tcb` class field, the scheduler's reserve accounting, and the
pool's per-class floor. Nothing else: a reserve that reaches outside `kernel` is a
reserve the kernel cannot enforce.

## Depends on

`FEAT-P1-04` (preemption and deadline enforcement — a reserve that survives is only
meaningful where something can be preempted away from). Nothing else, and in particular
**not** `FEAT-P1-05`: the reserve is what that Feature's criterion 1 is asserted
*against*, so it precedes rather than follows it.

## What this Feature does NOT block, established before it was written

**`FEAT-P1-06`'s half 3 does not need the reserve.** Its two gates ask for measurements,
not guarantees, and the difference is the whole reason this section exists:

| gate | metric | target |
|---|---|---|
| `PERF-D05-G19` | `loaded_degradation` | p99 degradation ≤ 5%; max ≤ 10%; hard deadline misses = 0 |
| `PERF-D05-G21` | `fault_completion` | fault decision and containment ≤ 0.8 µs; leaks, deadlocks and unsafe retries = 0 |

Both are *run the RT task, flood the machine, measure what happens*. A reserve is what
would **guarantee** those targets; the gates only ask whether they **are** met. What they
need is a flooding fixture and a campaign harness — `STORY-P1-05-01`'s items 3 and 4,
both session-sized and neither blocked by anything.

**And the measurement should come first.** If `PERF-D05-G19` fails without a reserve,
that is not a wasted session: a failing `loaded_degradation` row is exactly the evidence
that justifies this Feature. Today the reserve is proposed on architecture prose. Every
other claim in this repository is held to a number, and this one should be too — measure
the degradation, then build the floor the number argues for, rather than building a floor
and measuring afterwards whether it was needed.

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) ·
implementation **C0** · subject **C1/C2** · boundary tests **BND-15, BND-16**.

Implementation is **C0 only**, and that is narrower than `FEAT-P1-05`'s `C0,C1`
deliberately: a reserve enforced from anywhere but the kernel's own allocation and
scheduling paths is a reserve a compromised C1 can decline to honour. The subject is
`C1/C2` — the classes whose budgets are being separated. `BND-20`
(single-compromise insufficiency) is **not** claimed here: proving it needs the flood,
which is `FEAT-P1-05`'s, and claiming it from a mechanism with no adversary exercising it
would be the shape `ADR 0005` calls a clean run proving nothing.

## Exit criteria

1. **A task's class is a declared field, not an inference.** `Tcb` carries a containment
   class, it is set at creation, and no path mutates it afterwards — asserted, because a
   class that can be changed at run time is a budget that can be bought.
2. **The pool has a per-class floor that allocation cannot cross**, and the refusal is
   `Err`, bounded in cost, and attributable. A floor enforced by convention rather than
   by the allocator is not a floor.
3. **The reserve is demonstrated to hold by being attacked**, not by being declared —
   `ADR 0005`'s standing trap, and this Feature's own detector. That demonstration is
   `FEAT-P1-05`'s campaign, so this Feature's exit is *reachable only through that one*,
   and the two are stated as separate Features precisely so neither is mistaken for the
   other's evidence.
4. **`BND-15` and `RCG-08` have a mechanism behind them**, named in this Feature's row
   rather than inherited from `FEAT-P1-05`'s.

**No `G04` row comes out of this.** Bound-class gates are refused at `T0`, on `x86_64`,
and from any platform absent from `qualified-platforms.tsv` — which holds zero qualified
platforms (`ADR 0005`, `LE-09`, `LE-94`). Stated here so the refusal lands at the start
of the work rather than at the end of it.

## Stories

**None, deliberately.** `agent.md` says decompose just-in-time and do not pre-build the
tree. This Feature exists to hold the scope that was blocking `FEAT-P1-05` by association,
and it should be decomposed when it is allocated — which is a deliberate act, not a
session's spare capacity.

## Named debt this Feature is answerable for

- `BND-15` and `RCG-08` are selected by `FEAT-P1-05`'s contract row today and have no
  mechanism. That does not change until this Feature lands; it is now *visible* rather
  than buried in a Story's scope section.
- The do-not-start rule now names **this** Feature. It was always aimed here.
