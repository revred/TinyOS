# TEST-P1-04-02-A — WCET Enforcement on the Real Timer

Status: **Specification written at Story start (2026-07-28), before implementation — see the process note**
Story: [`STORY-P1-04-02`](../stories/STORY-P1-04-02.md)
Tier: Host unit tests (the policy decision table, the attribution rule) **plus** Tier 0 QEMU runs of three new fixtures, one per policy arm, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D03`
Security controls: `SEC-14`, `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-15`, `BND-16`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-08`
Code admission gates: `RCG-13`, `RCG-14`
Assurance state: `specified`

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
**then** a task deliberately exceeding its declared budget is detected within an asserted, bounded number of ticks, with the bound a constant fixed **in this document before the fixtures existed** — not a number read out of the capture. Detection happens on the tick that crosses the budget, which is `record_tick`'s existing contract and is re-checked here against real time rather than against a loop that calls it directly.

**Observed:**

> _To be filled from the Tier 0 captures._

### 5. Tier 0: each policy arm is observed happening, not merely decided

The failure mode this clause exists to prevent is a fixture that asserts the disposition function returned the right value and never checks the system did anything about it.

- **Restart** — the restarted task is observed executing **from its entry point again**: a counter it increments on entry advances a second time, and a value it had accumulated before the overrun is gone. "It was marked Ready" is not the claim.
- **Degrade** — the degraded task is observed **losing a selection it would previously have won**. A competitor at a priority between the task's original and its floor must be Ready throughout and must start being chosen after the degrade and not before. Without that competitor the arm is indistinguishable from "reset the budget and carry on", which is the failure this arm most easily rots into.
- **TripToSafeState** — the system is observed **stopping, with its reason reported**, and the fixture's pass condition is a distinguishable fail-closed exit rather than success — the precedent `broken-boot` and `idt-apic-unrouted` already set for fixtures whose correct outcome is a failure code.

**Observed:**

> _To be filled from the Tier 0 captures._

### 6. Tier 0: enforcement does not punish the innocent

**Given** an overrunning task and a within-budget task at RT priority running alongside it,
**then** the innocent task's own tick accounting is unaffected across the whole enforcement window: it is charged for the ticks it ran and no others, it does not overrun, and its own progress counter continues advancing across the detection and disposition. Measured on the same tick counter the enforcement uses, not asserted from the absence of a failure.

**Observed:**

> _To be filled from the Tier 0 captures._

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

## Process note: how strictly TDD was followed here

Clauses 1–9 were written **before any implementation code**, from the Story's finalized acceptance criteria — as with `TEST-P1-04-01-A`. The design decision that shaped clause 2 (that an overrun must not be routed through `kernel::fault::Disposition`) was made against the existing source *before* the Test document, and is recorded in the Story rather than discovered during implementation.

The pure seams — attribution and the policy table — are drivable Red-to-Green in the ordinary way. The three Tier 0 fixtures will not be, for the reason the previous Test document recorded: a fixture's first run against real interrupt behaviour is a debugging exercise, not a Red-to-Green cycle. What is held to instead is that the bounds and the observable consequences in clauses 4–6 are fixed here, before the fixtures exist.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/kernel/src/wcet.rs`) plus Tier 0 QEMU runs (`qemu-x86_64 --fixture=wcet-restart`, `--fixture=wcet-degrade`, `--fixture=wcet-trip`).

## Implementation location

_To be filled as the Story lands._ Expected: `os/src/kernel/src/wcet.rs` (`OverrunPolicy`, the disposition and the attribution rule), `os/src/kernel/src/sched.rs` (the per-task policy), three fixtures under `os/src/kernel/src/`, `os/src/xtask/src/main.rs` and `.github/workflows/ci.yml`.

## Reports

_To be filed when the Story completes._
