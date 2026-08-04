# TEST-P1-07-04-A — A Tick Verified by Ratio, and the Counter Decision That Closes `LE-15`

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P1-07-04`](../stories/STORY-P1-07-04.md)
Tier: Host unit tests (`hal::time::conformance`, divisor arithmetic, GIC register encoding) **plus** a Tier 1 hardware run on a Raspberry Pi 5 executing the conformance suite against real registers, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D02`, `D03`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: `PERF-D02-G01`, `PERF-D03-G01`, `PERF-D03-G05` — interrupt entry latency, tick accounting latency and its jitter envelope. This Story makes them measurable on this board; `STORY-P1-07-06` is where anything is measured.

## What this test is for

Two questions that share one set of registers.

**Does a tick fire at the interval it was told to?** Verified the way `kernel::fixture_idt_apic_timer` already verifies it — by ratio between consecutive intervals, never by absolute value. On a first hardware bring-up an absolute-value check conflates a wrong divisor, a wrong clock source and a wrong frequency register into one indistinguishable failure.

**Which counter do microbenchmarks read?** `STORY-P1-01-03` raised this as `LE-15` and deferred it until a board existed. The arithmetic that forces the answer: at 54 MHz one `CNTVCT_EL0` tick is ~18.5 ns, and a Cortex-A76 at ~2.4 GHz retires roughly **44 cycles per tick**. `D05/dispatch_select` measures a Tier 0 p50 of ~168 cycles — **under four ticks**. That is quantisation noise, not a measurement, and it is the same failure `LE-24` already documents for `D07` arriving on a second axis.

**The decision is recorded** (user confirmation, [Handover 19](../../session/hand-2026-07-28/19-feat-p1-07-acceptance-and-spine.md) §7.5): `PMCCNTR_EL0` becomes the ARM64 `CycleSource` for microbenchmarks; `CNTVCT_EL0` stays the `Timebase`/wall-clock source. The `hal::time` seam already separates the two roles. This test is where the decision meets registers that may refuse it.

## Specification

### 1. The tick is verified by ratio

**Given** a periodic timer programmed to a declared interval,
**when** consecutive intervals are captured,
**then** their ratio is within tolerance of 1, and the *declared* interval is reported alongside without being the pass condition.

**And** a run in which every interval is uniformly wrong by the same factor **passes clause 1 and must**: that is a divisor or frequency error, it is real, and clause 4 is where it is caught. Conflating the two into one absolute check makes both invisible.

### 2. `hal::time::conformance` runs against real registers (`LE-27`)

**Given** the shared conformance suite `STORY-P1-01-03` already passes on the host,
**when** it runs on the board driving the real `CNTVCT_EL0` and `CNTFRQ_EL0`,
**then** it passes unchanged.

**And `LE-27` closes here and only here.** Passing on the host is evidence about arithmetic. The two `mrs` instructions have been compiled since 27 July and never executed; this clause is the first time anything in this repository has read an AArch64 system register in anger.

### 3. `PMCCNTR_EL0` is enabled, or the fallback is taken and named

**Given** the PMU,
**when** `PMCR_EL0`, `PMCNTENSET_EL0` and `PMUSERENR_EL0` are programmed (and `MDCR_EL2` trap configuration is accounted for if the firmware left us at `EL2`),
**then** `PMCCNTR_EL0` advances monotonically and at a rate consistent with the CPU clock.

**And if it traps or reads zero, this Story does not fail.** It falls back to `CNTVCT_EL0` with batched iteration and records a **narrowed** `LE-15` naming exactly which register was unavailable, at which exception level, and why. A bring-up Story that can only succeed is a bring-up Story that will be made to look successful — and a trapping PMU is a finding about this board that is worth more than a green tick.

**And** the fallback is exercised deliberately at least once (by not enabling the PMU) so that the fallback path itself is tested rather than assumed.

### 4. Measured resolution is recorded, not quoted from the manual

**Given** both counters,
**then** the Report states the **measured** tick rate of each on this board — `CNTFRQ_EL0`'s claim checked against elapsed wall time, and `PMCCNTR_EL0`'s rate checked against `CNTVCT_EL0` — rather than the architectural value.

**And** this is the clause that catches the uniform-factor error clause 1 deliberately lets through.

### 5. Honest absence survives silicon

**Given** firmware that may genuinely not have programmed `CNTFRQ_EL0`,
**then** `GenericTimerTimebase` returns `None` for zero, for sub-MHz and for absurdly high frequencies, exactly as `STORY-P1-01-03` specified — and the board's actual value is recorded whatever it is.

**And** a `None` here is a *pass* for the code and a finding about the firmware. Substituting a plausible constant at this point would silently invent the denominator of every subsequent number.

### 6. Tick storms cannot starve the reporter (`SEC-20`)

**Given** the timer interrupt routed through the GIC,
**then** the handler is bounded, allocates nothing, and a tick arriving during fault reporting cannot preempt or corrupt the report.

### 7. What this test explicitly does **not** establish

- **No preemption, no scheduler, no WCET enforcement.** A tick that fires is not a scheduler; the `FEAT-P1-04` port is a follow-on Feature.
- **No device-IRQ routing beyond the timer.** Only the timer interrupt is routed; the AArch64 analogue of `LE-08` is untouched.
- **No measurement.** `STORY-P1-07-06` measures; this Story chooses the ruler.
- **`LE-09` stays open. `LE-15` closes here; `LE-24` does not** — it closes on `STORY-P1-07-06`'s batched shape.

### Amended 2026-08-04, at implementation — how the clauses meet this bench

Recorded rather than silently absorbed. Nothing above is weakened; the channel and two
mechanics are pinned down:

1. **The evidence channel is the canvas** (`LE-47`: five zero-byte serial captures, loopback
   owner-declared infeasible). Clause 1's ratio evidence is the live `TOS64-TICK/1` line —
   `count=`, the declared `tval=540000` (10 ms at the expected 54 MHz, reported and not the
   pass condition), and `rmin=`/`rmax=` in per-mille over the last eight intervals — repainted
   every second by the park loop, so the owner transcribes an *accumulated* verdict, not one
   sample. Clauses 2, 4 and 5 land as the boot-time `TOS64-CONF/1` line (conformance outcome
   with its span, the raw `CNTFRQ_EL0` beside the `cpus=` judgement); clause 3 as
   `TOS64-PMU/1` (the `PMCCNTR_EL0` advance across a generic-timer-measured ~10 ms window,
   the cross-counter `rate=` in MHz, and the `source=` decision). A refused GIC or timer
   enable pins the tick row to `TOS64-TICK/1 refused=<register> readback=<hex>` — a dead tick
   is a diagnosis, never a hang.
2. **The one resumable vector.** Slot 5 (`cur_el_spx/irq`) is the single entry in the table
   with a save/restore/`eret` path; the other fifteen keep `STORY-P1-07-02`'s fail-closed
   report. Clause 6 holds architecturally: exception entry sets `PSTATE.I` and the fault path
   never clears it, so a tick cannot preempt fault reporting; the handler is one `IAR` read,
   at most one interval record, one `TVAL` re-arm and one `EOIR` write — bounded,
   allocation-free, loop-free.
3. **The fallback is exercised on the host**, per clause 3's "tested rather than assumed":
   `cycle_source_decision` is a pure function and its `cntvct-fallback` arm is a host test,
   alongside the `EL2` half of the same clause — `MDCR_EL2` is now written to zero at the
   drop, so "PMCCNTR reads zero" can no longer be caused by an unknowable trap configuration.

### The board captures (to be quoted verbatim when the Tier 1 run happens)

Pending. The transcribed `TOS64-TICK/1` (with its ratio bounds), `TOS64-CONF/1` (the run
`LE-27` closes on) and `TOS64-PMU/1` (the run `LE-15` closes on) land here, via the
ground-truth file's `===== BOARD VERDICT N =====` record first.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`, `os/src/hal/src/time.rs` conformance) plus a Tier 1 hardware run.

## Implementation location

- `os/src/hal-arm64/` — GIC distributor/redistributor/CPU interface, generic-timer programming, PMU enablement, the `PMCCNTR_EL0` `CycleSource`.
- `os/src/kernel/` — the tick-ratio and conformance fixture.

## Reports

To be filed when the Story goes Green.
