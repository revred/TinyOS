# STORY-P1-06-01 — Bounded Decision-to-Actuation Path, Scheduler-Enforced

Status: **Verified (Tier 0, mechanism half), 2026-07-29** — assurance state `baseline-debt`. The *bound* half of the Feature's exit criteria is **not** claimed and is stated debt against `LE-09`; the *under-hostile-load* half waits on `FEAT-P1-05`
Feature: [`FEAT-P1-06`](../features/FEAT-P1-06.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)
Started: 2026-07-29

## Description

One RT task computes an actuation command and emits it to a bounded output primitive (under Tier 0, a
real ISA port write standing in for an actuator line); the task's WCET budget **and deadline** are
declared, the deadline monitor enforces the deadline, the WCET enforcement (`STORY-P1-04-02`) enforces
the budget, and the decision-to-actuation latency distribution is measured end to end. `G-PA-1`'s own
wording is the bar: the bound is *enforced by the scheduler, not merely observed in testing* — so a
deliberate overrun tripping the declared policy is part of the proof, not an appendix to it.

## What this Story claims, and the two halves it does not

`FEAT-P1-06`'s exit criteria have three halves gated by three different things. This Story takes one:

| Half | Gate | Taken here? |
|---|---|---|
| Mechanism + **enforcement firing** + distribution recorded | nothing | **Yes** |
| The same distribution **under hostile load** | `FEAT-P1-05`, `Specified — no Story started` | No |
| `PERF-D03-G04`/`PERF-D05-G04` — the **bound** | hardware **and** a qualification record | No |

**Ordering buys nothing.** Building `FEAT-P1-05` first — an entire adversarial Feature — would delay the
mechanism proof by a Feature and would **not** unlock the bound, because the bound is gated by
[`ADR 0005`](../../docs/adr/0005-arm64-real-time-tier-is-conditional-on-secure-world-qualification.md)
and hardware, not by load. Both deferred halves stay deferred either way, so the mechanism proof is the
largest increment actually available. The Feature therefore stays open on its own terms and this Story
does not pretend to close it.

**No `G04` row is filed, and none could be.** `xtask`'s `bound_provenance` check refuses a `G04` sourced
from Tier 0, and it is right to: zero platforms hold a secure-world qualification record. The measured
distribution is recorded as Tier 0 *mechanism* evidence with its margin; the bound is **stated debt
against `LE-09`**.

## The deadline is not the WCET budget

`STORY-P1-04-02` built budget enforcement and scoped the other quantity out honestly: *"A declared
deadline is a different quantity from a declared execution budget, and this Story enforces the latter."*
`FEAT-P1-04`'s title names both; only one existed. This Story builds the other:

| Quantity | Counts | Advances while the task is descheduled? | Enforced by |
|---|---|---|---|
| `WcetBudgetTicks` | ticks *attributed* to the task | **no** | `wcet::account_tick` → declared `OverrunPolicy` |
| `DeadlineTicks` | ticks since the activation was armed | **yes** | `actuation::ActuationPort::on_tick` → the emit is refused |

A task that is preempted and starved meets its budget perfectly and misses its deadline badly. That
divergence is why this is a separate mechanism rather than a second reading of the budget counter, and
it is pinned by a host test rather than left as an argument.

## Depends on

`STORY-P1-04-02` (the WCET enforcement the overrun trips), `STORY-P1-01-01`/`STORY-P1-01-02` (the
measurement harness and the UART-borne verdict), `STORY-P1-04-01` (timer-driven preemption and the tick
hook the deadline monitor is driven from). `STORY-P1-05-01` gates the under-load half and **is not
started**; this Story is startable and finishable without it, and cannot make the Feature Verified.

## Acceptance criteria (finalized 2026-07-29 at Story start)

1. **The path exists and is measured.** 1,000 sampled decision-to-actuation iterations, each arming an
   activation, computing a command and emitting it through `hal::actuation::OutputLine`, with
   p50/p99/p99.9/max recorded in a `TINYOS-MEAS/2` envelope carrying `tier`, `platform` and
   `qualification`. Every sample is a command that actually reached the line, on a write count the
   fixture keeps **independently of the port's own bookkeeping**.

2. **A late command is prevented, not logged.** An emit presented after the declared deadline has passed
   is refused and the line is not written — asserted against that independent write count, which is the
   only place "the actuator moved" can be observed.

3. **Only the declared actuation task can reach the output primitive.** The port is declared over one
   owner and has no setter for it. An emit presented with another task's real `TaskId` is refused, the
   line is untouched, and the authority check runs *before* the deadline state is consulted — so a
   refusal cannot be used as an oracle for the RT task's timing state.

4. **The enforcement is shown firing.** A deliberately-overrunning decision trips the `TripToSafeState`
   it declared, within a tick bound fixed in the Test document, and **no command reaches the line**. A
   clean run does not stand in for a demonstrated trip.

5. **Every actuation decision is a spoor**, with emitted, refused-unauthorized and refused-late
   distinguishable, and no arm of the emit path silent.

6. **The Report names Tier 1/Tier 2 hardware measurement as explicit open debt**, in `LE-09`'s terms:
   this Story proves the mechanism under QEMU; the boards prove the product's numbers.

7. **Both fixtures are shown to detect the defects they exist to catch** — one falsification per claim,
   per `ADR 0005` and `STORY-P0-01-07` clause 2.

## Named debt this Story leaves open

- **The bound itself (`LE-09`, `ADR 0005`).** No `PERF-D03-G04`/`PERF-D05-G04`, and no
  `guardrail-evidence.tsv` row of any kind — including `G20`, whose denial-cost gate this Story *does*
  measure but whose Tier 0 run-to-run p99 variance (20–48%) is too wide to file as evidence for a
  threshold. Measuring something is not the same as being able to stand behind a number for it.
- **Hostile load.** `FEAT-P1-05` has no Story started. The Feature stays open.
- **The shipping image.** The path is proven in a fixture, not on the real boot path — the same
  "proven in a fixture" shape `LE-20` already tracks for WCET enforcement, and this Story does not
  narrow it.
- **One address space.** Composing with `FEAT-P1-03` (actuation from a task in its own address space) is
  worth demonstrating and is deliberately not gated on, per the Feature's own `Depends on`.
- **No periodic activation model.** There is still no notion of a task *period* in this scheduler, so a
  deadline here is per-activation and armed explicitly.

## Tests

[`TEST-P1-06-01-A`](../tests/TEST-P1-06-01-A.md) — written before implementation, per the TDD mandate.
Host unit tests for `kernel::actuation`, plus two Tier 0 fixtures (`actuation`, `actuation-overrun`)
sharing one source file.

## Reports

- [`REPORT-2026-07-29-02`](../reports/REPORT-2026-07-29-02.md)

## Goals verified

G-PA-1 (mechanism and enforcement half); the primitive `G-PA-8`'s CNC flagship milestone later builds
on. G-RT-3's deadline half, alongside `STORY-P1-04-02`'s budget half.
