# Handover 11 — `STORY-P1-04-02`: WCET enforcement on the real timer, and `FEAT-P1-04` exits

Written at the close of 2026-07-28. `STORY-P1-04-02` is Verified, `LE-02` is closed, and **`FEAT-P1-04` meets every exit criterion**. `EPIC-P1` is now four Features of six.

## What changed

`kernel::wcet::record_tick` has existed since `STORY-P0-02-04` with nothing driving it and nothing to hand a detected overrun off to — its own module doc has said exactly that for two Epics. `STORY-P1-04-01` built the first missing half. This Story built the second: the real local-APIC tick now charges execution to the running task, and a task that crosses its declared budget has the consequence **it declared at creation** applied to it, audited, within a bound fixed before the fixtures existed.

Detection landed on the offender's 5th attributed tick against a budget of 4, in all three runs — the first tick that could possibly have crossed it.

Read [`goals/tests/TEST-P1-04-02-A.md`](../../goals/tests/TEST-P1-04-02-A.md) for the captures and [`goals/reports/REPORT-2026-07-28-04.md`](../../goals/reports/REPORT-2026-07-28-04.md) for the findings. This document is the short version plus the loose-ends delta.

## The finding worth carrying: all three fixtures passed first time, and two proved nothing

This is the lesson of the session and it generalises past this Story.

Every enforcement mutation was removed from `wcet::apply` — the decision still taken, the spoor still stamped, nothing done to the task — and everything re-run. Host tests behaved: five of five enforcement tests failed, and the two that legitimately don't depend on a mutation stayed green. **At Tier 0 it found two defects in the evidence.**

**`wcet-restart` stayed green with enforcement entirely removed.** The fixture was proving its *own* context rewind. The hook rebuilds the offender's `Context` — that half genuinely belongs to the caller, since `wcet` owns neither stacks nor entry points — and `dispatch::run_once`'s ordinary `Running → Ready` transition re-queued the task. Between them they reproduce every *visible* effect of a restart: entry counter advances, accumulator zero, task runs again.

What they cannot reproduce is the **spacing**. A task whose budget window is never reset is over budget on every subsequent tick, so enforcements pile up one per tick instead of one per window — 7 attributed ticks across 3 enforcements, against 15 across 3 when it works. The fixture now asserts every enforcement is a full `budget + 1` attributed ticks after the last, which is the only externally visible consequence of the kernel's reset and the one thing the fixture's own machinery cannot fake.

**`wcet-trip` reported `ok=true` with enforcement removed.** It checked the disposition it was handed and never checked the kernel had marked the task `Finished`. That matters more here than anywhere else in this workspace, because **this fixture's pass condition is a failure exit code**. `broken-boot` and `idt-apic-unrouted` set that precedent and the precedent has a hole: a fixture that breaks for any other reason exits 1 exactly as a correct run does. The falsification demonstrated it directly.

The fixture now reads the state back out of the scheduler (`task_finished=`), and its CI step greps the serial capture for that and for `ok=true` rather than trusting exit 1. **That is a stronger pass condition than the two fixtures whose precedent it follows**, and it is worth going back for: `broken-boot` and `idt-apic-unrouted` have the same hole and nobody has looked.

Neither defect was in the kernel. Both were in the evidence, and both would have shipped as a green Tier 0 fixture proving nothing. Criterion 4 of the Story was written to prevent precisely this, and a careful reading did not catch it — a falsification did.

## Design decisions that should not be re-litigated

- **An overrun is not a fault.** `kernel::fault` is untouched. `Disposition::of` reads exactly one field because every other quantity at fault time comes from possibly attacker-steered execution (`BND-04`); `wcet::disposition_for` holds the same discipline over a different input — only the policy declared *in advance*, never how far over budget, never how many times. A task cannot influence its own consequence by misbehaving harder. A host test asserts `Disposition::of` still answers as it did, so a later change routing overruns through it must edit that test.
- **The policy is a parameter of `create_task` with no default.** 78 call sites. A defaulted field would have made a task's overrun behaviour the property of whoever wrote the enforcement path. There is no setter — the declaration is immutable for the task's life.
- **A degrade floor above the task's own priority is rejected, not clamped** (`TaskCreateError::DegradeFloorAbovePriority`, checked before a slot is claimed). An overrun that *raised* priority would make missing a deadline a route to a criticality level nobody granted.
- **`OverrunPolicy` is defined in `sched` and re-exported as `wcet::OverrunPolicy`.** Defining it in `wcet` would have made `sched` depend on `wcet` while `wcet` already depends on `sched`.
- **One fixture file, three feature-gated arms.** The claim is that the *same* enforcement path produces three correct outcomes because the *declaration* differed; three files would let them drift.
- **Clause 6's evidence is an equality, not an absence.** The hook keeps its own per-slot tick count independent of the scheduler's books and asserts they agree task for task (`books_agree=true`). A tick charged to the wrong task, twice, or to whoever ran last breaks it.

## Two smaller things this session added

**`xtask qemu-x86_64 --serial-capture=PATH`.** Fixtures ran with `-serial none` and reported a single pass/fail bit, so a fixture's own diagnostic lines were unreadable and no Test document could quote a capture without hand-driving QEMU. Opt-in; every existing CI step is unchanged. `wcet-trip`'s new CI step is the first consumer.

**Two clippy errors caught in fixture-gated code that CI would never have linted.** `LE-12` says CI never lints target-only fixture code, and that is exactly what happened: `cargo clippy -p kernel --bins --features fixture-wcet-restart --target ...` found `int_plus_one` and `manual_is_multiple_of` that no command in CI runs. **If you add a fixture, lint it with its feature enabled** — the mandate's per-binary command plus `--features <the fixture>`. This is a concrete, cheap mitigation for `LE-12` and it should probably become a CI matrix step.

## Loose-ends delta

**Closed:** `LE-02` (WCET had no timer or watchdog). Both halves now exist and both run on the real timer.

**New — `LE-22`: degrade and priority inheritance have not been reconciled.** `PriorityInheritingLock` records a holder's pre-boost priority and restores it on unlock. So a boosted holder that is degraded mid-hold has its degrade **silently undone** by the subsequent unlock; and degrading a boosted task discards a boost a high-priority waiter is depending on, reintroducing the very inversion `STORY-P0-02-03` exists to prevent. No fixture combines the two, no test pins it, and neither module's doc mentions the other. Found by reading, not by a failure. Unowned.

**Still open and unchanged:** `LE-09` (no hardware tier — every timing claim still carries release-blocking hardware debt), `LE-12`, `LE-16` + `LE-18` (the timing gate — see below), `LE-19(b)`, `LE-03`, `LE-08`, `LE-10`, `LE-11`, `LE-15`.

**`LE-20` is now the oldest thing of its shape.** Neither preemption *nor* enforcement runs on the shipping `os` image; `os` installs no tick hook. Two Stories have now shipped whose mechanism is proven only in fixtures. It should not become three.

## What I did not do, and why

**The timing gate (`LE-16` + `LE-18`) is untouched.** Handover 10 called it the highest-value unowned work and it still is. I judged the Story the mandate named as "start here" to be the mandate's actual instruction, and the gate work needs a Story document and a Test document with a deliberately-injected regression — that is a session's work, not a tail-end task. It remains unowned and unscheduled, and the diagnosis in Handover 10 §"The one regression this session leaves behind" is still the best statement of it. **Read that section before touching the gate**, in particular the caution that re-recording baselines without fixing the methodology first is not an exemption from Handover 05's rule.

`check-timing-regression` was **not** run locally this session and is not evidence for this Story either way: `fixture_measure` installs a fault-only IDT and arms no timer, so none of the gated paths take a tick at all. The enforcement path's own cost is therefore unmeasured — that is `D03`'s to measure, and `D03` still has no baseline.

## Where to start next

1. **`LE-20` — put the tick hook on the real boot path.** Small, and it is the only thing standing between two Verified Features and a system that actually does what they claim. Both mechanisms are proven and both are inert in the shipping image.
2. **The timing gate (`LE-16` + `LE-18`)**, as a fourth Story under `FEAT-P1-01`. Highest-value unowned work in the register; `main`'s CI is red on it.
3. **`LE-22`**, which is cheap to at least *document* in both modules' doc comments even if reconciling it is a Story.
4. **`FEAT-P1-05` / `FEAT-P1-06`** — the two proof Features; `-06` is `EPIC-P1`'s flagship exit.
5. **`FEAT-P9-01`'s two Stories** — independent of all the above, no hardware precondition.

## State of the tree

Everything committed on `feat/p1-04-02-wcet-enforcement` and merged to `main` with `--no-ff`, so the branch stays visible as a unit. **Not pushed** — two commits are ahead of `origin/main`. Push them before relying on CI for anything, including the timing-gate diagnosis in Handover 10.

Verification at the close, all green: 395 host tests (383 before; 12 added), `cargo fmt --all -- --check`, `cargo clippy --workspace --lib --tests -- -D warnings`, the per-binary target clippy for `kernel`/`exec`/`os`, **and** that command again with each of the three new fixture features. `check-assurance-spine` (22 Features / 47 Stories / 34 Tests / **41** Reports), `check-crate-sizes`, `check-image-size` (`os`, 74,568 bytes — unchanged), `check-performance-catalogue`.

Every Tier 0 fixture re-run. All exit 0 except `broken-boot`, `idt-apic-unrouted` and the new `wcet-trip`, each of whose documented pass condition is a distinguishable failure.

## Standing constraints — unchanged, do not relax

- **TDD.** Test document when a Story starts. Note the process deviation this session recorded in `TEST-P1-04-02-A`: the host seams were written Green-first with a recorded falsification in place of a recorded Red. That is weaker than the rule and is written down as such rather than smoothed over. Do not treat it as the new normal.
- **Tier 0 is not hardware evidence.** `LE-09` remains open; every timing claim carries release-blocking hardware debt. `D03` still has no measured baseline even after this Story — enforcement being *correct* and enforcement latency being *bounded in real time* are different claims, and only the first is made.
- **Never call TinyOS a hobby OS.** It targets data-centre, local-AI, UAV, medical, edge and consumer deployment.
- **No assurance state may be claimed beyond its evidence.** Every Verified Story is still `baseline-debt`. The first `baseline-debt → verified` conversion is `EPIC-P1`'s explicit charge and has not happened.
- **When sweeping fixtures from PowerShell, pass arguments literally rather than splatting.** And note a second one this session: PowerShell mangles `-Zbuild-std=core,compiler_builtins` on the comma. Run the target-clippy command from bash.
