# TEST-P1-04-02-A — WCET Enforcement on the Real Timer

Status: **Verified 2026-07-28 — see the process note for how strictly, and clause 10 for what a falsification found**
Story: [`STORY-P1-04-02`](../stories/STORY-P1-04-02.md)
Tier: Host unit tests (the policy decision table, the attribution rule) **plus** Tier 0 QEMU runs of three new fixtures, one per policy arm, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D03`
Security controls: `SEC-14`, `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-15`, `BND-16`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-08`
Code admission gates: `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

`kernel::wcet::record_tick` has existed since `STORY-P0-02-04` with nothing driving it. Its own module doc has said so plainly for two Epics: "this kernel has neither a periodic timer-tick source that calls `record_tick` on its own … nor the documented watchdog/failsafe system to hand a detected overrun off to — both are concrete, still-open prerequisites, not silently assumed to exist." `STORY-P1-04-01` built the first. This test is the evidence for the second: that a task exceeding its declared budget is caught by real time, that the consequence it declared in advance actually happens, and that the tasks around it are unaffected.

## Specification

### 1. Attribution: a tick is charged to exactly the task that was running

**Given** the tick hook `STORY-P1-04-01` installed,
**then** each tick attributes exactly one tick to the running task, and a tick that lands in the dispatcher or an idle context is **attributed to nobody**.

This is the clause most likely to be got wrong in a way that still looks like it works. Charging kernel time to whichever task ran most recently makes every budget quietly wrong in the direction of over-charging the last task to yield — and a budget that is wrong in a consistent direction is worse than one that is obviously broken, because it produces plausible numbers. The attribution rule is therefore a pure function over "was a task running, and which", host-tested on both arms, and the hook is the only caller.

### 2. The policy decision table is total, and is *not* `kernel::fault::Disposition`

**Given** `kernel::wcet`'s new `OverrunPolicy` and its disposition function,
**then** each declared policy maps to exactly one disposition, every arm is pinned by a host test, and the mapping is exhaustive so a fourth arm added later cannot silently fall through:

| Declared `OverrunPolicy` | Disposition | What must actually happen |
|---|---|---|
| `Restart` | re-initialize and re-queue | context reset to entry, budget window reset, state `Ready` |
| `Degrade(floor)` | lower priority, keep running | priority becomes `floor`, budget window reset, state `Ready` |
| `TripToSafeState` | stop | task `Finished`, system enters its declared safe state |

**And `kernel::fault` is not modified.** An overrun is not a CPU fault: there is no frame, no vector, no hardware event, and — unlike a fault — a genuine choice of outcomes the task declared in advance. `Disposition::of` reads exactly one field on purpose and refuses both a `Resume` arm and a single-variant double-fault enumeration on the grounds that unreachable arms and non-decisions are liabilities. Routing overruns through it would mean giving it a second input and ending the invariant it exists to hold. A test asserts `Disposition::of`'s behaviour is byte-for-byte what it was before this Story.

### 3. A task cannot hold a budget with no declared consequence

**Given** task creation,
**then** the overrun policy is supplied alongside the WCET budget it governs, and there is no defaulted arm. A `Tcb` that carried a budget and no policy would be a task whose overrun behaviour is decided by whoever wrote the enforcement path rather than by whoever declared the task — which is the same "silently clamped priority" failure `sched::Priority` already refuses.

### 4. Tier 0: overrun is detected within a bounded number of ticks

**Given** each of the three fixtures,
**then** a task deliberately exceeding its declared budget is detected within an asserted, bounded number of ticks, with the bound a constant fixed **before the fixtures existed** — not a number read out of the capture. Detection happens on the tick that crosses the budget, which is `record_tick`'s existing contract and is re-checked here against real time rather than against a loop that calls it directly.

**The bound: `MAX_TICKS_TO_ENFORCE = 1`.** The offender declares a budget of `OFFENDER_BUDGET` ticks; enforcement must be applied no later than its `OFFENDER_BUDGET + 1`-th *attributed* tick — that is, at most one attributed tick may elapse between the budget being exhausted and the declared consequence being applied. One rather than zero because detection and disposition are separate steps, and a bound of zero would be a bound on the implementation's internal structure rather than on its observable behaviour.

**A correction recorded rather than papered over.** This clause was written at Story start naming a bound but never stating its number, so the number was in fact fixed at *implementation* start, not at specification time — after `record_tick`'s per-tick contract was known but before any fixture existed or any capture was taken. That is weaker than the discipline this document claims for itself and is recorded here as the deviation it is. Nothing was adjusted after a capture; see `REPORT-2026-07-28-04`.

**Observed: pass, on the exact tick, in all three runs.** The offender declares `OFFENDER_BUDGET = 4` ticks; enforcement fired on its 5th attributed tick in every run — the first tick that could possibly have crossed the budget, and one below the bound of 5.

```text
fixture-wcet-restart: enforcements=3 ticks_at_first_enforce=5 budget=4 (bound 5) first_enforce_tick=12 wrong_task=false wrong_disposition=false
fixture-wcet-degrade: enforcements=1 ticks_at_first_enforce=5 budget=4 (bound 5) first_enforce_tick=12 wrong_task=false wrong_disposition=false
fixture-wcet-trip:    TRIP task=0 attributed_ticks=5 budget=4 (bound 5) tick=12 within_bound=true task_finished=true
```

`wrong_task` and `wrong_disposition` are checked in the hook against the *mode*, not against whatever came back, so they assert that the declared policy produced its own disposition rather than that the enumeration round-trips.

Note `first_enforce_tick=12` against `ticks_at_first_enforce=5`: twelve real timer ticks elapsed, of which five were attributed to the offender and seven to the innocent RT task that kept preempting it. That gap *is* the attribution rule working — a naive implementation charging every tick to whoever ran last would have tripped this task at tick 5.

### 5. Tier 0: each policy arm is observed happening, not merely decided

The failure mode this clause exists to prevent is a fixture that asserts the disposition function returned the right value and never checks the system did anything about it.

- **Restart** — the restarted task is observed executing **from its entry point again**: a counter it increments on entry advances a second time, and a value it had accumulated before the overrun is gone. "It was marked Ready" is not the claim.
- **Degrade** — the degraded task is observed **losing a selection it would previously have won**. A competitor at a priority between the task's original and its floor must be Ready throughout and must start being chosen after the degrade and not before. Without that competitor the arm is indistinguishable from "reset the budget and carry on", which is the failure this arm most easily rots into.
- **TripToSafeState** — the system is observed **stopping, with its reason reported**, and the fixture's pass condition is a distinguishable fail-closed exit rather than success — the precedent `broken-boot` and `idt-apic-unrouted` already set for fixtures whose correct outcome is a failure code.

**Observed: pass, all three arms.**

**Restart.** The task entered its entry point three times (once plus two restarts), and each re-entry found real accumulated work and discarded it:

```text
fixture-wcet-restart: offender entries=3 acc_at_enforce=2265348 acc_on_reentry=1574987 restarts=3 exhausted=false
fixture-wcet-restart: ticks_attributed=[15, 40, 0] unattributed=0 unknown=0 preemptions=9
fixture-wcet-restart: spoors overrun=3 restart=3 degrade=0 terminate=0
TINYOS-RESULT/1 fixture=wcet-restart ok=true
```

`acc_on_reentry=1574987` is the claim: the accumulator is zeroed at the entry point and nowhere else, so a re-entry that found 1.57 million counts there is a re-entry that threw away 1.57 million counts of work. `ticks_attributed[0]=15` against three enforcements is the other half — exactly `3 × (budget + 1)`, which is the only observable consequence of the kernel having reset the budget window each time. **That second assertion was added because a deliberate falsification showed the fixture passing without it**; see clause 10.

**Degrade.** The dispatch order is the whole finding, and it splits cleanly at the enforcement:

```text
fixture-wcet-degrade: competitor counter=20000000 at_enforce=0 (target 20000000)
fixture-wcet-degrade: dispatch order=[1, 0, 1, 0, 1, 2, 1, 2, 1, 2, 1, 2, ... 1, 2]
fixture-wcet-degrade: spoors overrun=1 restart=0 degrade=1 terminate=0
TINYOS-RESULT/1 fixture=wcet-degrade ok=true
```

Slot 0 is the offender (priority 20 → floor 5), slot 2 the competitor (priority 15), `Ready` from before the overrun until after it. Before the degrade the competitor appears **nowhere** in the dispatch order and its counter is exactly `0`; after it, the offender appears nowhere and the competitor takes every selection it used to lose. That is a task losing a selection it would previously have won, not a priority field changing value.

**TripToSafeState.** The run stops, reports why, and exits with QEMU's failure code:

```text
fixture-wcet-trip: TRIP task=0 attributed_ticks=5 budget=4 (bound 5) tick=12 within_bound=true task_finished=true
fixture-wcet-trip: entering declared safe state — fail-closed stop is this fixture's pass condition
TINYOS-RESULT/1 fixture=wcet-trip ok=true
```

(xtask exit code 1.)

`task_finished=true` is read back out of the scheduler, not assumed from the returned disposition — see clause 10 for why that distinction is load-bearing here and not in `broken-boot`.

### 6. Tier 0: enforcement does not punish the innocent

**Given** an overrunning task and a within-budget task at RT priority running alongside it,
**then** the innocent task's own tick accounting is unaffected across the whole enforcement window: it is charged for the ticks it ran and no others, it does not overrun, and its own progress counter continues advancing across the detection and disposition. Measured on the same tick counter the enforcement uses, not asserted from the absence of a failure.

**Observed: pass, as an exact equality rather than as an absence.**

The tick hook keeps its own per-slot count of every tick it attributed, entirely independent of the scheduler's books, and `books_agree` asserts the two match task for task:

```text
fixture-wcet-restart: innocent counter=26000000 at_enforce=4000000 after=26000000 arms=12 books_agree=true
fixture-wcet-degrade: innocent counter=96000000 at_enforce=4000000 after=96000000 arms=47 books_agree=true
```

Three things are established together. The innocent task was charged for exactly the ticks it ran and no others (`books_agree=true` — a tick charged to the wrong task, charged twice, or charged to whoever ran last breaks this equality in one direction or the other). It never overran, despite the offender being terminated, restarted or demoted beside it. And its progress counter advanced from 4,000,000 at the moment of detection to 26,000,000 (restart) and 96,000,000 (degrade) by the end of the run — across the detection *and* the disposition, not merely up to it.

`unattributed=0` in every run: no tick ever landed in the dispatcher, which is expected rather than surprising — the dispatcher runs with `IF` clear, so a tick can only be delivered once a task has been switched into and `CURRENT_TASK` is already set. The `Nobody` arm is therefore exercised by host tests, not here, and this fixture deliberately does not gate on that count. See clause 9.

### 7. Every decision is audited, and there is no ignore branch

**Given** any overrun,
**then** a spoor is stamped with class/actor/action/outcome naming the task and the arm taken, in addition to the `Category::Wcet` / `Action::Overrun` spoor `record_tick` already stamps. Every disposition arm either changes the task's state or its priority — there is no arm that observes an overrun and does nothing — and a host test asserts the match is exhaustive so a later addition cannot fall through silently.

### 8. No regression in the bookkeeping half

**Given** `cargo test --workspace`,
**then** `wcet::record_tick`'s existing semantics are untouched and its existing tests pass unmodified: detection on the exact tick that crosses, consumption exactly equal to the budget is *not* an overrun, a reset clears prior consumption, and an unknown task fails closed with the handler never called and nothing stamped. Those tests are the no-regression guard for every pre-existing caller.

**And** `STORY-P1-04-01`'s two fixtures still pass unchanged — preemption and enforcement share the same hook, and a change to one must not perturb the other.

### 9. What this test explicitly does **not** establish

- **No hardware tier.** Tier 0 QEMU only; `LE-09` open. The detection bound is counted in *ticks*, not microseconds, for the same reason `TEST-P1-04-01-A` clause 4 gave: QEMU's APIC-timer-to-wall-clock relationship is not a number to build a `D03` budget on. **`D03` still has no measured baseline after this Story** — enforcement being *correct* and enforcement latency being *bounded in real time* are different claims, and only the first is made here.
- **No deadline monitoring.** A declared deadline is a different quantity from a declared execution budget; `FEAT-P1-04`'s title names both and this Story enforces the latter.
- **No periodic budget windows.** This scheduler has no notion of a task period, so a budget is per-activation and resets only where a policy arm resets it.
- **The shipping image does not enforce.** `os` installs no tick hook (`LE-20`), so this is proven in fixtures exactly as preemption was.
- **Not a safety case.** "Trips to a declared safe state" at Tier 0 means a reported fail-closed stop. What a real safe state is for a UAV or a medical device is a deployment question, not a kernel one.
- **The `Nobody` attribution arm is not exercised at Tier 0.** `unattributed=0` in every run, and that is structural rather than incidental: the dispatcher runs with `IF` clear, so a tick is only delivered once a task has been switched into and the running slot is already known. The arm is covered by host tests (including one that delivers a thousand dispatcher ticks and asserts the last task to run is charged none of them). The fixture reports the count and deliberately does **not** gate on it — whether a tick ever lands in the dispatcher is a property of QEMU's interrupt delivery, and gating on it would be gating on the emulator.
- **No interaction between degrade and priority inheritance.** `PriorityInheritingLock` records a holder's pre-boost priority and restores it on unlock. A boosted holder that is degraded mid-hold therefore has its degrade *undone* by the subsequent unlock, and a degrade applied to a boosted task discards a boost a high-priority waiter depends on. No fixture combines the two and no test pins the behaviour; recorded as `LE-22`.
- **No escalation on repeated overrun.** A degraded task that keeps overrunning is degraded again, to no further effect, and keeps running at its floor. That is pinned by a host test rather than left to be discovered, but it is a deliberate non-decision: escalation is a scheduling policy this Story has no requirement for.

### 10. The falsification, and the two fixture defects it found

A test nobody has watched fail is a test nobody has evidence for. Every enforcement mutation was removed from `wcet::apply` — the decision was still taken, the spoor still stamped, and nothing happened to the task — and everything was re-run.

**At the host level it worked as intended.** All five enforcement tests failed; the decision-table and attribution tests correctly stayed green, since neither depends on a mutation.

**At Tier 0 it found two real defects**, both of which had passed a green first run:

1. **`wcet-restart` passed with enforcement entirely removed.** The fixture was proving *its own* context rewind, not the kernel's. The hook rebuilds the offender's `Context`, and `dispatch::run_once`'s ordinary `Running → Ready` transition put the task back in the queue — together reproducing every *visible* effect of a restart. What they cannot reproduce is the spacing: a task whose budget window is never reset overruns again on the very next tick. With enforcement removed the offender took 7 attributed ticks across 3 enforcements; with it, exactly 15 across 3. The fixture now asserts every enforcement is a full `budget + 1` attributed ticks after the previous one, which is the only externally visible consequence of the window reset.

2. **`wcet-trip` reported `ok=true` with enforcement removed.** It checked the returned disposition and never checked that the kernel had actually marked the task `Finished`. This matters far more here than in a normal fixture, because this one's pass condition *is* a failure exit code — so a fixture that broke for any other reason exits 1 exactly as a correct trip does, and the exit code carries no information on its own. The fixture now reads the task's state back out of the scheduler (`task_finished=`), and the CI step greps the serial capture for it rather than trusting exit 1. This is a stronger pass condition than `broken-boot` or `idt-apic-unrouted` have, and the difference is deliberate.

With both fixed, the same falsification produces `ok=false` in all three runs (`wcet-restart` and `wcet-degrade` also flipping from exit 0 to exit 1), and reverting it restores all three to green.

## Process note: how strictly TDD was followed here

Clauses 1–9 were written **before any implementation code**, from the Story's finalized acceptance criteria — as with `TEST-P1-04-01-A`. The design decision that shaped clause 2 (that an overrun must not be routed through `kernel::fault::Disposition`) was made against the existing source *before* the Test document, and is recorded in the Story rather than discovered during implementation. Clause 10 was written after the fact, because it records something that was found rather than something that was specified.

The pure seams — attribution and the policy table — were drivable Red-to-Green in the ordinary way. **They were not driven that way, and that is the honest account.** The host tests and their implementation were written together, and the evidence offered in their place is clause 10's falsification: with the enforcement mutations removed, all five enforcement tests fail and the two that should not depend on a mutation stay green. That is stronger evidence than a Red that was never recorded, but it is not the same discipline, and the two should not be conflated. The one seam where a genuine Red was structurally impossible is the attribution rule's `Nobody` arm: removing the early return does not produce a failing test, it produces a program that does not compile, because there is no task to charge the tick to.

The three Tier 0 fixtures were not Red-to-Green either, for the reason the previous Test document recorded: a fixture's first run against real interrupt behaviour is a debugging exercise. What was held to instead is that the bounds and observable consequences in clauses 4–6 were fixed before the fixtures existed — with the one deviation recorded in clause 4 about *when* its number was fixed. Both Tier 0 fixtures that a green first run had wrongly reassured are documented in clause 10.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/kernel/src/wcet.rs`) plus Tier 0 QEMU runs (`qemu-x86_64 --fixture=wcet-restart`, `--fixture=wcet-degrade`, `--fixture=wcet-trip`).

## Implementation location

| What | Where |
|---|---|
| `TickAttribution`, `attribute_tick`, `OverrunDisposition`, `disposition_for`, `TickAccounting`, `account_tick`, `apply` | `os/src/kernel/src/wcet.rs` |
| `OverrunPolicy` (defined here, re-exported as `wcet::OverrunPolicy`), the per-task declaration, `create_task`'s new parameter, `overrun_policy_of`, `TaskCreateError::DegradeFloorAbovePriority` | `os/src/kernel/src/sched.rs` |
| `Action::Restart`, `Action::Degrade` | `os/src/kernel/src/spoor.rs` |
| All three fixtures | `os/src/kernel/src/fixture_wcet.rs` (one file — see its module doc for why) |
| `--fixture=wcet-{restart,degrade,trip}`, `--serial-capture=` | `os/src/xtask/src/main.rs` |
| Three feature gates, three entry points | `os/src/kernel/Cargo.toml`, `os/src/kernel/src/main.rs` |
| Three CI steps | `.github/workflows/ci.yml` |

`OverrunPolicy` is *defined* in `sched` rather than `wcet` and re-exported, so the module dependency stays one-way (`wcet` reads `sched`, never the reverse) — the same discipline `sched`'s own "no dependency on `context`" note records. `kernel::fault` and `kernel::dispatch` are **not modified by this Story**.

## Reports

- [`REPORT-2026-07-28-04`](../reports/REPORT-2026-07-28-04.md) — the captures, the two fixture defects the falsification found, the design decisions, and what remains open.
