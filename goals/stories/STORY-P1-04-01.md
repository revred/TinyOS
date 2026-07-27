# STORY-P1-04-01 — Timer-Driven Preemption

Status: **Verified (Tier 0 + Host) — assurance `baseline-debt`**
Feature: [`FEAT-P1-04`](../features/FEAT-P1-04.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)
Started: 2026-07-28 · Verified: 2026-07-28 ([`REPORT-2026-07-28-03`](../reports/REPORT-2026-07-28-03.md))

## Description

The armed-but-unconsumed local-APIC timer finally gets its consumer: the tick ISR invokes a preemption decision (highest-priority-ready wins, per the existing `Scheduler` ordering), performing an interrupt-driven context switch instead of waiting for cooperative yield — converting `kernel::dispatch` from cooperative-only to genuinely preemptive. The priority-inheriting lock's behavioral half (boost actually preventing inversion under real preemption) becomes testable and tested here, closing `STORY-P0-02-03`'s host-only caveat.

## Depends on

`STORY-P1-02-01` (a preemption path that faults lands in a real handler); `STORY-P1-01-01` (tick-to-dispatch latency is measured, D03/D05).

## Acceptance criteria (finalized 2026-07-28 at Story start)

1. **Preemption is real.** A busy-looping low-priority task that contains no yield of any kind is suspended by the timer ISR in favour of a task that became Ready while it was running, within a **measured, bounded number of ticks**. The bound is asserted by the fixture, not read off afterwards.

2. **Extended state survives preemption (`LE-14`, in scope here — see below).** `kernel::context::switch` saves callee-saved integer registers and flags only. Cooperative switching is sound without more because the SysV ABI makes every XMM register caller-saved, so a task that *calls* `switch` has already spilled anything live. A timer interrupt can suspend anywhere, including between two halves of an SSE computation, and `hal_x86_64::boot` deliberately enables SSE (ADR 0003) — so preemption without extended-state save/restore is **silent data corruption, not a fault**. The preemption path must therefore save and restore x87/SSE state per task, and the fixture must prove it adversarially: the preempted task's XMM contents must survive across a preemption in which the preempting task writes a *different* value into the same register.

3. **Priority inversion is demonstrably avoided under real preemption.** The classic three-task scenario, run for real: low holds the lock, high preempts and blocks on it, medium is Ready and busy-spinning. High must proceed because low is boosted above medium — proven by the *dispatch order actually taken* and by medium's own progress counter not advancing while the boosted holder runs. This closes `STORY-P0-02-03`'s host-only caveat.

4. **Interrupt-context discipline.** The ISR-side work is bounded and allocation-free, per `agent/CODING_STANDARDS.md`'s RT rules: a fixed-size scheduler read, one pure decision, and — only when the decision says preempt — one `fxsave` plus one register swap. No allocation, no unbounded iteration, no serial I/O on the tick path.

5. **The re-entrancy rule is stated and enforced, not assumed.** The dispatcher holds `&mut Scheduler`; the ISR reads the same scheduler. The rule is that **interrupts are enabled only while a task runs** — the dispatcher body runs with `IF` clear and a task's own saved `RFLAGS` re-enables them across the switch — so the ISR can never observe a scheduler the dispatcher is mid-mutation of. Task code that itself touches the scheduler does so inside a real interrupt-free critical section. Both halves are Tier 0 mechanisms, not conventions.

### `LE-14` is in this Story's scope, explicitly

The next-session mandate required this to be scoped or split, never implied. It is **in scope here**: criterion 2 is an acceptance criterion of this Story, not a follow-up. Preemption that can corrupt a task's floating-point state is not preemption this project would ship, and shipping it with a named follow-up would be claiming an assurance state beyond its evidence.

## Explicitly out of scope

- **WCET enforcement on the real timer** — `record_tick` driven by real ticks, overrun → declared fault policy. That is [`STORY-P1-04-02`](STORY-P1-04-02.md), and `LE-02` stays open until it lands.
- **Equal-priority time-slicing / round-robin.** The rule implemented here is strictly-higher-priority preemption. Two Ready tasks at the same priority do not rotate on a tick; that is a scheduling *policy* addition, and inventing one this Story has no requirement for would be speculative.
- **SMP.** Single CPU throughout; the tick hook and the interrupt-free critical section are both single-CPU arguments and say so.
- **`XSAVE`/AVX state.** `FXSAVE`/`FXRSTOR` covers x87, MMX and XMM0–15, which is exactly what this kernel's own code generation uses (`CR4.OSFXSR` is set by `boot.rs`; `OSXSAVE` is not). A build that enables AVX would need `XSAVE` and a wider area — named here rather than silently assumed away.

## Tests

[`TEST-P1-04-01-A`](../tests/TEST-P1-04-01-A.md) — host unit tests for the pure seams (the preemption decision table, the extended-state save/restore round trip, the interrupt-free critical section's flag arithmetic) plus two Tier 0 QEMU fixtures (`--fixture=preempt`, `--fixture=priority-inversion`).

## Goals verified

G-RT-1 (preemption + inversion avoidance, behaviorally).
