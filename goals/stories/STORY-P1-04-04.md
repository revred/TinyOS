# STORY-P1-04-04 — Degrade and Priority Inheritance Compose

Status: **Functionally Verified (Host), 2026-07-28** — assurance state `baseline-debt`; `LE-22` closed. Host-level only, and deliberately so: the Tier 0 behavioural proof is `LE-50`, and the per-lock release hole found while fixing this is `LE-49`
Feature: [`FEAT-P1-04`](../features/FEAT-P1-04.md)
Introduced in: [`session/hand-2026-07-28/11-story-p1-04-02-wcet-enforcement.md`](../../session/hand-2026-07-28/11-story-p1-04-02-wcet-enforcement.md), which registered `LE-22` while building the degrade path that collides
Fix shape supplied by: the 2026-07-28 external expert audit recorded in [`29-next-session-mandate.md`](../../session/hand-2026-07-28/29-next-session-mandate.md)

## Description

**`LE-22`, and it is a live scheduler defect rather than a governance row.**

`crate::lock::PriorityInheritingLock` and `crate::wcet::apply` both write `Tcb::priority`. The lock
captures a value at boost time and replays it on unlock; the WCET path writes a declared floor when a
task overruns. Neither knows the other exists, so whichever writes last wins and **both orderings
lose something that matters**:

| Order | What is silently discarded | Why it matters |
|---|---|---|
| Degrade, then unlock | **The degrade.** `unlock` writes back the pre-boost priority | A task demoted for exceeding its declared budget walks away at its original priority. The enforcement decision is undone by an unrelated lock release |
| Boost, then degrade | **The boost.** `wcet` writes the floor while a waiter is still blocked | The priority inversion the boost exists to prevent, re-created by the enforcement path. `G-RT-1` is exactly the claim this breaks |

The second is the more serious: a high-priority waiter can be starved indefinitely because a
*different* task overran its budget. Under `agent/CODING_STANDARDS.md`'s priority ordering that is a
safety-and-correctness defect, above any performance consideration.

**Both halves are already documented in the code**, in `lock::PriorityInheritingLock::unlock` and in
`wcet::apply`'s `DegradeTo` arm, each pointing at the other and at the registered fix shape. This
Story removes those comments by removing the defect they describe.

### The fix, and the alternative it refuses

`effective(task) = max(base_priority, inherited_priority)`, evaluated on demand. `base_priority` is
the task's own and is what `degrade` lowers. `inherited_priority` is the highest priority currently
inherited from a waiter. **Neither subsystem writes the other's field, and neither writes the
effective priority at all** — it is derived, so there is no stored decision for a second writer to
invalidate.

`unlock`'s existing doc comment already refuses the tempting alternative: *"Do not 'fix' this by
making the restore conditional; that reintroduces the same defect with a narrower trigger."* A
conditional restore still stores a decision two subsystems can change. **The bug is the storage, not
the condition.**

## Depends on

`STORY-P0-02-03` (the inheritance mechanism), `STORY-P1-04-01` (its behavioural proof under real
preemption), `STORY-P1-04-02` (the degrade path that created the collision).

## Acceptance criteria

1. **Two fields, one derived.** `Tcb` carries `base_priority` and an optional `inherited_priority`;
   `effective_priority()` is their maximum. `Scheduler::priority_of` and `live_priority_of` return the
   effective value, unchanged in meaning, so `dispatch`, `preempt` and every fixture keep selecting
   and preempting on the same quantity with no edit. The writers are separated by name —
   `set_base_priority` for `wcet`, `inherit_priority`/`release_inheritance` for `lock` — so neither can
   reach the other's field.
2. **Degrade-then-unlock: the degrade survives.** A task degraded to floor 2 while boosted to 25 by a
   waiter leaves `unlock` at **2**, not at its pre-boost 5.
3. **Boost-then-degrade: the waiter keeps its boost.** That task's effective priority never falls
   below 25 while the waiter is blocked, and a medium task at 15 never outranks it in that window.
4. **Nothing regresses in either subsystem alone.** Uncontended boost/release behaves exactly as
   `STORY-P0-02-03` specified; an overrun with no lock held behaves exactly as `STORY-P1-04-02`
   specified. Inheritance raises and never lowers, structurally, via `max`.
5. **The tests are shown to fail on the defect.** Reverting the composition -- `set_base_priority`
   also clearing the inherited priority, the old single-field collision -- turns them red with
   diagnostics that name the defect, while **every pre-existing `lock.rs` test stays green**, which is
   the demonstration that the old suite could not see this. The overrun is driven through
   `wcet::account_tick`, the path the timer ISR calls. Per `ADR 0005` and `STORY-P0-01-07` clause 2.

## Named debt this Story leaves open

- **Two contended locks held by one task (`LE-49`, registered here).** `release_inheritance` clears
  the task's inherited priority outright, so unlocking one contended lock drops a boost the other's
  waiter still needs. **Not a regression** — the old code wrote back a stale absolute value in the
  same scenario, which is worse — but a real remaining hole. The correct fix needs per-lock
  inheritance records, which needs blocking waiters, which this kernel does not have.
- **Inheritance is not transitive.** A holder itself blocked on a second lock does not propagate its
  inherited priority onward. Untouched.
- **`LE-45` is not claimed as explained.** The soak anomaly was a `priority-inversion` fixture
  returning a non-zero exit with **no serial capture** (`LE-46`), in exactly this subsystem. An
  unreproduced failure with no diagnostic cannot be attributed to a defect after the fact, and doing
  so would be the over-claiming [`40A`](../../session/hand-2026-07-28/40A-soak-anomaly-decision.md)
  refused. `LE-45` stays open on its own terms.
- **The Tier 0 behavioural proof is not claimed (`LE-50`, registered here).** The composed scenario
  needs its own fixture, feature flag, `xtask` table entry and CI step; extending
  `fixture_priority_inversion` would alter evidence a Verified Story rests on. Nothing blocks it. The
  defect is fixed and demonstrated at the host level against the real enforcement path -- what is
  outstanding is the proof under preemption, exactly as `STORY-P0-02-03` once carried it for the
  un-composed case.
- **No performance guardrail closes.** `D03`/`D05`/`D06` are selected because the composition governs
  dispatch and lock latency, not because this Story measures either.

## Tests

[`TEST-P1-04-04-A`](../tests/TEST-P1-04-04-A.md) — written before implementation, per the TDD mandate.

## Reports

- [`REPORT-2026-07-28-12`](../reports/REPORT-2026-07-28-12.md)
