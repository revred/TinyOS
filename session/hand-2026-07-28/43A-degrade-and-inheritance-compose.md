# Handover 43A — A Real Scheduler Defect, Fixed and Shown To Have Been Real

**Kernel code, not governance.** `LE-22` is closed by `STORY-P1-04-04`. This is the first substantive
behaviour change in four sessions, and the three before it were machinery — that imbalance was noted
by the owner and is the reason this session went at the most serious *substantive* row open rather
than the next item on a checklist.

`STORY-P1-04-04` · [`TEST-P1-04-04-A`](../../goals/tests/TEST-P1-04-04-A.md) ·
[`REPORT-2026-07-28-12`](../../goals/reports/REPORT-2026-07-28-12.md). **Host tests 607 → 613**,
`kernel` 121 → 127.

## 1. The defect

`crate::lock::PriorityInheritingLock` and `crate::wcet::apply` both wrote `Tcb::priority`. The lock
captured the holder's pre-boost priority at boost time and replayed it on unlock; the WCET path wrote
a declared floor on overrun. **Whichever wrote last won, silently**, and both orderings lost something
that mattered:

| Order | Discarded | Consequence |
|---|---|---|
| Degrade → unlock | **the degrade** | A task demoted for blowing its budget walks away at its original priority. The enforcement decision undone by an unrelated lock release |
| Boost → degrade | **the boost** | The priority inversion the boost exists to prevent, re-created by the enforcement path. A high-priority waiter starvable because a *different* task overran |

The second is a `G-RT-1` defect: unbounded waiter latency caused by an unrelated task's overrun.
Under the standing priority ordering that sits above correctness, not beside it.

## 2. The fix, and the alternative the code had already refused

```text
effective(task) = max(base_priority, inherited_priority)
```

Evaluated on demand, **never stored**. `base_priority` is the task's own and is the only field the
degrade lowers; `inherited_priority` is what a waiter grants and is written only by the lock.
**Neither subsystem writes the quantity the scheduler actually reads.** There is no stored decision
left for a second writer to invalidate, because there is no stored decision.

The writers are separated by name — `set_base_priority` versus
`inherit_priority`/`release_inheritance` — so neither can reach the other's field by accident, which
is exactly what both of them were doing. `max` also makes *"inheritance raises and never lowers"*
structural rather than a guard at each call site.

`unlock`'s own doc comment had already refused the tempting shortcut, and it was right: *"Do not
'fix' this by making the restore conditional; that reintroduces the same defect with a narrower
trigger."* **The bug was the storage, not the condition.**

## 3. The part that matters: it was shown to have been real

Reverting the composition — `set_base_priority` also clearing the inherited priority, which is
precisely the old single-field collision — gives:

```text
---- lock::tests::a_degrade_taken_while_boosted_survives_the_unlock stdout ----
assertion `left == right` failed: the waiter still needs the holder to finish
  left: Some(Priority(2))
---- lock::tests::the_two_priority_writers_are_independent stdout ----
assertion `left == right` failed: degrade cannot cancel a boost
  left: Some(Priority(3))

test result: FAILED. 9 passed; 4 failed
```

Four of the six new tests go red with diagnostics that name the defect.

**And all seven pre-existing `lock.rs` tests stayed green under that same falsification.** That is the
finding worth carrying: the old suite asserted priority values in scenarios where only *one*
subsystem was involved, so it could not see a collision that needs both. **A suite that cannot fail
on a defect is not evidence the defect is absent** — the same sentence `ADR 0005` applies to
qualification campaigns, arriving here from a completely different direction.

The overrun is driven through **`wcet::account_tick`**, the entry point the timer ISR calls and the
only one that both detects an overrun *and* applies the policy. `record_tick` alone only detects;
testing against it would have exercised half the path.

## 4. What is deliberately not claimed

This is the section to read before quoting anything above.

- **No Tier 0 fixture (`LE-50`).** The composed scenario has not run under real timer-driven
  preemption. `fixture_priority_inversion` proves the *inheritance* half that way; the composed case
  needs its own fixture, feature flag, `xtask` table entry and CI step, and extending the existing one
  would alter evidence a Verified Story rests on. **Nothing blocks it** — it was cut for time, and the
  Story is `Host` rather than `Tier 0 + Host` accordingly. `TEST-P1-04-04-A` clause 5 was rewritten to
  say so rather than quietly dropped, because inventing a weaker Tier 0 claim to close a clause on
  schedule is what `ADR 0005`'s trap section forbids.
- **Two contended locks are still wrong (`LE-49`).** `release_inheritance` clears inheritance
  outright, so unlocking one contended lock drops a boost the other's waiter needs. **Not a
  regression**: the old code wrote back a stale *absolute* value in the same scenario, which could
  raise a task above any priority it was ever entitled to. The new hole is strictly smaller and fails
  toward the base rather than toward a fabricated high. Found while fixing `LE-22`, not reported.
- **`LE-45` is not explained, and the temptation to say it is was real.** The soak anomaly was a
  `priority-inversion` fixture returning a non-zero exit with **no serial capture** (`LE-46`), in
  exactly this subsystem, hours before this defect was fixed. An unreproduced failure with no
  diagnostic **cannot be attributed to a defect after the fact** — that is the over-claiming
  [`40A`](40A-soak-anomaly-decision.md) refused when it was the owner's own gate at stake. What can
  honestly be said is narrower and still worth saying: **a real defect did exist in that subsystem, so
  "environmental artifact" was never the only live hypothesis.** `LE-45` stays open on its own terms,
  and `LE-46` — run the sweep under capture — just got more valuable.

## 5. `goals/index.html`

Updated, and this is the first session where that was not a hand-sync: `STORY-P1-04-04` was added to
`FEAT-P1-04`'s list, the Feature's badge notes the reopen, and **every number came from
`emit-dashboard`**. The `LE-30` gate then caught two things in this session's own work — a stale
count sentence, and a badge reading `FUNCTIONALLY VERIFIED (Host)` for a Story whose header still
said `Specified`. Both were named precisely by the error rather than found by a reader. That is the
machinery from `42A` paying for itself on its first day.

## 6. Concurrency

Two register rows arrived mid-session from another session: `LE-47` (Pi 5 evidence-egress option
space) and `LE-48` (MS-DOS 4+ ergonomic parity). **`LE-48` collided with the number this session had
drafted**, so its two rows were renumbered to `LE-49`/`LE-50` and every reference re-pointed before
the register was validated. Neither of those rows is this session's and no credit for them is claimed.
`41A-dos-parity-standing.md` is untracked and belongs to that session; it is left alone.

## 7. The work order

| # | Action | Blocked on |
|---|---|---|
| **W1** | **The board.** Moves at most 46 of 391 release gates, and remains the highest-value session because `Q1`/`Q2` and every `T1`/`T2` bound depend on it. **A procurement decision, not an engineering one** (`41A` §4.1) | An adapter |
| **`LE-50`** | **The Tier 0 fixture for this Story.** Template exists (`STORY-P1-04-01`), scenario is written down, nothing blocks it. **This is the smallest piece of real engineering currently available** | Nothing |
| **`LE-42`** | The `D09` accept path at 17.6–39.1× its own budgets. Still the most serious *unanalysed* substantive finding | A decision; `W3` first |
| **W3 / `LE-23`** | Re-record the baseline from a CI run. The data to act on already exists | Nothing |
| **`LE-40`** | `exec::shared_memory::grant` panics rather than failing closed — a containment defect in a fail-closed system, and the same *shape* as `LE-22`: a rule the code states and does not enforce | Nothing |
| **W2** | The `-M virt` fixture. Three decisions | Nothing |
| **`LE-49`** | Per-lock inheritance records. Needs blocking waiters, so it is a scheduler Story rather than a lock patch | A scope decision |

**Recommendation, since the last four sessions have skewed toward machinery**: `LE-50` then `LE-40`.
Both are kernel work, both are small, both are fully unblocked, and `LE-40` is the same defect shape
as the one closed today — a rule stated in a comment with nothing enforcing it.

## State at the close

```text
main                    b4f590e + this session's commit
                        FOURTEEN commits ahead of origin before this one, UNPUSHED
assurance spine         23 Features, 61 Stories, 48 Tests, 49 Reports
                        50 loose ends (30 open), 87 status headers
                        11 release gates with evidence, of 391
                        345 of 391 reachable with no board
                        61 Feature/Story status rows agree, 50 dashboard badges agree
host tests              613 across the workspace, from 607; kernel 127, from 121
kernel behaviour        CHANGED -- degrade and priority inheritance now compose
loose ends closed       LE-22. Registered: LE-49, LE-50
Stories verified        0 / 61 assurance-verified; unchanged and correct
best UNBLOCKED work     LE-50's fixture, then LE-40
```

`goals/reports/_soak-p0-03-01.log` is still dirty and still left alone. Ninth session.
