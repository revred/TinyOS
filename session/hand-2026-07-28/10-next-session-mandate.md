# Handover 10 — Next-Session Mandate

Written at the close of 2026-07-28, after `STORY-P1-04-01` completed and `EPIC-P9` was decomposed. This is the start-here document for the next session: what state the project is actually in, what to do first, and what not to be misled by.

## Where the project stands

**`EPIC-P1` is two thirds done: `FEAT-P1-01`, `-02` and `-03` are complete; `FEAT-P1-04` is half done; `-05` and `-06` are Specified and untouched.**

The material change this session is that **this kernel preempts**. The local-APIC timer, armed since `STORY-P0-04-02` and consumed by nothing since, finally has a consumer. A task whose body contains no `switch`, no `hlt` and no scheduler call ran ~1.3 million iterations, was taken off the CPU by a real tick in favour of a task the tick hook itself had just made `Ready`, and resumed exactly where it was for another ~420,000. Priority inheritance is proven behaviourally with a real medium-priority task competing, closing a caveat `kernel::lock`'s own module doc has carried since `EPIC-P0`.

A second, unplanned body of work also landed: the memory-confidentiality proposal was reviewed, revised and decomposed into **`EPIC-P9`**.

Read, in this order, before starting anything:

1. [`09-story-p1-04-01-preemption.md`](09-story-p1-04-01-preemption.md) — preemption, extended state, inversion avoidance, and **the canonical loose-ends register (`LE-01`…`LE-21`)**.
2. [`goals/stories/STORY-P1-04-02.md`](../../goals/stories/STORY-P1-04-02.md) and [`goals/tests/TEST-P1-04-02-A.md`](../../goals/tests/TEST-P1-04-02-A.md) — your actual work, already specified. See the next section for why that changes how you start.
3. [`07-memory-confidentiality-proposal.md`](07-memory-confidentiality-proposal.md) and [`goals/epics/EPIC-P9.md`](../../goals/epics/EPIC-P9.md) — only if you intend to touch that work. It is deliberately parked.

## Start here: finish `STORY-P1-04-02` (WCET watchdog on the real timer)

**This Story is already started, and you begin from an unusual position: the Test document is written and no implementation code exists.** That is the process working as intended for once — `TEST-P1-04-02-A` clauses 1–9 were written from the finalized acceptance criteria before any code, which is the second Test document in this Feature for which that is true without qualification.

**Do not rewrite the Test document because the hardware behaves differently than you expected.** Every bound and every observable consequence in clauses 4–6 was fixed before the fixtures existed, precisely so that no number in it is chosen after seeing what QEMU produced. If a bound turns out to be wrong, that is a finding to record and argue, not a number to quietly adjust.

Four things make this Story harder than it looks, and all four are already documented rather than waiting to be discovered:

- **A WCET overrun is not a CPU fault, and the decision not to route it through `kernel::fault::Disposition` is already made.** That module's doc is emphatic: `Disposition::of` reads exactly one field — which context was running — and never the vector, error code, faulting address or RIP, because those come from possibly attacker-steered execution (`BND-04`). It refuses a `Resume` arm and refuses a single-variant double-fault enumeration on the stated grounds that unreachable arms and non-decisions are liabilities. An overrun has no frame, no vector, no hardware event, and — unlike a fault — a genuine choice of outcomes the task declared in advance. Routing it through `Disposition` means giving that function a second input and ending the invariant it exists to hold. Build a **parallel** `OverrunPolicy`/`OverrunDisposition`, the same way `STORY-P1-04-01` left `kernel::dispatch` untouched.

- **The attribution rule is the clause most likely to be got wrong in a way that still looks like it works.** A tick that lands in the dispatcher or an idle context must be charged to **nobody**. Charging it to whichever task ran most recently makes every budget consistently wrong in one direction — and a budget wrong in a consistent direction is worse than one obviously broken, because it produces plausible numbers nobody questions.

- **The Degrade arm is vacuous without a competitor.** A fixture that degrades a task's priority and asserts the priority changed has proven nothing an assignment statement could not. Clause 5 requires a third task at a priority *between* the offender's original and its declared floor, `Ready` throughout, which starts winning selections only after the degrade. Without it, "degrade" is indistinguishable from "reset the budget and carry on", which is what that arm most easily rots into.

- **The disposition runs in interrupt context, on the offending task's own stack.** Detection happens on a tick, so by the time you act, you are inside the ISR that `STORY-P1-04-01`'s hook calls, standing on the very stack you may be about to restart or abandon. `fixture_fault`'s `ABANDONED_CTX` pattern is the established shape for "save the dying context somewhere nothing will resume". For **Restart** specifically: re-initializing a `Context` over a stack that currently holds a suspended ISR frame is sound only because nothing ever resumes that frame — the same argument `hal_x86_64::fault`'s stubs already rely on. State it; do not leave it implied.

Two smaller notes. `wcet::record_tick` takes `&mut Scheduler`, and calling it from interrupt context is sound **only** under the `IF` discipline `STORY-P1-04-01` established — interrupts are enabled only while a task runs, so the dispatcher's `&mut` and the hook can never coexist. If you call it from anywhere else, that argument does not carry. And the `TripToSafeState` fixture's *correct* result is a fail-closed exit code, so its CI step must expect failure, exactly as `broken-boot` and `idt-apic-unrouted` already do.

### After it, in order of how cheap they are

- **`LE-20` — the shipping image does not preempt.** `os` installs no `TickHook`, so it still runs its workload cooperatively. Small, and the same "proven in a fixture, not on the real boot path" shape `LE-05` had for a whole Feature. It should not be allowed to sit as long.
- **`FEAT-P1-05` / `FEAT-P1-06`** — the two proof Features, and `-06` is the Epic's flagship exit.
- **`FEAT-P9-01`'s two Stories** — the dump-scan audit and the staging-arena wipe. Independent of everything above, no hardware precondition, and the audit is the instrument `EPIC-P9`'s own falsification tests need. Pick these up whenever `EPIC-P1` stalls.

## The state of the tree — read this before you type anything

**The work is on a branch and is not merged, not pushed, and not entirely committed.**

- Branch `feat/p1-04-preemption-and-epic-p9`, three commits ahead of `main` (`5ae9904` FEAT-P1-03 completion, `27126e1` preemption, `4944fbe` `EPIC-P9`).
- **Only `HEAD` is verified.** The tree had never been committed incrementally, and fourteen files carry changes from more than one body of work, so commits 1 and 2 do not pass `check-assurance-spine` standalone — the expected counts and the contract rows landed in different commits. If you need per-commit green, squash; do not try to "fix" the intermediate commits.
- **Three files are uncommitted, deliberately**, because they are `STORY-P1-04-02`'s opening move rather than part of any commit above: `goals/stories/STORY-P1-04-02.md` (finalized criteria), `goals/tests/TEST-P1-04-02-A.md` (new), and `os/src/xtask/src/assurance.rs` (expected Test count 33 → 34). Commit them with your first implementation commit, or on their own — but do not lose them, and note that the spine gate already expects the higher count.

## Two CI facts you will otherwise rediscover the hard way

**The timing gate is flaky on the GitHub runner, and there is now evidence rather than suspicion.** `D05/dispatch_select_highest_priority_ready` sits almost exactly on its own tolerance boundary under CI conditions. Three consecutive runs, same baseline and same limit throughout:

| Run | Commit | observed p50 | limit | Verdict |
|---|---|---|---|---|
| `30257003163` | `91c95c1` (before any of this work) | 123 | 121 | REGRESSED |
| `30273423631` | `c72a1b6` (the merge) | 123 | 121 | REGRESSED |
| `30274004446` | `f58dd8d` | **90** | 121 | ok |
| `30274317558` | `9ffdf44` — **a markdown-only commit** | **182** | 121 | REGRESSED (`min` too) |

The last row is the one that settles it. `9ffdf44` changed a single documentation file and **not one byte of code**, so its binaries are identical to `f58dd8d`'s — and the same measurement moved from 90 to 182. A 2x swing on identical code. The final run also tripped `min`, which should be the *most* stable statistic of the three (`min` baseline=74, observed=182).

The baseline is 76 and the tolerance is `max(60%, 24 cycles)`, so the limit is 121 and the true value under CI conditions wanders across it. **This is `LE-16` meeting `LE-18`**: a gate that can only detect ~1.6x-or-worse regressions, evaluated against a baseline recorded on different hardware, applied to a metric whose CI value *is* around 1.6x and swings by 2x on its own.

**Practical consequence: this step's verdict currently carries no information about the code.** Read the `observed` value, and compare it against the 90–182 range recorded above before concluding anything. A genuine regression would have to clear that band to be visible at all, which is a fair statement of how weak this gate is on this runner.

**Do not "fix" it by widening the tolerance or re-recording the baseline.** That is precisely the move Handover 05 recorded as a mistake — it looks harmless, is hard to argue against later, and would permanently destroy the only gate this project has against timing regressions in exchange for suppressing a symptom. `LE-18` is unowned and needs a Story that decides what these baselines are *of* — per-runner baselines, a noise-floor calibration step, or moving the gate off shared CI hardware entirely. **That Story is the fix, and it is now the highest-value unowned item in the register.**

**A Windows development machine cannot reproduce CI's lint job.** CI runs `cargo clippy --workspace --all-targets`. On Windows that command fails outright — every `no_std` binary needs `hal_x86_64::{boot,interrupts,qemu_exit,serial}`, all gated `not(target_os = "windows")`. The command used throughout this session, `--workspace --lib --tests`, never lints a binary at all, and that is how a `deref_addrof` break reached `main`. What does work locally:

```
cargo clippy -p <kernel|exec|os> --bins \
  --target targets/x86_64-tinyos.json -Zjson-target-spec \
  -Zbuild-std=core,compiler_builtins \
  -Zbuild-std-features=compiler-builtins-mem -- -D warnings
```

Run that before pushing. It is the mirror image of `LE-12` — that one says CI never lints target-only fixture code; this says a Windows dev box never lints the binaries CI does.

## Standing constraints — do not relax these

- **TDD.** Test document when a Story starts, never pre-written for a Story that has not begun; Red before Green where the seam allows it, and where it does not (a Tier 0 fixture debugged against real hardware behaviour), say so plainly in the Test document's process note, as the last four have.
- **Tier 0 is not hardware evidence.** `LE-09` remains open. Every timing claim carries release-blocking hardware debt. Note that `D03` still has **no measured baseline** even after `STORY-P1-04-02`: enforcement being *correct* and enforcement latency being *bounded in real time* are different claims, and only the first is in scope.
- **Never call TinyOS a hobby OS.** It targets data-centre, local-AI, UAV, medical, edge and consumer deployment.
- **No assurance state may be claimed beyond its evidence.** Every Verified Story is still `baseline-debt`. The first `baseline-debt → verified` conversion is `EPIC-P1`'s explicit charge and has not happened.
- **`EPIC-P9` cannot exit at Tier 0**, and its Epic document says so. Seven of its eight Features are gated on hardware that does not exist in this project's test estate. A software TPM under QEMU verifies the emulator.

## Three lessons this session earned, worth carrying

**An adversarial test written to fail found the mechanism in the wrong place.** `LE-14` was scoped correctly and implemented in the obvious place — save extended state around the context switch on the preemption path. The fixture's first run failed anyway, reading back `0x124df8`: neither the victim's pattern nor the preemptor's. An interrupt handler is *itself* ordinary compiled code running on the interrupted task's stack, free to touch SSE registers whether or not it goes on to preempt anything, so guarding only the preempting ticks left every other tick able to corrupt the task it interrupted. The fix moved into the ISR stub and is broader than the criterion asked for. **The specification demanded a check that would fail if the mechanism were absent; the mechanism was present and the check failed anyway, which is the only reason the placement was found before it shipped.**

**The mandate's own caution held up on its first opportunity.** Mid-session `--fixture=priority-inversion` returned exit 2 — `HarnessError`, the exact symptom Handover 04 once misdiagnosed as a boot-timeout flake and "fixed" by loosening the budget. This time the exit code was read rather than pattern-matched: it was a compile error, reported plainly by `xtask` two lines above. Nothing about the timeout was involved.

**Decomposing blocked work is worth doing when the point is to separate what is blocked from what is not.** `EPIC-P9` was decomposed against this project's own just-in-time rule, deliberately: almost all of it is gated on hardware, and writing the dependency chain down is exactly what made the one workable Feature (`FEAT-P9-01`, two Stories, no cryptography and no hardware) separable from the seven that are not. Decomposition surfaced an objection neither source document had — `LE-21`, the fallback tier as a downgrade attack on the forged-kernel defence — which would otherwise have been found during implementation, or not at all.

## Open items, by owner

**Owned by `FEAT-P1-04`:** `LE-02` (WCET has no timer/watchdog — `STORY-P1-04-02`, in progress), `LE-20` (the shipping image installs no tick hook).

**Closed this session:** `LE-01` (priority-inheritance behavioural proof) and `LE-14` (extended-state save/restore — in the ISR stub, so it covers every tick rather than only preempting ones).

**New this session:** `LE-20`, `LE-21` (the fallback-tier downgrade attack — owned by `STORY-P9-04-02`, and a named exit criterion of `EPIC-P9`).

**Reframed:** `LE-11` (`Context::new` seeds task `rflags` with `IF` set). Under preemption this is now *load-bearing by design* — it is what re-enables interrupts across a switch into a task — rather than an accident to be mitigated. Still open for fixtures that arm no IDT.

**Open, unowned:** `LE-08`, `LE-10`, `LE-12`, `LE-18`, `LE-19` part (b). **Open, owned:** `LE-03`, `LE-09`, `LE-15`, `LE-16`.

Full register with origins and fix paths: [Handover 09 §Loose-ends](09-story-p1-04-01-preemption.md#loose-ends-register-canonical-as-of-this-handover).

Still unowned and unscheduled from Handover 06, unchanged: clean task termination (`ExitProcess` routes into the capability trap — contained, but a fault rather than a teardown), real Win32 subsystems, loading from storage, a compiled probe, and the ungated `D04` baseline.

## How to verify you have a good starting state

```
git checkout feat/p1-04-preemption-and-epic-p9
cd os
cargo test --workspace                                    # 383 passing
cargo fmt --all -- --check
cargo clippy --workspace --lib --tests -- -D warnings
cargo run -p xtask --quiet -- check-assurance-spine        # 22 Features / 47 Stories / 34 Tests / 40 Reports
cargo run -p xtask --quiet -- check-image-size             # os, 74,568 bytes
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=preempt
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=priority-inversion
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=os
```

Every Tier 0 fixture should pass, with exactly two exceptions that are *supposed* to return exit 1: `broken-boot` and `idt-apic-unrouted`, each of whose documented pass condition is a distinguishable failure. `STORY-P1-04-02` will add a third (`wcet-trip`).

When sweeping fixtures from PowerShell, pass arguments literally rather than splatting an array — see Handover 05 for what a splat cost.
