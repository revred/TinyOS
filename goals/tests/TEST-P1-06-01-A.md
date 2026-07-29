# TEST-P1-06-01-A — The Bounded Decision-to-Actuation Path: Enforced, Measured, and Shown to Refuse

Status: **Verified (Tier 0, mechanism half)** — specification written before implementation, per the TDD mandate
Story: [`STORY-P1-06-01`](../stories/STORY-P1-06-01.md)
Tier: Tier 0 QEMU run of two new fixtures (`actuation`, `actuation-overrun`) sharing one source file, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix), plus host unit tests for `kernel::actuation`
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D03`, `D05`
Security controls: `SEC-14`, `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-15`, `BND-16`, `BND-17`
Protection Domain contracts: `PD-05`, `PD-07`, `PD-08`
Code admission gates: `RCG-12`, `RCG-13`
Assurance state: `baseline-debt`

## What this test is for, and the half it deliberately does not attempt

`FEAT-P1-06` is `EPIC-P1`'s integration exit and `G-PA-1`'s flagship path. Its exit criteria have three
halves gated by three different things, and **this test takes exactly one of them**:

| Half | Gate | Claimed here? |
|---|---|---|
| Mechanism + **enforcement firing** + distribution recorded | nothing | **Yes** |
| The same distribution **under hostile load** | `FEAT-P1-05`, `Specified — no Story started` | No |
| `PERF-D03-G04`/`PERF-D05-G04` — the **bound** | hardware **and** a qualification record | No |

The third is not deferred for convenience. Under [`ADR 0005`](../../docs/adr/0005-arm64-real-time-tier-is-conditional-on-secure-world-qualification.md)
a worst-case bound is quotable only from a platform holding a secure-world qualification record, and
**zero platforms hold one**. `xtask`'s `bound_provenance` check refuses a `G04` row sourced from Tier 0
and it is right to. So this test measures a distribution and records it as Tier 0 *mechanism* evidence;
the bound itself stays **stated debt against `LE-09`**, in those words, and no `G04` row is filed.

What is fully provable here is the thing `G-PA-1` actually asks for — *"enforced by the scheduler, not
merely observed in testing"*. Enforcement firing is a Tier 0 claim, and it is the claim below.

## The three quantities, and why the deadline is not the budget

`STORY-P1-04-02` built WCET budget enforcement and closed its own scope note honestly: *"Deadline
monitoring separate from WCET. A declared deadline is a different quantity from a declared execution
budget, and this Story enforces the latter."* `FEAT-P1-04`'s title names both; only one was built.

This Story builds the other, and the distinction is the reason it is a separate mechanism rather than a
second reading of `wcet::account_tick`:

| Quantity | Counts | Advances while the task is descheduled? | Enforced by |
|---|---|---|---|
| **WCET budget** (`WcetBudgetTicks`) | ticks *attributed* to the task | **No** — an unattributed tick is charged to nobody | `wcet::account_tick` → declared `OverrunPolicy` |
| **Deadline** (`DeadlineTicks`) | ticks elapsed since the activation was **armed** | **Yes** — wall time does not stop because a task lost the CPU | `actuation::ActuationPort::on_tick` → the emit is refused |

A task that is preempted and starved meets its budget perfectly and misses its deadline badly. That
divergence is the whole reason a deadline monitor exists, and clause 3 asserts it directly rather than
asserting two counters that happen to agree.

## The scenario

One source file, two fixtures, one constant apart — the shape [`fixture_wcet`](../../os/src/kernel/src/fixture_wcet.rs)
established for the three policy arms, and for the same reason: the claim is that the *same* path
produces the right outcome in both, because the *declaration* differed.

| Task | Priority | Budget | Deadline | Policy | What it does |
|---|---|---|---|---|---|
| `control` | 25 | clean: generous · overrun: **4 ticks** | **2 ticks** | `TripToSafeState` | arms its window, computes a command, emits it through the output line |
| `background` | 5 | generous | — | trip (never fires) | busy-increments a counter; **is the unauthorized caller identity clause 2 uses** |

`background` exists for two jobs at once: it is a real competitor the RT task must keep outranking, and
it is a live `TaskId` that is *not* the declared actuation task — so "no ambient path" can be attacked
with a real identity rather than a fabricated one.

## Specification

### 1. The path exists, end to end, and is measured

**Given** the `actuation` fixture booted under QEMU with the local-APIC timer armed,
**then** `control` completes at least 1,000 sampled decision-to-actuation iterations, each one:

- arming its deadline window,
- computing a command word from a deterministic decision function,
- emitting that word through `hal::actuation::OutputLine` — under Tier 0, a real `out` to the ISA
  POST port, which is the *measurable I/O-port write standing in for an actuator line* the Story names,
- with the cycle count from **the start of the decision to immediately after the line write returns**
  recorded through `kernel::measure`.

**And** the fixture emits a `TINYOS-MEAS/2` envelope carrying `p50`/`p99`/`p99.9`/`max` for that path,
with `tier=T0`, `platform=qemu-tcg-x86_64`, `qualification=none`. The provenance travels with the
number from the moment it exists; nothing downstream has to be trusted to add it.

**And** every sample is a command that actually reached the line: the fixture's own line wrapper counts
writes independently of the port's bookkeeping, and the two must agree exactly. A measured path that
emitted nothing is a timing figure for a function that returned early.

**What this clause explicitly does not claim.** The timed region runs with interrupts masked (it is a
bounded critical section over the port, and the alternative is a data race with the tick hook that
reads the same state), so **no timer tick lands inside a sample**. That is a stated property of the
measurement, not a hidden one: these percentiles are the cost of the path, not of the path plus an
arbitrary interrupt.

### 2. Only the declared actuation task can reach the output primitive

**Given** the same run,
**then** an emit presented with `background`'s real `TaskId` is **refused**, the output line is
**not written** — the wrapper's write count is unchanged across the whole denial phase — and the
refusal is stamped.

**And** the authority check is *first*: a caller that is not the declared task is refused before the
deadline state is consulted at all, so an unauthorized caller can neither actuate nor learn anything
about the window it was refused from.

**And** the declaration is immutable. `ActuationPort` is constructed with its owner and exposes no
setter; a host test pins that there is no path — safe or unsafe — from a live port to a different
owner. "No ambient path" that could be revoked by one `pub fn` is not a containment property.

**And** the denial path is measured too, on the same harness, so its cost is evidence rather than an
assumption (`PERF-D03-G20`'s shape: a denial must be *cheap* and must change *nothing*).

### 3. The deadline is a different quantity from the budget, and the monitor enforces it

**Given** a host-driven activation in which the armed task is **not** the task the ticks are attributed
to,
**then** the deadline window advances anyway and expires on schedule, while the armed task's consumed
WCET ticks stay at zero.

**And** expiry is exact: elapsed **equal to** the declared deadline is not yet a miss, and the tick that
takes elapsed *past* it is. Same discipline `wcet::record_tick` holds for budgets, stated here so the
two cannot drift into disagreeing about what "exceeded" means.

**And** the miss is stamped exactly once per activation, however many further ticks arrive.

### 4. A late command is *prevented*, not logged

**Given** an armed window that has expired,
**then** an emit by the declared owner is **refused**, the line is **not written**, and the refusal is
stamped with an outcome distinguishable from an unauthorized refusal.

This is the clause the Story states as *"late actuation is prevented, not logged"*, and it is asserted
against the line's own write count — the only place "the actuator moved" can be observed.

### 5. The enforcement fires, and the deliberate overrun trips the declared policy

**The positive control, and it is not optional.** `FEAT-P1-06`'s own exit criterion says *"the proof
must show the enforcement firing, not only clean runs"* — written 2026-07-26, two days before
`ADR 0005` said the same thing about `Q3` campaigns, and the third independent arrival at that rule in
this repository. **A clean run does not stand in for a demonstrated trip.**

**Given** the `actuation-overrun` fixture, identical but for `control`'s declared budget (4 ticks) and a
decision deliberately long enough to exceed it before emitting,
**then**, in order:

1. the deadline window **expires** while the decision is still running — the miss is detected by the
   monitor, not by the emit;
2. the WCET budget is crossed, and enforcement fires **no later than** the `budget + 1`-th attributed
   tick, a bound fixed in this document rather than read out of the capture;
3. the disposition is the `TripToSafeState` the task **declared**, checked against the declaration
   rather than against whatever came back;
4. the task is left `TaskState::Finished` by the kernel — the trip is a state change, not a returned
   value the fixture chose to act on;
5. **no command ever reaches the line.** The wrapper's write count is `0`. This is the claim.

**And** the port refuses one final emit attempted from the safe-state path with the *owner's own*
identity, because the owner is no longer `Running`. Late actuation is refused by the port even if the
task's code were somehow resumed — the prevention does not rest on the task never being scheduled again.

**And** the system enters its declared safe state: at Tier 0 a reported, fail-closed stop, so this
fixture's **correct** outcome is a QEMU `isa-debug-exit` **failure** code, exactly as `broken-boot`,
`idt-apic-unrouted` and `wcet-trip` already establish. A safe state that returned to the dispatcher
would not be a safe state.

### 6. Every decision is a spoor, and there is no silent arm

**Given** either run's spoor journal,
**then** every emit, every refusal and every deadline miss carries `Category::Actuation`, `Actor::Kernel`,
its verb and an outcome that distinguishes *emitted* from *refused-unauthorized* from
*refused-late*, with the acting task in `TARGET`.

**And** structurally: every arm of the emit path either writes the line or stamps a refusal, asserted
over the whole `ActuationError` enumeration so that an arm added later cannot fall through silently —
the same guard `wcet`'s own "no ignore branch" test holds.

### 7. The fixtures are shown to detect the defects they exist to catch

Per `ADR 0005` and `STORY-P0-01-07` clause 2: **an instrument never demonstrated to detect anything
cannot be believed when it reports a pass.** Two independent falsifications, because this test makes two
independent claims:

- **Authority.** With the owner check removed from `ActuationPort::emit`, the `actuation` fixture must
  **fail**, naming clause 2 — an unauthorized identity reaching a real actuator line.
- **Prevention.** With the expiry check removed, so an expired window still writes the line, the
  `actuation-overrun` fixture must **fail**, naming clause 4/5 — a late command emitted.

**And** each failure must name its clause on the serial line rather than reporting a bare non-zero exit.
For `actuation-overrun`, whose pass condition is *already* a failure exit code, that is not a nicety:
the exit code cannot distinguish a correct trip from a broken one, so the `TINYOS-RESULT/1` line is the
only thing that can, and it must be wrong when the kernel is wrong.

### 8. The run is bounded, and a failure is a failure rather than a hang

**Given** any defect in the above,
**then** the run ends by its own dispatcher-round bound or its own loop ceilings and reports a failing
`TINYOS-RESULT/1` line, rather than spinning until the harness kills it and leaves an empty capture —
the `LE-46` shape, refused here as `TEST-P1-04-05-A` clause 7 refused it.

## What this test explicitly does not establish

- **The bound.** No `PERF-D03-G04` or `PERF-D05-G04` row is filed, and none can be until a platform
  holds a secure-world qualification record (`ADR 0005`, `LE-09`). A QEMU/TCG distribution is the
  mechanism's proof; the boards' numbers are the product's.
- **The distribution under hostile load.** `FEAT-P1-05` has no Story started. The Feature cannot be
  Verified without it, and this test does not pretend otherwise — it is the Feature's own
  `Depends on` amendment of 2026-07-29, not a gap discovered here.
- **Actuation from a task in its own address space.** `FEAT-P1-03` composes with this and is
  deliberately not gated on (the Feature says so). One address space, one output, one bound.
- **Any timing baseline or gate.** This fixture records no `goals/performance/baselines/` entry and adds
  no `check-timing-regression` step. The ratio gate's reference loop belongs to `fixture-measure`;
  duplicating it here would create a second reference that could drift against the first, which is the
  one failure mode that gate's own doc comment warns about at length.
- **A periodic activation model.** There is no notion of a task *period* in this scheduler
  (`STORY-P1-04-02` said so and it is still true), so a deadline here is per-activation and is armed
  explicitly. Inventing a period model this Story has no requirement for would be speculative.

## Reports

- [`REPORT-2026-07-29-02`](../reports/REPORT-2026-07-29-02.md)
