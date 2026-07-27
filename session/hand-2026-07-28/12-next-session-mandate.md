# Handover 12 — Next-Session Mandate

Written at the close of 2026-07-28, after `STORY-P1-04-02` completed and `FEAT-P1-04` exited. This is the start-here document for the next session: what state the project is actually in, what to do first, and what not to be misled by.

## Where the project stands

**`EPIC-P1` is four Features of six. `FEAT-P1-01`, `-02`, `-03` and `-04` are all complete; `FEAT-P1-05` and `-06` are Specified and untouched.**

The material change this session is that **this kernel now takes the CPU away from software that will not give it up, and holds it to a budget it declared in advance.** `STORY-P1-04-01` made dispatch preemptive; `STORY-P1-04-02` made the same tick enforce WCET budgets. A task that crosses its declared budget is restarted from its entry point, degraded beneath a real competitor, or trips the system to a fail-closed safe state — whichever it declared at creation, audited, detected on the exact tick that crossed.

Read, in this order, before starting anything:

1. [`11-story-p1-04-02-wcet-enforcement.md`](11-story-p1-04-02-wcet-enforcement.md) — what landed, the design decisions not to re-litigate, and the loose-ends delta.
2. [`09-story-p1-04-01-preemption.md`](09-story-p1-04-01-preemption.md) — **the canonical loose-ends register (`LE-01`…`LE-21`)**. `LE-22` is in Handover 11.
3. [`10-next-session-mandate.md`](10-next-session-mandate.md) — **§"The one regression this session leaves behind"** is still the best statement of the timing-gate problem and is still unfixed. Read it before touching the gate.

## Start here: `LE-20` — put the tick hook on the real boot path

**Two Verified Features now describe mechanisms that do not run in the shipping image.** `os` installs no `TickHook`, so the system image still dispatches cooperatively and enforces no budget. Preemption has been in that state for one Feature; enforcement joins it today.

This is small — `os/src/os/src/main.rs` needs the hook `fixture_preempt` and `fixture_wcet` already demonstrate — and it is the highest-value cheap work in the register, for a reason worth stating plainly: **`LE-05` had exactly this shape and was allowed to sit for a whole Feature.** Handover 10 said `LE-20` "should not be allowed to sit as long." It has now sat one Feature longer and acquired a companion. A third would be a pattern rather than an oversight.

Four things to get right, all of which the fixtures already resolved:

- **The `os` dispatcher must run with `IF` clear**, and let each task's own saved `RFLAGS` re-enable interrupts across the switch into it. That is the whole re-entrancy argument (`kernel::preempt`'s module doc); a dispatcher that leaves interrupts enabled while holding `&mut Scheduler` is unsound, not merely racy.
- **The hook needs a `CURRENT_TASK`-equivalent**, written only by the dispatcher with interrupts disabled, and checked *first* in the hook before the scheduler is touched at all. That check is simultaneously the attribution rule's `Nobody` arm and the soundness precondition.
- **`os` uses `run_once_in_space`, not `run_once`.** The fixtures use the latter. A preempting tick switches into the dispatcher's suspended context, so control returns inside `run_once_in_space` — which then does *not* reinstall the dispatcher's own `CR3`. Handover 06 already named the dispatcher's non-restoration of its address space as a trap that becomes load-bearing under interrupt re-entry. **Check it before assuming the fixture pattern transfers.**
- **What policy should the real workload declare?** `blue-sharc.exe` currently gets `TripToSafeState` with a generous budget, which is a placeholder rather than a decision. Making it a real declaration is part of this work, not a follow-on.

A Story and Test document are required — this is behaviour change on the shipping image, not a refactor. There is no Story document yet.

### After it, in order of how cheap they are

- **The timing gate (`LE-16` + `LE-18`).** `main`'s CI is red on it and has been for four sessions. It belongs under `FEAT-P1-01` as a fourth Story; no Story document exists. **This outranks everything below it on cost-of-leaving.** See the section below.
- **`LE-22`** — degrade and priority inheritance have not been reconciled. Documenting it in both modules' doc comments is minutes; reconciling it is a Story.
- **`FEAT-P1-05` / `FEAT-P1-06`** — the two proof Features, and `-06` is the Epic's flagship exit.
- **`FEAT-P9-01`'s two Stories** — the dump-scan audit and the staging-arena wipe. Independent of everything above, no hardware precondition. Pick these up whenever `EPIC-P1` stalls.

## The state of the tree

**Everything is committed, merged to `main` with `--no-ff`, and pushed. The working tree is clean.**

```
566d3e2  Correct the handover: the work is merged locally, not pushed
cd67b5b  Merge: WCET enforcement on the real timer (STORY-P1-04-02, FEAT-P1-04 exits)
  d0d8e65  Enforce WCET budgets on the real timer: FEAT-P1-04 exits
0316d53  Promote the timing gate from a documented flake to the regression it is
```

Handover 10's caution still applies to anything older than `c72a1b6`: the tree was never committed incrementally before that point, fourteen files carry changes from more than one body of work, and bisecting across `5ae9904`/`27126e1` will produce confusing results. That is a property of how the tree arrived, not a defect to repair.

## CI, and the one step you must not read as a signal

**CI run `30285899382` on `566d3e2`: every job green except the timing gate.** All three new WCET steps passed, including `wcet-trip`'s capture assertions, and the lint job — which lints things a Windows box cannot — is clean.

**The timing gate failed with `D05/dispatch_select_highest_priority_ready` p50 = 123 against limit 121.** Before concluding anything from that: **123 is the exact value observed at `91c95c1`, which predates all of `FEAT-P1-04`, and again at the `c72a1b6` merge.** The same metric on byte-identical binaries has been recorded at 90 and at 182. This step is measuring the GitHub runner.

Three facts, all evidenced in Handover 10:

1. **The noise is global.** Between two runs of *identical binaries*, every gated metric moved together by 1.8–2.2x. `D05/dispatch_select` is not specially unstable; it simply has the least headroom, so it trips first.
2. **The committed baselines are not mutually consistent.** In the same run, two metrics report `improved (is the baseline stale?)` by 3–6x while `D05/dispatch_select`'s baseline is *tighter* than anything the runner achieves. They cannot all have been captured under the same conditions.
3. **Therefore the verdict carries no information about the code.** Read the `observed` value and compare it against the recorded 90–182 band before concluding anything.

**The fix, and the trap.** The strong fix is to stop gating on absolute cycle counts and gate on same-run *ratios* — every metric moves together, so a ratio between two metrics in the same run is far more stable than either absolute, and a genuine regression changes the ratio while a slow runner does not. This project already solved this problem once: `kernel::fixture_idt_apic_timer` gates on `MAX_INTERVAL_RATIO` precisely because "QEMU's own APIC-timer-to-wall-clock relationship under software emulation is not itself a stable absolute number this fixture should depend on."

**Do not reach for `--update-baseline` on its own.** Handover 05 recorded that re-recording a baseline to make a failing gate green is a mistake that looks harmless, is hard to argue against later, and destroys the signal in exchange for suppressing a symptom. Re-recording is defensible here *only* as part of a Story that fixes the methodology first. And `LE-19(b)` is a prerequisite: `--update-baseline` rewrites every measured row, so it cannot currently refresh one metric without silently re-recording the rest.

Whatever Story fixes this needs a Test document with a **deliberately-injected regression the new gate still catches** — a gate made noise-tolerant is worthless if it has also been made blind. `--inject-regression` exists for exactly that.

## The lesson this session earned, and it is the important one

**All three new Tier 0 fixtures passed on their first run. Two of them proved nothing.**

Every enforcement mutation was removed from `wcet::apply` — the decision still taken, the spoor still stamped, nothing done to the task — and everything re-run. Host tests behaved. At Tier 0:

- **`wcet-restart` stayed green.** The fixture was proving its *own* context rewind. The hook rebuilds the task's `Context` and `dispatch::run_once`'s ordinary `Running → Ready` transition re-queues it — together reproducing every *visible* effect of a restart. What they could not reproduce was the **spacing**: a task whose budget window is never reset overruns again on the very next tick, so enforcements pile up one per tick instead of one per window. 7 attributed ticks across 3 enforcements when broken; 15 across 3 when working.
- **`wcet-trip` reported `ok=true`.** It checked the disposition it was handed and never checked the kernel had marked the task `Finished`. That matters especially because **its pass condition is a failure exit code**, so exit 1 carries no information on its own.

Generalise both. A fixture that constructs part of the effect it is testing will pass without the part it is meant to test — look for a consequence the fixture's own machinery *cannot* fake. And **`broken-boot` and `idt-apic-unrouted` have the same hole `wcet-trip` had**: their pass condition is an exit code that any failure also produces. `wcet-trip` now greps its serial capture (`--serial-capture=` is new this session and exists for this). The other two have not been looked at, and should be.

A second, smaller lesson: **linting fixture-gated code found two clippy errors CI would never have caught.** `LE-12` says CI never lints target-only fixture code, and that is exactly what happened. If you add a fixture, lint it with its feature enabled — the per-binary command below plus `--features <the fixture>`. Making that a CI matrix step is cheap and would close `LE-12` in practice.

## Standing constraints — do not relax these

- **TDD.** Test document when a Story starts, never pre-written for a Story that has not begun. Red before Green where the seam allows it — and note the deviation `TEST-P1-04-02-A` records honestly: the host seams were written Green-first with a recorded falsification in place of a recorded Red. That is weaker than the rule, is written down as such, and **is not the new normal**.
- **Tier 0 is not hardware evidence.** `LE-09` remains open. Every timing claim carries release-blocking hardware debt. `D03` still has **no measured baseline** even after `STORY-P1-04-02`: enforcement being *correct* and enforcement latency being *bounded in real time* are different claims, and only the first is in scope.
- **Never call TinyOS a hobby OS.** It targets data-centre, local-AI, UAV, medical, edge and consumer deployment.
- **No assurance state may be claimed beyond its evidence.** All 35 functionally Verified Stories are still `baseline-debt`. The first `baseline-debt → verified` conversion is `EPIC-P1`'s explicit charge and has not happened.
- **`EPIC-P9` cannot exit at Tier 0**, and its Epic document says so. Seven of its eight Features are gated on hardware that does not exist in this project's test estate. A software TPM under QEMU verifies the emulator.

## Open items, by owner

**Closed this session:** `LE-02` (WCET had no timer or watchdog behind it).

**New this session:** `LE-22` — degrade and priority inheritance are unreconciled. `PriorityInheritingLock` restores a holder's pre-boost priority on unlock, so a degrade applied to a boosted holder is **silently undone**; and degrading a boosted task discards a boost a high-priority waiter depends on, reintroducing the inversion `STORY-P0-02-03` exists to prevent. No fixture combines them, no test pins it, neither module's doc mentions the other. Found by reading, not by a failure. Unowned.

**A live regression, and the first thing to fix after `LE-20`:** `LE-16` + `LE-18` together. Diagnosed with evidence in Handover 10; unchanged this session. Belongs under `FEAT-P1-01`; **no Story document exists yet.**

**Open, unowned:** `LE-08`, `LE-10`, `LE-12`, `LE-18`, `LE-19` part (b), `LE-22`.
**Open, owned:** `LE-03`, `LE-09`, `LE-11` (reframed — under preemption, `Context::new` seeding `IF` is load-bearing by design, not an accident to mitigate; still open for fixtures that arm no IDT), `LE-15`, `LE-16`, `LE-20`, `LE-21`.

Full register with origins and fix paths: [Handover 09 §Loose-ends](09-story-p1-04-01-preemption.md#loose-ends-register-canonical-as-of-this-handover).

Still unowned and unscheduled from Handover 06, unchanged: clean task termination (`ExitProcess` routes into the capability trap — contained, but a fault rather than a teardown), real Win32 subsystems, loading from storage, a compiled probe, and the ungated `D04` baseline.

## How to verify you have a good starting state

```
git checkout main            # 566d3e2; the work is merged and pushed
cd os
cargo test --workspace                                    # 395 passing
cargo fmt --all -- --check
cargo run -p xtask --quiet -- check-assurance-spine        # 22 Features / 47 Stories / 34 Tests / 41 Reports
cargo run -p xtask --quiet -- check-image-size             # os, 74,568 bytes
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=preempt
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=priority-inversion
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=wcet-restart
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=wcet-degrade
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=wcet-trip     # exit 1 is the PASS
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=os
```

To read any fixture's own diagnostic output, add `--serial-capture=PATH` (new this session — without it fixtures run with `-serial none` and report only a pass/fail bit).

**The lint command CI actually uses**, which is *not* `--workspace --lib --tests` and cannot be run as `--all-targets` on Windows (`hal_x86_64::{boot,interrupts,qemu_exit,serial}` are all `not(target_os = "windows")`):

```
cargo clippy -p <kernel|exec|os> --bins \
  --target targets/x86_64-tinyos.json -Zjson-target-spec \
  -Zbuild-std=core,compiler_builtins \
  -Zbuild-std-features=compiler-builtins-mem -- -D warnings
```

**Run it from bash, not PowerShell** — PowerShell mangles `-Zbuild-std=core,compiler_builtins` on the comma. And repeat it with `--features fixture-<name>` for any fixture you touch, per the `LE-12` lesson above. All must be clean before you push.

Every Tier 0 fixture should pass, with exactly three exceptions that are *supposed* to return exit 1: `broken-boot`, `idt-apic-unrouted` and `wcet-trip`, each of whose documented pass condition is a distinguishable failure.

When sweeping fixtures from PowerShell, pass arguments literally rather than splatting an array — see Handover 05 for what a splat cost.
