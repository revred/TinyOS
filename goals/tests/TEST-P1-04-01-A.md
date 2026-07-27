# TEST-P1-04-01-A — Timer-Driven Preemption, Extended State, and Inversion Avoidance

Status: **Verified (Tier 0 + Host)** — specification written at Story start, before implementation; see the process note
Story: [`STORY-P1-04-01`](../stories/STORY-P1-04-01.md)
Tier: Host unit tests (the preemption decision table, the extended-state round trip, the critical-section rule) **plus** Tier 0 QEMU runs of two new fixtures, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D03`, `D05`, `D06`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-15`, `BND-16`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-08`
Code admission gates: `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

Every dispatch this kernel had ever performed was cooperative: the running task called `switch` itself. This test is the evidence that a task which never yields can be taken off the CPU anyway, that the state it was using survives the experience, and that the priority-inheriting lock built in `STORY-P0-02-03` actually prevents inversion when a real medium-priority task is really competing — the claim that Story explicitly could not make.

## Specification

### 1. The preemption decision is total and pure

**Given** `kernel::preempt::tick_outcome(running, best_ready)`,
**then** it answers exactly one of three ways and every arm is pinned by a host test:

| `running` | `best_ready` | outcome |
|---|---|---|
| `None` | anything | `NoRunningTask` — a tick that lands in the dispatcher or the idle context never switches |
| `Some(t, p)` | `None` | `Continue` |
| `Some(t, p)` | `Some(r, q)` where `q > p` | `Preempt(r)` |
| `Some(t, p)` | `Some(r, q)` where `q <= p` | `Continue` |

The boundary that matters is `q == p`: **equal priority does not preempt.** A tick-driven rotation between equal-priority tasks is a scheduling policy this Story has no requirement for, and implementing one silently would change dispatch behaviour nothing asked for. Pinned by its own test, so a later Story that *does* want round-robin has to change a test rather than discover the behaviour.

**And** the candidate `tick_outcome` is asked about is the one `Scheduler::highest_priority_ready` itself selects — read through that same function, never re-derived by a parallel iteration that could drift from it. A decision about a task the dispatcher would not then choose is worse than no decision. Asserted against a real `Scheduler` rather than hand-built tuples, because the failure being guarded is precisely a *drift between two selections*, which a pure-function test over invented inputs could never see.

### 2. Extended state round-trips, and a fresh area is architecturally valid

**Given** `hal_x86_64::extended_state::ExtendedState`,
**then** it is exactly 512 bytes, 16-byte aligned (`FXSAVE`'s architectural requirement — a misaligned area is `#GP`, and the type system is where that should be settled). A host test writes a known 128-bit pattern into `XMM0`, saves, overwrites `XMM0` with a different pattern, restores, and reads `XMM0` back: it must hold the original. This runs on the host toolchain because `FXSAVE`/`FXRSTOR` are ordinary x86-64 user-mode instructions — the mechanism is provable without QEMU, and only the *scheduling* half needs Tier 0.

**And** `ExtendedState::new()` is not merely zeroed. A zeroed area has `MXCSR == 0`, which is a legal encoding that **unmasks every SIMD floating-point exception** — restoring it would arm `#XM` on the next inexact result. `new()` therefore writes the architectural defaults (`FCW == 0x037F`, `MXCSR == 0x1F80`), and a host test asserts both fields, so a never-saved area is well-defined rather than quietly hostile.

### 3. The interrupt-free critical section restores what was there

**Given** `hal_x86_64::rflags::should_reenable`,
**then** the pure half — deciding whether to re-enable on the way out, from the `RFLAGS` value saved on the way in — is a host-tested function over a `u64`, so the restore rule is checked without a CPU: a section entered with `IF` set re-enables on exit, one entered with `IF` already clear does not (a nested section must not silently enable interrupts its caller deliberately disabled). A third test walks all 64 bits and asserts only bit 9 can be read as the interrupt flag, so an implementation testing a neighbouring bit — `TF` at 8, `DF` at 10 are the easy slips — fails here rather than passing against realistic-looking flag values. The `cli`/`pushfq` half is exercised at Tier 0 by clause 6, whose scenario is unsound without it.

### 4. Tier 0: a task that never yields is preempted, within a bounded number of ticks

**Given** `cargo run -p xtask -- qemu-x86_64 --fixture=preempt`,
**then** the fixture arms the local-APIC timer, installs the preemption hook, and dispatches a low-priority task whose body is a busy loop containing **no `switch`, no `hlt`, and no scheduler call** — inspectable in `os/src/kernel/src/fixture_preempt.rs` as a plain counting loop. A high-priority task starts `Blocked`; the tick hook itself marks it `Ready` after a fixed tick count, which is the one event the low task cannot cooperate with.

**Then**: the high task runs; the low task's counter is non-zero (it really was running, not merely created); the number of ticks between "high became Ready" and "high first ran" is recorded and asserted `<= 2`. A bound is only evidence if it is asserted before the run, so it is a constant in the fixture, fixed by this document before the fixture existed.

**Observed** (`isa-debug-exit` success):

```text
fixture-preempt: preemptions=1 high_ready_tick=3 high_first_ran_tick=3 ticks_to_preempt=0 (bound 2)
fixture-preempt: low_iterations=1723324 resumed_at=1301806 exhausted=false retired_by_tick=true
fixture-preempt: xmm0 pattern=0x123456789abcdef clobber=0xfedcba9876543210 corrupted=false first_foreign_value=0x0 at_iteration=0
TINYOS-RESULT/1 fixture=preempt ok=true
```

`resumed_at=1301806` against `low_iterations=1723324` is the part worth reading twice: the victim was suspended after ~1.3M iterations, the preemptor ran, and the victim then **continued from where it was** for another ~420,000 iterations. "Preempted" is thereby distinguished from "killed and restarted", which the counter alone would not have separated. `retired_by_tick=true` records that it left the CPU the same way it was suspended — by an interrupt, never by yielding.

### 5. Tier 0: the preempted task's SSE state survives — adversarially, and not where it was first guarded

**Given** the same fixture,
**then** before entering its busy loop the low task loads a known 64-bit pattern into `XMM0` and re-reads it from `XMM0` (via inline assembly, so no compiler spill can stand in for the register) on **every** iteration, recording the first foreign value it sees and the iteration at which it saw it. The high-priority task, which runs in between, deliberately writes a different pattern into `XMM0`.

**This clause found a real defect, and the defect was in the placement.** The first implementation saved and restored extended state around the *context switch* on the preemption path — save when a tick decides to preempt, restore when the task is resumed. That is the obvious design and it is wrong, and this fixture's first run said so:

```text
fixture-preempt: xmm0 pattern=0x123456789abcdef clobber=0xfedcba9876543210 corrupted=true first_foreign_value=0x124df8
```

`0x124df8` is neither the victim's pattern nor the preemptor's — which is the whole diagnosis. An interrupt handler is *itself* ordinary compiled code running on the interrupted task's stack, and it may touch SSE registers whether or not it goes on to preempt anything. Guarding only the preempting ticks left every other tick free to corrupt the task it interrupted. The save/restore therefore moved into `hal_x86_64::interrupts`' timer ISR **stub**, wrapping the entire handler call, with the 512-byte area carved out of the interrupted stack — correct by construction rather than by an argument about what the handler happens to compile to, since nothing Rust can emit runs before the `fxsave` or after the `fxrstor`. It composes with a switch taken inside the handler for free: the area lives on the task's own stack, so it travels with the suspended task.

**And it was then deliberately falsified.** With the `fxsave`/`fxrstor` pair removed from that stub and nothing else changed:

```text
fixture-preempt: xmm0 pattern=0x123456789abcdef clobber=0xfedcba9876543210 corrupted=true first_foreign_value=0x124c18 at_iteration=495535
TINYOS-RESULT/1 fixture=preempt ok=false
```

Corruption at iteration 495,535 — mid-run, at a tick, not at iteration 1 — which is what distinguishes "an interrupt destroyed the register" from "the task's own compiled code did". A save/restore that has never been observed failing is a save/restore nobody has evidence for; this one has been.

### 6. Tier 0: priority inversion avoided, with a real medium task competing

**Given** `cargo run -p xtask -- qemu-x86_64 --fixture=priority-inversion`,
**then** the classic three-task scenario runs under genuine preemption:

- **low** (priority 5) takes the lock, makes medium and high `Ready`, and busy-works while holding it;
- **high** (priority 25) preempts low on a timer tick, fails to take the lock, is boosted-by-contention into low, marks itself `Blocked` and yields;
- **medium** (priority 15) is `Ready` throughout and busy-increments a counter whenever it runs;
- **low**, now boosted to 25, outranks medium, finishes its work, unlocks (restoring priority 5 and unblocking high);
- **high** runs to completion.

**Then** the dispatch order is recorded as a bounded run-log and asserted to begin `low → high → low → high`, and **medium's counter is unchanged across the window between high blocking and high resuming**. Both are required, and so is a third assertion that is easy to leave out: that medium **was `Ready` during that window and does run afterwards**. A frozen counter for a task that was never runnable would prove nothing at all, which is exactly how this test could have been written to pass for the wrong reason.

**And** every task-context scheduler mutation in this fixture happens inside `without_interrupts`, which is what makes the scenario sound rather than a race that happens to pass — clause 3's Tier 0 half.

**Observed** (`isa-debug-exit` success):

```text
fixture-inversion: acquired=true contended=true boost=Some(25) released=true priority_after_release=Some(5) high_completed=true
fixture-inversion: dispatch order=[0, 2, 0, 2, 1] (0=low 1=medium 2=high), preemptions=1, low_preempted=true
fixture-inversion: medium ready_in_window=true counter_at_block=0 counter_at_resume=0 counter_final=1000 (min 1000)
TINYOS-RESULT/1 fixture=priority-inversion ok=true
```

`low → high → low → high → medium` is the sequence inheritance is supposed to produce, and `0 → 0 → 1000` is the counter that says it did. Without the boost, low at priority 5 loses every selection to medium at 15: medium's counter would climb through the window and high would never run at all. `low_preempted=true` records that low reached its second turn because a *tick* took it off the CPU, not because it cooperated. This closes `STORY-P0-02-03`'s explicit host-only caveat, three Features after it was raised.

### 7. No regression in the cooperative path

**Given** the full Tier 0 fixture sweep and `cargo test --workspace`,
**then** `dispatch::run_once` and `run_once_in_space` are unchanged, every pre-existing fixture still passes with its own documented result (`broken-boot` and `idt-apic-unrouted` exit 1, as their pass conditions require; every other fixture exits 0), and `--fixture=os` behaves exactly as `TEST-P1-03-03-A` clause 6 recorded. Preemption is opt-in per binary: a build that never calls `set_tick_hook` ticks exactly as it did before this Story.

The one change every binary *does* inherit is the ISR stub's extended-state save (clause 5), which is a strict correctness improvement — before this Story, any timer tick could silently corrupt the SSE state of whatever it interrupted, cooperative scheduling or not.

### 8. What this test explicitly does **not** establish

- **No hardware tier.** Tier 0 QEMU only; `LE-09` open. The tick-to-preemption bound in clause 4 is counted in *ticks*, not cycles or microseconds, precisely so it is not mistaken for a latency figure — QEMU's APIC-timer-to-wall-clock relationship is not a number this project should build a budget on. `D03`'s real budget stays unmeasured until `STORY-P1-04-02` and a hardware tier.
- **No WCET enforcement.** `record_tick` is still not driven by the real timer and no overrun trips a policy; `LE-02` remains open and is `STORY-P1-04-02`'s to close.
- **No equal-priority rotation.** Clause 1 pins its absence deliberately.
- **`FXSAVE` only, not `XSAVE`.** x87/MMX/XMM0–15 are covered; AVX and wider state are not, and a build enabling them would need a wider area and `XSAVE`. Stated rather than assumed away.
- **Single CPU.** The interrupt-free critical section is a uniprocessor argument and nothing here is an SMP claim.
- **The shipping image does not preempt yet.** `os` installs no tick hook in this Story; the preemption path is proven in its own fixtures first, exactly as `run_once_in_space` was before `STORY-P1-03-03` moved it onto the real boot path.
- **One preemption, not many.** Each fixture demonstrates a single preemption in a scripted scenario. Sustained preemptive multitasking under load is not tested here.
- **The cost of the ISR-stub save is unmeasured.** `check-timing-regression` passes, but that says nothing about this change: `fixture_measure` installs a fault-only IDT and arms no timer, so no gated path takes a tick. Interrupt latency is `D03`'s to measure, and `D03` has no baseline yet.

## Process note: how strictly TDD was followed here

Clauses 1–8 were written **before any implementation code**, from the Story's finalized acceptance criteria — the first Test document in this Feature for which that is true without qualification. The pure seams (clauses 1–3) were then driven Red-to-Green in the ordinary way: each host test was written against a signature that did not yet have a body.

Clauses 4–6 could not be, and the honest statement of why is that a Tier 0 fixture's first run is a debugging exercise against real hardware behaviour — interrupt-delivery ordering, EOI placement, which stack the ISR lands on — not a Red-to-Green cycle. What was held to instead is that the *bounds and orderings they assert* were fixed in this document before the fixtures existed, so no number in clauses 4–6 was chosen after seeing what the hardware produced. The `<= 2` tick bound and the frozen-counter assertion are both from the pre-implementation draft.

That discipline is what caught clause 5's defect. The specification demanded an adversarial check written to fail if the mechanism were absent; the mechanism was present, in the obvious place, and the check failed anyway — which is the only reason the placement was found to be wrong before it shipped.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/kernel/src/preempt.rs`, `os/src/hal-x86_64/src/extended_state.rs`, `os/src/hal-x86_64/src/rflags.rs`) plus Tier 0 QEMU runs (`qemu-x86_64 --fixture=preempt`, `--fixture=priority-inversion`).

## Implementation location

- `os/src/kernel/src/preempt.rs` — `TickOutcome`, `tick_outcome`, `on_timer_tick`.
- `os/src/kernel/src/sched.rs` — `Scheduler::live_priority_of`, the shared-reference priority read an interrupt path needs.
- `os/src/hal-x86_64/src/extended_state.rs` — `ExtendedState`, `EXTENDED_STATE_BYTES`.
- `os/src/hal-x86_64/src/rflags.rs` — `interrupts_enabled`, `should_reenable`.
- `os/src/hal-x86_64/src/interrupts.rs` — `TickHook`, `set_tick_hook`, `clear_tick_hook`, `disable_interrupts`, `restore_interrupts`, `without_interrupts`, and the ISR stubs' `fxsave`/`fxrstor`.
- `os/src/kernel/src/fixture_preempt.rs`, `os/src/kernel/src/fixture_priority_inversion.rs` — the two Tier 0 fixtures.
- `os/src/kernel/Cargo.toml`, `os/src/kernel/src/main.rs` — the two fixture features and their boot branches.
- `os/src/xtask/src/main.rs` — the two `--fixture=` mappings.
- `.github/workflows/ci.yml` — the two new CI steps.

## Reports

- [`REPORT-2026-07-28-03`](../reports/REPORT-2026-07-28-03.md) — the captures, the misplaced-save finding, the falsification run, and what remains open.
