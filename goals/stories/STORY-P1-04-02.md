# STORY-P1-04-02 — Deadline Monitor & WCET Watchdog on the Real Timer

Status: **In progress — acceptance criteria finalized 2026-07-28, Test document written**
Feature: [`FEAT-P1-04`](../features/FEAT-P1-04.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)
Started: 2026-07-28

## Description

`kernel::wcet::record_tick` stops being a bookkeeper waiting for a clock: the real timer drives per-task budget accounting, and an overrun triggers the task's declared fault policy — restart, degrade, or trip to safe state — through `FEAT-P1-02`'s fault machinery, with a spoor for every enforcement decision. This closes the "no timer, no watchdog" structural gap `STORY-P0-02-04` named and twice re-surfaced, and gives `G-RT-3` its enforcement half: budgets are held by the scheduler, not just declared.

## Depends on

`STORY-P1-04-01` (the tick hook that will drive accounting); `STORY-P1-02-01` (the policy lands in real fault handling).

## The design decision this Story has to make first

**A WCET overrun is not a CPU fault, and must not be routed through `kernel::fault::Disposition`.**

That module's own doc comment is explicit about why it is shaped the way it is: `Disposition::of` reads *exactly one field* — which context was running — and never the vector, the error code, the faulting address or the instruction pointer, because all of those come from arbitrary and possibly attacker-steered execution (`BND-04`). It also refuses a `Resume` arm on the grounds that an unreachable arm in a fault path is a liability rather than future-proofing, and refuses a `DoubleFaultDisposition` on the grounds that an enumeration with one variant is a decision that isn't one.

An overrun has none of a fault's properties. There is no fault frame, no vector, no hardware event — it is a *scheduler-detected budget condition*, and unlike a fault it has a genuine choice of outcomes that the task itself declared in advance. Adding overrun arms to `Disposition` would mean either giving it a second input (ending the one invariant it exists to hold) or making "which context faulted" answer a question it cannot answer.

So this Story adds a **separate, parallel policy type** — an `OverrunPolicy` declared per task at creation and an `OverrunDisposition` derived from it — which *composes with* `FEAT-P1-02`'s machinery (the same spoor journal, the same containment discipline, `TerminateTask` reused where the outcome really is termination) without extending it. `kernel::fault` is not modified by this Story, exactly as `kernel::dispatch` was not modified by `STORY-P1-04-01`.

## Acceptance criteria (finalized 2026-07-28 at Story start)

1. **Budget accounting is driven by the real timer.** Each tick attributes exactly one tick to the task that was running when it fired, via `wcet::record_tick`, from the same hook `STORY-P1-04-01` installed. A tick that lands in the dispatcher or an idle context is attributed to nobody — charging kernel time to whichever task happened to run last is how a budget silently becomes a lie.

2. **Every task declares its overrun policy, and there is no default that hides the choice.** The policy is part of task creation alongside the WCET budget it governs, so a task cannot exist with a budget and no declared consequence for exceeding it. The three arms are concrete and separately observable:
   - **Restart** — the task's context is re-initialized to its entry point, its budget window resets, it returns to `Ready`. It runs again from the beginning.
   - **Degrade** — the task's priority drops to a declared floor and its budget window resets. It keeps running, but it can no longer preempt anything above that floor. This is the arm that must not be allowed to quietly become "ignore".
   - **TripToSafeState** — the task is `Finished` and the system enters its declared safe state. At Tier 0 that is a reported, fail-closed stop, matching the precedent `Disposition::HaltSystem` already set.

3. **The overrun is caught within a bounded, asserted number of ticks**, and the bound is a constant in the fixture rather than a number read out of the capture afterwards — the same discipline `STORY-P1-04-01`'s tick bound held to.

4. **One Tier 0 fixture per policy arm**, each proving the arm actually happened rather than that a function was called: the restarted task is observed running from its entry again, the degraded task is observed losing a selection it would previously have won, and the tripped system is observed stopping with its reason reported.

5. **Enforcement never punishes the innocent.** A within-budget task at RT priority keeps meeting its deadlines while an offender is detected and handled — measured on the same tick counter, not asserted. The accounting path must not charge the offender's overrun, or the enforcement work, to anybody else.

6. **Every enforcement decision is a spoor with class/actor/action/outcome, and silent overruns are structurally impossible.** The accounting path has no "ignore" branch: every arm of the disposition either changes the task's state or its priority, and stamps. A test asserts the enumeration is exhaustively handled, so adding a fourth arm later cannot silently fall through.

7. **The bookkeeping half is unchanged.** `wcet::record_tick`'s existing semantics — detection on the exact tick that crosses, exactly-equal-to-budget is not an overrun, unknown task fails closed with no side effect — are the no-regression guard, and its existing tests must pass untouched.

## Explicitly out of scope

- **Wiring this into the shipping `os` image.** That is `LE-20`, and it is the same "proven in a fixture, not on the real boot path" shape `LE-05` had. It should not sit as long, but it is not this Story.
- **Periodic budget windows.** There is no notion of a task *period* in this scheduler, so a budget here is per-activation and resets only where a policy arm resets it. Inventing a period model this Story has no requirement for would be speculative.
- **Extending `kernel::fault`** — see the design decision above.
- **Deadline monitoring separate from WCET.** A declared deadline is a different quantity from a declared execution budget, and this Story enforces the latter. The Feature's title names both; the honest scope here is the budget.

## Tests

[`TEST-P1-04-02-A`](../tests/TEST-P1-04-02-A.md) — host unit tests for the policy decision table and the accounting attribution rule, plus three Tier 0 fixtures, one per policy arm.

## Goals verified

G-RT-3 (enforcement half), G-PA-1 (groundwork), G-SEC-14.
