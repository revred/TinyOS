# TEST-P1-11-01-A — A Round That Dispatched Nothing Must Say So

Status: **In progress — 6 host tests Green 2026-08-05; the Tier 1 clause has no evidence yet.**
Story: [`STORY-P1-11-01`](../stories/STORY-P1-11-01.md)
Tier: Host unit tests (`kernel::board_dispatch`, `hal_arm64::spoor` parity) **plus** a Tier 1 hardware capture, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D05`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-03`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

## What this test is for

The failure this guards against is not a crash. It is a park loop that calls a dispatch round
every beat, dispatches **nothing at all**, and looks exactly like one that works — because the
board is beaconing, the spoors are flowing, the thermal rung is sampling, and nothing anywhere
says otherwise. That is the same shape as `LE-71`, where a masked interrupt made a criterion
unsatisfiable on any board while every host test stayed green.

## Clauses

**Clause 1 — the sentinel cannot be mistaken for an answer.** `NO_TASK` is outside the task
index range and is not zero, because zero is the index this board actually dispatches.

**Clause 2 — the seam agrees on it.** `hal-arm64`'s copy equals `kernel::board_dispatch::NO_TASK`,
pinned by a parity test, exactly as the `Rung` vocabulary is.

**Clause 3 — a round that dispatched nothing stamps `Skipped` or `Failed`, never silence.**
This is the clause the whole test exists for.

**Clause 4 — the priority and stack are ones the kernel accepts.** Driven on the host, so a
board run cannot fail for a reason a host test could have caught.

**Clause 5 — a second initialisation is refused.** Rebuilding a context that may be suspended
mid-switch is worse than refusing.

**Clause 6 — the taxonomy matches the x86_64 path.** `Dispatch`/`Select`, so one decoder reads
either architecture without knowing which board it is reading.

**Clause 7 — Tier 1.** A capture carries `Dispatch Kernel Select Ok rung=DispatchRound` from a
running system, one per beat, with the outcome visible on every round.

## What this test does not cover

- **Preemption, `EL0`, containment, throughput.** None are claimed; the task yields immediately
  and does no work, so its own correctness is never between the claim and the evidence.
- **The cost of a round with interrupts live**, which is a different number from the fixture's
  masked measurement and is not taken here.
