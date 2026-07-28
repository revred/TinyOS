# TEST-P1-04-04-A — Degrade and Priority Inheritance Compose: One Effective Priority, Evaluated On Demand

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P1-04-04`](../stories/STORY-P1-04-04.md)
Tier: Host unit tests only -- the composition algebra in both directions, driven through the real enforcement path. The Tier 0 behavioural proof is deliberately **not** claimed; see clause 5 and `LE-50`
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D03`, `D05`, `D06`
Security controls: `SEC-14`, `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-15`, `BND-16`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-08`
Code admission gates: `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

`LE-22`, and it is a real scheduler defect rather than a governance row.

Two subsystems write one quantity. `crate::lock::PriorityInheritingLock` boosts a lock holder to a
waiter's priority and, on unlock, **writes back a value it captured at boost time**.
`crate::wcet::apply` degrades an overrunning task to its declared floor by **writing the same field**.
Whichever writes last wins, silently, and both orderings are wrong:

- **Degrade-then-unlock.** A task overruns while holding a contended lock. `wcet` lowers it to its
  floor. The later `unlock` restores the *pre-boost* priority — **the degrade is silently undone**, and
  a task that was demoted for exceeding its budget walks away at its original priority.
- **Boost-then-degrade.** A task is boosted because a high-priority waiter needs it to finish. It
  overruns; `wcet` writes the floor. **The boost is discarded while the waiter is still blocked**,
  which is the priority inversion the boost existed to prevent, re-created by the enforcement path.

The first is a safety-policy escape; the second is an unbounded-latency hole. Under
`agent/CODING_STANDARDS.md`'s priority ordering, both are above performance and the second is above
correctness — a waiter that can be starved indefinitely by an unrelated overrun is the failure
`G-RT-1` exists to deny.

**Both defects are already documented in the code**, in `lock::PriorityInheritingLock::unlock` and in
`wcet::apply`'s `DegradeTo` arm, each pointing at the other. This Story removes the comments by
removing the defect.

### The fix shape is named, and one alternative is explicitly refused

The 2026-07-28 external audit supplied it: **a dynamic effective priority**, evaluated on demand,

```text
effective(task) = max(base_priority, inherited_priority)
```

where `base_priority` is the task's own — the field `degrade` lowers — and `inherited_priority` is
the highest priority currently inherited from a waiter. Neither subsystem writes the other's field
and neither writes `effective` at all.

**Making the restore conditional is refused**, and `unlock`'s existing doc comment already says so:
*"Do not 'fix' this by making the restore conditional; that reintroduces the same defect with a
narrower trigger."* A conditional restore still stores a decision that two subsystems can both
change; it merely narrows the window in which the stored value is stale. The bug is the storage.

## Specification

### 1. The scheduler holds two fields and derives the third

**Given** a `Tcb`,
**then** it carries `base_priority` and an optional `inherited_priority`, and exposes
`effective_priority()` computed as their maximum. **No field named `priority` remains writable by
two callers.**

**And** `Scheduler::priority_of` and `Scheduler::live_priority_of` return the **effective** priority,
unchanged in meaning for every existing reader — `dispatch`, `preempt`, and the fixtures all select
and preempt on effective priority today and must keep doing so with no edit.

**And** the two writers are separated by name and cannot reach each other's field:
`set_base_priority` (what `wcet::apply` calls) and `inherit_priority` / `release_inheritance` (what
`lock` calls). A caller that wants to lower a task's own priority cannot accidentally clear an
inheritance, and a caller releasing an inheritance cannot resurrect a stale base.

### 2. Degrade-then-unlock: the degrade survives

**Given** `low` at priority 5 with `OverrunPolicy::Degrade(floor = 2)`, holding a lock,
**and** `high` at priority 25 contending, so `low`'s effective priority is 25,
**when** `low` overruns its budget and `wcet::apply` degrades it,
**then** `low`'s **base** becomes 2 and its **effective priority stays 25**, because the waiter is
still blocked and still needs it to finish.

**And when** `low` unlocks,
**then** its effective priority becomes **2 — the floor, not the pre-boost 5.** The degrade survives
the unlock. This is the assertion the old code fails.

### 3. Boost-then-degrade: the waiter keeps its boost

**Given** the same scenario,
**then** at no point between the boost and the unlock does `low`'s effective priority fall below 25.
Specifically, the degrade **does not** lower it to 2 while `high` is still waiting.

**And** a third task `medium` at priority 15 — outranking both `low`'s base (5) and its floor (2) —
must not outrank `low` at any point in that window, because that is precisely the inversion the boost
prevents and the degrade was re-creating.

### 4. Nothing regresses when only one subsystem is involved

**Given** a contended lock with no overrun,
**then** boost and release behave exactly as `STORY-P0-02-03` specified: the holder rises to the
waiter's priority and returns to its own on unlock. Every existing test in `lock.rs` passes unmodified
in meaning.

**And given** an overrun with no lock held,
**then** the degrade behaves exactly as `STORY-P1-04-02` specified: priority drops to the floor, the
budget window resets, the task stays `Ready`.

**And** a boost that would *lower* the holder's effective priority is still not applied — inheritance
raises, never lowers, and `max` makes that structural rather than a guard someone can forget.

### 5. The tests are shown to fail on the defect, against the real enforcement path

The clause carried forward from `STORY-P0-01-07` and `ADR 0005`: **an instrument never demonstrated
to detect anything cannot be believed when it reports a pass.**

**Given** the composition reverted — `set_base_priority` also clearing the inherited priority, which
is exactly the old single-field collision —
**then** the tests above must go **red**, with diagnostics naming the defect rather than a generic
mismatch.

**And** the overrun in those tests is driven through `wcet::account_tick`, the entry point the timer
ISR actually calls and the only one that both detects an overrun *and* applies the declared policy.
Driving `record_tick` alone would exercise detection without enforcement and prove nothing about the
composition.

**And** the pre-existing `lock.rs` tests must **stay green** under that same falsification. That is
not a footnote: it is the demonstration that the old suite could not see this defect, which is why it
shipped.

#### Tier 0 is deliberately not claimed here, and this is the reason

This clause originally required a QEMU fixture running the composed scenario under genuine
timer-driven preemption. **It does not, and the Story is `Host` rather than `Tier 0 + Host`
accordingly.**

`kernel::fixture_priority_inversion` already proves the *inheritance* half under real preemption
(`TEST-P1-04-01-A` clause 6). The composed scenario needs a second fixture — its own feature flag, its
own entry in `xtask`'s fixture table, its own CI step, and its own passing run — and extending the
existing one would alter the evidence a Verified Story rests on. That is a self-contained piece of
work, not a line of it is blocked, and **inventing a weaker Tier 0 claim to close this clause on
schedule is precisely what `ADR 0005`'s trap section forbids.**

Registered as **`LE-50`**. The defect itself is fixed and demonstrated at the host level against the
real enforcement path; what is outstanding is the behavioural proof under preemption, exactly as
`STORY-P0-02-03` once carried it for the un-composed case.

### 6. The known-defect comments are removed, not amended

**Given** `lock::PriorityInheritingLock::unlock` and `wcet::apply`,
**then** neither carries a `KNOWN DEFECT (LE-22)` section any more, because the defect is gone. Each
instead documents the composition rule and points at the other, so the next reader learns the
invariant rather than the history.

## What this test explicitly does not establish

- **That a task holding *two* contended locks releases inheritance correctly.** It does not:
  `release_inheritance` clears the task's inherited priority outright, so unlocking one contended lock
  drops a boost the other's waiter still depends on. This is **not a regression** — the previous code
  was worse in the same scenario, writing back a stale absolute value rather than falling to base —
  but it is a real remaining hole, registered rather than fixed here. The correct fix needs per-lock
  inheritance records, which needs blocking waiters, which this kernel does not have (`lock.rs`'s
  `try_lock` reports contention rather than parking). Registered as **`LE-49`**.
- **That priority inheritance is transitive.** A holder blocked on a second lock does not propagate
  its inherited priority onward. Out of scope and untouched by this Story.
- **Any timing guardrail.** No `PERF-Dnn-Gnn` closes here. `D03`/`D05`/`D06` are selected because the
  composition governs dispatch and lock latency, not because this Story measures them.
- **That `LE-45`'s soak anomaly is explained.** The anomaly was a `priority-inversion` fixture
  returning a non-zero exit with no serial capture (`LE-46`), and this Story touches exactly that
  subsystem — but an unreproduced failure with no diagnostic **cannot be attributed to a defect after
  the fact**, and claiming this fixed it would be precisely the over-claiming `40A` refused. `LE-45`
  stays open on its own terms.

## Reports

- [`REPORT-2026-07-28-12`](../reports/REPORT-2026-07-28-12.md)
