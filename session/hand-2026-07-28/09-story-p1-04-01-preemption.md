# Handover 09 — `STORY-P1-04-01` Implemented and Verified: Timer-Driven Preemption, Extended State, and Inversion Avoidance

Follows: [`06-next-session-mandate.md`](06-next-session-mandate.md), which set this Story as the start-here work and named three traps up front. This handover records what was built against it and what the Tier 0 runs actually showed.

Result: **both new fixtures green**, `TEST-P1-04-01-A` written *before* implementation, `REPORT-2026-07-28-03` filed, assurance state `specified` → `baseline-debt`. `FEAT-P1-04` is now In Progress (1 of 2 Stories Verified) and does **not** exit — `STORY-P1-04-02` is untouched.

## The headline

**This kernel preempts.** A task whose body contains no `switch`, no `hlt` and no scheduler call ran for ~1.3 million iterations, was taken off the CPU by a real local-APIC timer tick in favour of a task the tick hook itself had just made `Ready`, and then **resumed exactly where it had been** for another ~420,000 iterations. Every dispatch this project had performed before today was cooperative.

```text
fixture-preempt: preemptions=1 high_ready_tick=3 high_first_ran_tick=3 ticks_to_preempt=0 (bound 2)
fixture-preempt: low_iterations=1723324 resumed_at=1301806 exhausted=false retired_by_tick=true
```

The `resumed_at` figure is the one that matters: it is what separates "preempted" from "killed and restarted", which a bare counter could not have distinguished.

And priority inheritance is finally proven the way `STORY-P0-02-03` said it would have to be — with a real medium-priority task really competing:

```text
fixture-inversion: dispatch order=[0, 2, 0, 2, 1] (0=low 1=medium 2=high), preemptions=1, low_preempted=true
fixture-inversion: medium ready_in_window=true counter_at_block=0 counter_at_resume=0 counter_final=1000 (min 1000)
```

`low → high → low → high → medium`, with medium's counter frozen at 0 through the whole inversion window and at 1000 afterwards. That module's doc comment has carried an explicit "this cannot be verified without a dispatcher this kernel does not have" caveat since `EPIC-P0`. It is deleted.

## The finding: the obvious place to save extended state is the wrong place

The mandate required `LE-14` be scoped or split, never implied. It was scoped — an acceptance criterion of this Story. The implementation then did the obvious thing (save the task's x87/SSE state when a tick decides to preempt, restore it when the task resumes) and the fixture's **first run failed**:

```text
fixture-preempt: xmm0 pattern=0x123456789abcdef clobber=0xfedcba9876543210 corrupted=true first_foreign_value=0x124df8
```

`0x124df8` is neither the victim's pattern nor the preemptor's clobber value, and that is the entire diagnosis. **An interrupt handler is itself ordinary compiled code running on the interrupted task's stack**, free to touch SSE registers whether or not it goes on to preempt anything. Guarding only the preempting ticks left every *other* tick — the overwhelming majority — able to corrupt the task it interrupted.

`FXSAVE`/`FXRSTOR` therefore moved into the timer ISR **stub**, wrapping the whole handler call, with the 512-byte area carved out of the interrupted stack. That is correct by construction rather than by an argument about what the handler happens to compile to: nothing Rust can emit runs before the save or after the restore. It composes with a switch taken inside the handler for free — the area lives on the task's own stack, so it travels with the suspended task.

Two things worth carrying:

- **The correct fix was broader than the criterion asked for.** Every binary here that arms the timer now has its SSE state protected from its own tick handler, cooperative or not. That was a latent defect *before* this Story, not one it introduced.
- **The check was then deliberately falsified.** With the pair removed from the stub and nothing else changed, the fixture reports `corrupted=true ... at_iteration=495535` and `ok=false`. Mid-run, at a tick — not at iteration 1, which is what would have indicated the task's own compiled code. A save/restore nobody has watched fail is a save/restore nobody has evidence for.

## What was built

| Component | Location |
|---|---|
| `TickOutcome`, `tick_outcome` (pure decision), `on_timer_tick` | `os/src/kernel/src/preempt.rs` (new) |
| `Scheduler::live_priority_of` | `os/src/kernel/src/sched.rs` |
| `ExtendedState`, `EXTENDED_STATE_BYTES` | `os/src/hal-x86_64/src/extended_state.rs` (new) |
| `interrupts_enabled`, `should_reenable` | `os/src/hal-x86_64/src/rflags.rs` (new) |
| `TickHook`/`set_tick_hook`/`clear_tick_hook`, `disable_interrupts`/`restore_interrupts`/`without_interrupts`, ISR-stub `fxsave`/`fxrstor` | `os/src/hal-x86_64/src/interrupts.rs` |
| `preempt`, `priority-inversion` fixtures | `os/src/kernel/src/fixture_preempt.rs`, `fixture_priority_inversion.rs` |
| Two `--fixture=` mappings, two CI steps | `os/src/xtask/src/main.rs`, `.github/workflows/ci.yml` |

`kernel::dispatch` was **not modified**. That is not restraint, it is the design: a preempting tick calls `switch` from interrupt context into the same `dispatcher_ctx` `run_once` is already suspended at, so control returns to `run_once` at exactly the point a cooperative yield would have left it, and the still-`Running` task is returned to `Ready` by code that was already there.

## Design decisions worth carrying forward

**Interrupts are enabled only while a task runs.** The dispatcher holds `&mut Scheduler`; the tick hook reads the same scheduler. The mechanism keeping those apart is not a convention or a software flag — it is `RFLAGS`. The dispatcher body runs with `IF` clear, and a task's own saved flags re-enable interrupts across the switch *into* it and clear them again across the switch *back*. Nothing has to remember anything; the flag travels with the context. `kernel::preempt` therefore takes `*mut Scheduler`, never `&mut`. Task code that touches the scheduler uses `without_interrupts`, which is the same property bought explicitly.

**The tick hook is a registered pointer, not a linker symbol.** The fault path uses `tinyos_fault_entry` (HAL declares, binary defines) because *every* binary needs a fault handler. Most binaries here legitimately have no tick consumer, so preemption is opt-in via `set_tick_hook`, and a build that registers nothing ticks exactly as before. That is what leaves every pre-existing fixture bit-for-bit unaffected.

**The EOI must precede the hook.** A preempting hook does not return until the interrupted task is next resumed, and until the local APIC is told the interrupt is complete it delivers nothing further. Signalling afterwards would mean the *first* preemption silently stopped the clock and nothing could ever be preempted again.

**Equal priority does not preempt, and its absence is pinned by a test.** Tick-driven rotation between equal-priority Ready tasks is a policy this Story has no requirement for; a later Story that wants round-robin has to change a test rather than discover the behaviour.

**The inversion test asserts three things.** The dispatch order, the frozen counter, *and* that medium was `Ready` in the window and runs afterwards. The third is the one that would have been easy to omit, and without it a frozen counter proves nothing at all.

## A process note, and a small vindication of the mandate's second lesson

Mid-session, `cargo run -p xtask -- qemu-x86_64 --fixture=priority-inversion` returned **exit 2** — `HarnessError`, the same symptom Handover 04 once misdiagnosed as a boot-timeout flake and "fixed" by loosening the budget. This time the exit code was read rather than pattern-matched: it was a **compile error** in the fixture (a `static_mut_refs` denial), reported plainly by `xtask` two lines above the exit code. Nothing about the timeout was involved. The mandate's standing caution held up on its first opportunity.

## Loose-ends register (canonical as of this handover)

Carried forward from [Handover 06](06-next-session-mandate.md) / [Handover 10 of 27 July](../hand-2026-07-27/10-story-p1-03-01-cr3-switching.md#loose-ends-register-canonical-as-of-this-handover). **Two closed this handover (`LE-01`, `LE-14`); two new (`LE-20`, `LE-21`).**

| ID | Loose end | Origin | Owner / fix path | Status |
|---|---|---|---|---|
| LE-01 | Priority-inheritance behavioral half never verified | `STORY-P0-02-03` | `STORY-P1-04-01` criterion 3 | **Closed this handover** — `fixture_priority_inversion`, `TEST-P1-04-01-A` clause 6 |
| LE-02 | WCET enforcement has no timer and no watchdog behind it | `STORY-P0-02-04` | `STORY-P1-04-02` | Open — owned, untouched by this Story |
| LE-03 | No real fault handling for the remaining vectors | Handover 32 | `FEAT-P1-02` | Open — `#XF` (19), `#MC` (18) and every other vector still reach the shared fail-closed default |
| LE-04 | No TSS/IST; a fault during fault handling triple-faults | Handover 32 | `STORY-P1-02-02` | Closed (27 July, Handover 07) |
| LE-05 | `exec::AddressSpace` built but never installed | `STORY-P0-05-02` | `FEAT-P1-03` | Closed (Handover 05) |
| LE-06 | `pool-bench` was a divergent sibling harness | Handover 35 | `STORY-P1-01-01` | Closed |
| LE-07 | CI has never been observed running any of this work | Standing since Handover 07 | Phase-independent | Closed |
| LE-08 | I/O APIC device-IRQ routing deferred (local APIC only) | `STORY-P0-04-02`/`-03` | First Story routing a device IRQ | Open — deferred with trigger |
| LE-09 | No hardware tier; every number is Tier 0 | Handover 37 directive 1 | Option B with the carve-out | Open — narrowed (Handover 08); unchanged here |
| LE-10 | ECAM/MCFG config access and PCI bridge traversal deferred | `STORY-P0-04-03` | First Story needing extended config space | Open — deferred with trigger |
| LE-11 | `Context::new` seeds task `rflags` with `IF` set | `STORY-P1-01-01` | `FEAT-P1-02` | **Reframed, not closed** — under preemption this is now *load-bearing by design* (it is what re-enables interrupts across a switch into a task), not an accident to be mitigated. Still open for the fixtures that arm no IDT. |
| LE-12 | CI's clippy never lints target-only fixture code | `STORY-P1-01-01` | Per-fixture target clippy in the CI lint job | Open — unowned |
| LE-13 | Measurement ran dev-profile binaries | `STORY-P1-01-01` | `STORY-P1-01-02` | Closed |
| LE-14 | `context::switch` saves no SSE/x87 state | `STORY-P1-01-01` / ADR 0003 | `FEAT-P1-04` | **Closed this handover** — but in the ISR stub, not on the switch path; see the finding above |
| LE-15 | The AArch64 generic timer is a 54 MHz system counter | `STORY-P1-01-03` | Decide when a board exists | Open — owned |
| LE-16 | The Tier 0 timing gate detects only ~1.6x-or-worse regressions | `STORY-P1-01-02` | Only a hardware tier fixes it (`LE-09`) | Open — owned |
| LE-17 | The fault path has no timing baseline | `STORY-P1-02-01` | A fault-latency `fixture_measure` phase | Closed (27 July, Handover 09) |
| LE-18 | The timing gate is host-condition-sensitive | `STORY-P1-02-02` | Needs a decision about what baselines are *of* | Open — unowned, needs a Story |
| LE-19 | `--update-baseline` rewrites every measured row | 27 July, Handover 09 | Part (b): refresh a named metric without touching the rest | Part (a) closed; part (b) open — unowned |
| **LE-20** | **Preemption is proven in fixtures but the shipping image does not use it.** `os` installs no `TickHook`, so the system image still runs its workload to completion cooperatively | `STORY-P1-04-01` | `STORY-P1-04-02`, or a small companion Story — this is the same "proven in a fixture, not on the real boot path" shape `LE-05` had, and it should not be allowed to sit as long | **New — owned** |
| **LE-21** | **The graceful-degradation tier is a downgrade attack on the forged-kernel defence.** `EPIC-P9`'s central claim is that a TPM refuses to unseal for a boot that measures differently — but a fallback tier that activates when the TPM is *unreachable* hands an attacker a strictly easier path than forging measurements: make it unreachable. An OS also cannot refuse to boot because attestation is down, so the two requirements genuinely conflict and the resolution is per-deployment | [`07` §8.1](07-memory-confidentiality-proposal.md), raised while decomposing `EPIC-P9` | [`STORY-P9-04-02`](../../goals/stories/STORY-P9-04-02.md) — which exists separately from the TPM-driver Story for exactly this reason | **New — owned, and a named exit criterion of `EPIC-P9`** |

## What remains open

1. **`LE-02` / `STORY-P1-04-02`.** `wcet::record_tick` is still not driven by the real timer and no overrun trips a declared policy. `FEAT-P1-04` does not exit until it does. The tick hook is now the obvious place to drive it from, and the shape is clear: attribute a tick to the running task, and route `WcetError::BudgetExceeded` into `FEAT-P1-02`'s fault machinery rather than a log line.
2. **`LE-20` — the shipping image does not preempt.** Small, and the natural companion to the above.
3. **Tier 0 only.** The clause 4 bound is counted in *ticks*, not cycles or microseconds, deliberately — QEMU's APIC-timer-to-wall-clock relationship is not a number to build a `D03` budget on, and calling a tick count a latency figure is exactly the mistake `STORY-P1-03-03`'s `D04` result argued against.
4. **`FXSAVE`, not `XSAVE`.** x87/MMX/XMM0–15 covered; AVX and wider state are not. Nothing here generates AVX today, but a build that enabled it would be silently wrong until the area widened.
5. **One preemption per fixture, not sustained multitasking.** Each scenario is scripted and demonstrates a single preemption. Nothing here is evidence about behaviour under sustained preemptive load, and no such claim should be read into it.
6. **Equal-priority tasks do not rotate**, deliberately — a real gap for any workload wanting fair sharing at one priority level.
7. **The cost of the ISR-stub save is unmeasured.** `check-timing-regression` passed across all twelve gated statistics, but that is not evidence about this change: `fixture_measure` installs a fault-only IDT and arms no timer, so none of the gated paths take a tick. Two instructions is certainly cheaper than the corruption they prevent, but that is an argument, not a number, and `D03` has no baseline to turn it into one.

## Planning work recorded after this Story

Two documents arrived in this folder after this Story landed and are not part of it: [`07-memory-confidentiality-proposal.md`](07-memory-confidentiality-proposal.md) (revised 2026-07-28 against [`08`](08-memory-confidentiality-review.md) §15's eight requested changes) and [`08`](08-memory-confidentiality-review.md) itself. They are decomposed into [`EPIC-P9`](../../goals/epics/EPIC-P9.md) — 8 Features, 10 Stories, of which only [`FEAT-P9-01`](../../goals/features/FEAT-P9-01.md)'s two can be worked before a hardware tier exists. `LE-21` above is the one objection that decomposition surfaced which neither source document had.

**This handover was originally numbered 07** and was renumbered to 09 to resolve a collision with the confidentiality proposal, which claimed the same number in a concurrent thread.

## How to verify this state

```
cd os
cargo test --workspace                                             # 383 passing
cargo fmt --all -- --check
cargo clippy --workspace --lib --tests -- -D warnings
cargo run -p xtask --quiet -- check-assurance-spine                # 14 Features / 37 Stories / 33 Tests / 40 Reports
cargo run -p xtask --quiet -- check-image-size                     # os, 74,568 bytes
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=preempt
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=priority-inversion
```

Every Tier 0 fixture passes, with the same two documented exceptions that are *supposed* to return exit 1: `broken-boot` and `idt-apic-unrouted`. When sweeping fixtures from PowerShell, pass arguments literally rather than splatting an array — see Handover 05 for what a splat cost.
