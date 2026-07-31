# TEST-P1-04-05-A — The Composed Scenario Under Real Preemption: A Degrade Taken While Boosted, Proved By Who Runs

Status: **Verified (Tier 0)** — specification written before implementation, per the TDD mandate
Story: [`STORY-P1-04-05`](../stories/STORY-P1-04-05.md)
Tier: Tier 0 QEMU run of one new fixture (`degrade-inheritance`), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix). The host-level composition algebra is `TEST-P1-04-04-A` and is not repeated here
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D03`, `D05`, `D06`
Security controls: `SEC-14`, `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-15`, `BND-16`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-08`
Code admission gates: `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

`LE-50`, and it is the second half of a claim whose first half is already made.

`STORY-P1-04-04` fixed `LE-22` — degrade and priority inheritance no longer collide, because the
quantity the scheduler reads is `max(base, inherited)`, derived on demand and stored nowhere. It
proved that **at the host level**, driving the overrun through `wcet::account_tick`, the entry point
the timer ISR calls. What it could not say is that the composition holds when the ticks are real, the
preemption is real, and the evidence is *who actually ran*.

That gap is exactly the one `STORY-P0-02-03` carried for the un-composed case until `STORY-P1-04-01`
closed it with `kernel::fixture_priority_inversion`. This test is the same closure for the composed
case, on the same template, and it exists as its own fixture rather than as an extension of that one
because extending it would alter evidence a Verified Story rests on.

**The distinction that makes this worth a fixture rather than another host test.** A host test asserts
priority *values*. This asserts *dispatch outcomes*: a medium-priority task that is genuinely `Ready`
throughout, demonstrably able to run, and which makes no progress at all across a window that contains
a WCET degrade. Under the pre-`STORY-P1-04-04` implementation that degrade would have discarded the
holder's boost and medium would have started winning selections immediately — so this fixture's
frozen counter is a claim the old kernel could not have satisfied.

## The scenario

| Task | Priority | Budget | Policy | What it does |
|---|---|---|---|---|
| `low` | 5 | 4 ticks | `Degrade(floor = 2)` | takes the lock, releases the other two, then works while holding it — **no yield of any kind**, so what takes it off the CPU is always the timer |
| `high` | 25 | generous | trip (never fires) | preempts `low`, contends for the lock (boosting `low` to 25), blocks; resumes once `low` releases |
| `medium` | 15 | generous | trip (never fires) | busy-increments a counter whenever it is selected; touches neither the lock nor any other task |

`medium` sits **strictly between** `low`'s own priority (5) and the boost (25), and strictly above
`low`'s declared floor (2). That placement is the whole design: it is the task that must not run
during the window, and the task that must run after it.

## Specification

### 1. The composed scenario forms at all, under real ticks

**Given** the fixture booted under QEMU with the local-APIC timer armed,
**then** `low` acquires the lock, `high` contends for it and is refused with
`LockError::AlreadyLocked`, `low`'s effective priority becomes **25**, `low` is taken off the CPU by a
timer preemption at least once, `low` overruns its declared budget and is degraded, `low` releases the
lock, and `high` runs to completion.

**And** every enforcement the tick hook observes names `low` and carries the disposition `low`'s
*declared* policy maps to — `DegradeTo(2)`, checked against the declaration rather than against
whatever came back.

**And** `high` had already contended at the moment the degrade fired. A degrade that landed *before*
the boost would be a different scenario that happened to pass, so it is asserted rather than assumed.

### 2. The degrade lands, and the boost survives it — observed from inside the ISR

**Given** the tick on which `wcet::account_tick` applies the degrade,
**then** at that instant, read from interrupt context:

- `low`'s **base** priority is **2** — the declared floor. The enforcement decision really happened.
- `low`'s **effective** priority is still **25** — the waiter is still blocked and still needs it.

Both, at the same instant, in the real enforcement path. This is `LE-22`'s boost-then-degrade half
with the timer driving it.

### 3. The behavioural claim: `medium` makes no progress across a window containing the degrade

**Given** the window that opens when `high` blocks and closes when `low` releases the lock,
**then** `medium`'s counter is **identical** at three points inside it — when `high` blocks, when the
degrade fires, and when `low` unlocks.

**And** `medium` is `Ready` at all three of those points, checked at each. A frozen counter for a task
that was never runnable proves nothing at all, and that is precisely how this fixture could have been
written to pass for the wrong reason.

**And** the dispatcher's own selection log shows it: `medium`'s slot appears **nowhere** before the
release, so the claim does not rest on a counter alone.

### 4. `low` leaves the lock at its floor, and the dispatcher acts on it

**Given** `low`'s unlock,
**then** its effective priority is **2 — the floor it was degraded to, not the 5 it held before the
overrun.** The degrade survived the release. This is `LE-22`'s degrade-then-unlock half.

**And** the consequence is observable rather than merely recorded: after the release, `low` remains
`Ready` and busy — it does not retire — and **its slot never appears in the selection log again.** It
is outranked by `medium` at 15 for the rest of the run, which is what being at 2 rather than 5 means
in this kernel.

**And** `medium` then makes real progress, past a floor fixed in advance, proving it was a genuine
competitor throughout rather than an inert task whose frozen counter meant nothing.

### 5. The audit trail says what happened

**Given** the spoor journal at the end of the run,
**then** it carries a `Lock`/`Boost` at 25, at least one `Wcet`/`Degrade`, and a `Lock`/`Restore`
whose recorded cost is **2** — the priority the task actually landed on, not the one it left. An
audit trail that named 5 here would be reporting a priority the task never held.

### 6. The fixture is shown to detect the defect it exists to catch

Per `ADR 0005` and `STORY-P0-01-07` clause 2: **an instrument never demonstrated to detect anything
cannot be believed when it reports a pass.**

**Given** the composition reverted in `kernel::sched` — `set_base_priority` also clearing the
inherited priority, which is exactly the old single-field collision `LE-22` describes —
**then** this fixture must **fail**, and its serial output must name which clause failed rather than
reporting a bare non-zero exit.

**And** the failure must be the *behavioural* one, not only a priority-value mismatch: with the boost
discarded by the degrade, `low` drops to 2 while `high` is still blocked, `medium` at 15 starts
winning selections immediately, and clause 3's frozen counter breaks. That is the priority inversion
`G-RT-1` denies, reproduced under real preemption.

### 7. The run is bounded, and a failure is a failure rather than a hang

**Given** any defect in the above,
**then** the run ends by its own dispatcher-round bound or its own loop ceilings and reports a
failing `TOS64-RESULT/1` line, rather than spinning until the harness kills it. Every counter that
could run away has a ceiling a passing run never approaches, and reaching one is reported as a failed
run rather than as a timeout.

## What this test explicitly does not establish

- **Anything about two contended locks (`LE-49`).** One lock, one waiter. `release_inheritance` still
  clears inheritance outright, and this fixture cannot see that because it never holds two.
- **Anything about transitive inheritance.** `high` blocks on nothing further.
- **Any timing guardrail.** No `PERF-Dnn-Gnn` closes here. `D03`/`D05`/`D06` are selected because the
  composition governs dispatch and lock latency, not because this fixture measures either. It reports
  no `TOS64-MEAS/2` envelope and files no `guardrail-evidence.tsv` row.
- **That `LE-45` is explained.** The soak anomaly was a `priority-inversion` fixture returning a
  non-zero exit with **no serial capture** (`LE-46`). This fixture adds Tier 0 coverage of the
  neighbouring composed scenario; it does not recover a diagnostic that was never captured, and an
  unreproduced failure still cannot be attributed after the fact. `LE-45` and `LE-46` stay open on
  their own terms.
- **The host-level composition algebra.** That is `TEST-P1-04-04-A`, which remains the evidence for
  the fix itself. This test is the behavioural proof standing on it, not a replacement for it.

## Reports

- [`REPORT-2026-07-28-13`](../reports/REPORT-2026-07-28-13.md)
