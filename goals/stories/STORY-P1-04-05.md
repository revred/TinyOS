# STORY-P1-04-05 — The Composed Degrade/Inheritance Scenario, Proved Under Real Preemption

Status: **Verified (Tier 0), 2026-07-28** — assurance state `baseline-debt`; `LE-50` closed. Tier 0 only, and correctly so: this Story adds the behavioural proof and changes no kernel behaviour, so the host-level composition algebra remains `STORY-P1-04-04`'s evidence
Feature: [`FEAT-P1-04`](../features/FEAT-P1-04.md)
Introduced in: [`session/hand-2026-07-28/43A-degrade-and-inheritance-compose.md`](../../session/hand-2026-07-28/43A-degrade-and-inheritance-compose.md), which fixed `LE-22` at the host level and registered this gap as `LE-50` rather than quietly absorbing it

## Description

**`LE-50`.** `STORY-P1-04-04` closed `LE-22`: degrade and priority inheritance compose, because the
priority the scheduler reads is `max(base_priority, inherited_priority)`, derived on demand and never
stored. It demonstrated that at the **host** level, driving the overrun through `wcet::account_tick`
— the entry point the timer ISR actually calls. It deliberately did **not** claim Tier 0, and said so
in its own acceptance criteria rather than in a footnote.

This Story is that claim. One new fixture, `degrade-inheritance`, runs the composed scenario under
genuine timer-driven preemption and makes the evidence *who ran*, not what a field contained.

**Why a separate fixture rather than an extension of `fixture_priority_inversion`.** That fixture is
the evidence `STORY-P1-04-01` is Verified on. Adding a WCET budget and an overrun to it would change
what a Verified Story rests on in order to save a file — the same reasoning `STORY-P1-04-02` used when
it built `fixture_wcet` rather than growing `fixture_preempt`.

**Why this is worth Tier 0 at all, given the host tests already pass.** A host test asserts priority
values. This asserts a scheduling outcome: a `medium` task at priority 15, `Ready` throughout and
demonstrably able to run, makes **no progress at all** across a window that contains a WCET degrade of
the boosted holder. Under the pre-`STORY-P1-04-04` kernel that degrade discarded the boost, `low` fell
to its floor of 2 while `high` was still blocked, and `medium` would have started winning selections
immediately. The frozen counter is therefore a claim the old kernel could not have satisfied — which
is what makes this a demonstration rather than a re-assertion.

## Depends on

`STORY-P1-04-01` (timer-driven preemption, and the fixture template this one follows),
`STORY-P1-04-02` (the WCET enforcement path the overrun is driven through),
`STORY-P1-04-04` (the composition itself — this Story proves it, it does not implement it).

## Acceptance criteria

1. **The composed scenario runs under real ticks.** `low` (5, budget 4, `Degrade(floor = 2)`) takes a
   lock and never yields; `high` (25) preempts it, contends, boosts it to 25 and blocks; `medium` (15)
   is `Ready` throughout. `low` is preempted by the timer, overruns, is degraded, releases, and `high`
   completes. Every enforcement names `low` and carries the disposition its *declared* policy maps to.
2. **The boost survives the degrade, observed from inside the ISR.** At the instant
   `wcet::account_tick` applies the enforcement, `low`'s base priority is 2 **and** its effective
   priority is still 25 — both read from interrupt context on the tick the decision was made.
3. **`medium` makes no progress across the window.** Its counter is identical when `high` blocks, when
   the degrade fires, and when `low` unlocks; it is `Ready` at all three points; and its slot appears
   nowhere in the dispatcher's selection log before the release. All three, because each alone can be
   satisfied for the wrong reason.
4. **`low` leaves the lock at its floor, and the dispatcher acts on it.** Effective priority 2 after
   the unlock, not the pre-boost 5 — and `low` stays `Ready` and busy rather than retiring, yet its
   slot never appears in the selection log again, because `medium` at 15 now outranks it. `medium`
   then makes real progress past a floor fixed in advance.
5. **The fixture is shown to detect the defect.** With the composition reverted in `sched`
   (`set_base_priority` also clearing the inherited priority — the old single-field collision), the
   fixture **fails**, and its serial output names the failing clause rather than reporting a bare
   non-zero exit. Per `ADR 0005` and `STORY-P0-01-07` clause 2.
6. **The fixture is wired the way every other one is.** Its own feature flag, its own row in `xtask`'s
   fixture table naming an owning Test that exists, and its own CI step — so the two drift guards in
   `xtask`'s test module (`every_ci_fixture_value_exists_in_the_table` and
   `every_fixture_in_the_table_is_run_by_ci`) both cover it. A fixture that exists and is never run is
   an unverified fixture that looks verified.

## Named debt this Story leaves open

- **`LE-49` is untouched.** One lock, one waiter. A task holding *two* contended locks still loses the
  second lock's boost when it releases the first, and this fixture cannot see that because it never
  holds two. Fixing it needs per-lock inheritance records, which needs blocking waiters, which this
  kernel does not have.
- **Inheritance is still not transitive.** `high` blocks on nothing further. Untouched.
- **No performance guardrail closes.** `D03`/`D05`/`D06` are selected because the composition governs
  dispatch and lock latency, not because this Story measures either. No `guardrail-evidence.tsv` row
  is filed and no `TOS64-MEAS/2` envelope is emitted.
- **`LE-45` is not explained by this Story either.** It adds Tier 0 coverage next door to the fixture
  that logged the anomaly; it does not recover a diagnostic that was never captured (`LE-46`). Both
  rows stay open on their own terms.

## Tests

[`TEST-P1-04-05-A`](../tests/TEST-P1-04-05-A.md) — written before implementation, per the TDD mandate.

## Reports

- [`REPORT-2026-07-28-13`](../reports/REPORT-2026-07-28-13.md)
