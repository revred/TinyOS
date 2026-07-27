# Handover 14 — Next-Session Mandate

Written at the close of 2026-07-28, after `STORY-P1-04-03` completed and `FEAT-P1-04` exited on the shipping image. This is the start-here document: what state the project is actually in, what to do first, and what not to be misled by.

**On the folder date, before anything else.** The actual calendar date was 2026-07-27, and so were the commits underneath this whole folder. This repository's document dates run one day ahead of the clock and have since this folder was created. Handover 13 §"A note on dates" records why, what was done about it, and why it was not repaired. Do not read a Report's date as evidence of when anything happened.

## Where the project stands

**`EPIC-P1` is four Features of six. `FEAT-P1-01` through `-04` are complete; `FEAT-P1-05` and `-06` are Specified and untouched.**

The material change this session is that **the binary this project ships now does what its Verified Features say it does.** `os` installs a tick hook, dispatches preemptively with `IF` clear, charges every tick to a budget, and applies the consequence its workload declared. `LE-20` is closed, and it turned out to be a denial-of-service hole rather than a wiring gap — see Handover 01.

Read, in this order, before starting anything:

1. [`13-story-p1-04-03-shipping-image-enforcement.md`](13-story-p1-04-03-shipping-image-enforcement.md) — what landed, the design decisions not to re-litigate, and the loose-ends delta.
2. [`09-story-p1-04-01-preemption.md`](09-story-p1-04-01-preemption.md) — **the canonical loose-ends register (`LE-01`…`LE-21`)**. `LE-22` is in Handover 11.
3. [`10-next-session-mandate.md`](10-next-session-mandate.md) — **§"The one regression this session leaves behind"** is still the best statement of the timing-gate problem and is still unfixed. Read it before touching the gate.

## Start here: the timing gate (`LE-16` + `LE-18`)

**`main`'s CI has been red on it for five sessions, and it is now the oldest live regression in the register.** Two consecutive mandates have named it the highest-value unowned work and two consecutive sessions have done something else first — both times for a defensible reason, and the reasons are running out. It belongs under `FEAT-P1-01` as a fourth Story. **No Story document exists yet.**

The diagnosis is already done and is evidenced in Handover 10. The three facts:

1. **The noise is global.** Between two runs of *identical binaries*, every gated metric moved together by 1.8–2.2x. `D05/dispatch_select` is not specially unstable; it has the least headroom, so it trips first.
2. **The committed baselines are not mutually consistent.** In one run, two metrics report `improved (is the baseline stale?)` by 3–6x while `D05/dispatch_select`'s baseline is *tighter* than anything the runner achieves.
3. **Therefore the verdict carries no information about the code.** Read the `observed` value against the recorded 90–182 band before concluding anything.

**The fix, and the trap.** Gate on same-run *ratios* rather than absolute cycle counts: every metric moves together, so a ratio between two metrics in the same run is far more stable than either, and a genuine regression changes the ratio while a slow runner does not. This project already solved this once — `kernel::fixture_idt_apic_timer` gates on `MAX_INTERVAL_RATIO` for exactly this reason.

**Do not reach for `--update-baseline` on its own.** Handover 05 recorded that re-recording a baseline to make a failing gate green destroys the signal in exchange for suppressing a symptom. It is defensible here *only* as part of a Story that fixes the methodology first, and `LE-19(b)` is a prerequisite: `--update-baseline` rewrites every measured row, so it cannot refresh one metric without silently re-recording the rest.

Whatever Story fixes this needs a Test document with a **deliberately-injected regression the new gate still catches** — `--inject-regression` exists for exactly that. A gate made noise-tolerant is worthless if it has also been made blind.

**This session earned a directly relevant lesson**, and it is the reason this item is now first: `TEST-P1-04-03-A`'s clause 1 was originally written to assert a serviced-tick count, and two runs of *identical binaries* produced different counts. That is `LE-18` in miniature, caught before it shipped only because the same binary was re-run. The timing gate is the same failure mode, already shipped, already red.

### After it, in order

- **`LE-22`** — degrade and priority inheritance are unreconciled. `PriorityInheritingLock` restores a holder's pre-boost priority on unlock, so a degrade applied to a boosted holder is **silently undone**; and degrading a boosted task discards a boost a high-priority waiter depends on, reintroducing the inversion `STORY-P0-02-03` exists to prevent. Documenting it in both modules' doc comments is minutes; reconciling it is a Story. **This is now more pressing than it was**, because the shipping image's own workload declares `Degrade`.
- **`broken-boot` and `idt-apic-unrouted`** — both still have the exit-code hole `wcet-trip` closed: their documented pass condition is a failure exit code that any other failure also produces. `--serial-capture=` plus a grep, per the `wcet-trip` and `os-runaway` CI steps. Cheap, and it has now been deferred twice.
- **`FEAT-P1-05` / `FEAT-P1-06`** — the two proof Features, and `-06` is the Epic's flagship exit.
- **`FEAT-P9-01`'s two Stories** — the dump-scan audit and the staging-arena wipe. Independent of everything above, no hardware precondition. Pick these up whenever `EPIC-P1` stalls.

## The state of the tree — read this before anything else

**Nothing is committed.** All of this session's work is in the working tree. `main` is at `6b47b4d` and `git status` is dirty. Nothing has been branched, committed or pushed.

That is a deliberate stopping point, not an oversight: the session's mandate was to deliver the work, and committing/pushing was not requested. **Commit it before starting anything new**, on a branch, and merge with `--no-ff` per this project's convention so the Story stays visible as a unit.

Handover 10's caution still applies to anything older than `c72a1b6`: the tree was never committed incrementally before that point, fourteen files carry changes from more than one body of work, and bisecting across `5ae9904`/`27126e1` will produce confusing results. That is a property of how the tree arrived, not a defect to repair.

## CI

**Not run this session** — nothing was pushed. The last observed run is `30285899382` on `566d3e2`: every job green except the timing gate. Its `D05/dispatch_select` p50 of 123 against a limit of 121 is **the same value observed at `91c95c1`, before any of `FEAT-P1-04`'s work**, and again at the `c72a1b6` merge. The same metric on byte-identical binaries has been recorded at 90 and at 182. **Read the observed value against that band; the verdict carries no information about the code.**

One new CI step landed this session and has never run in CI: `TEST-P1-04-03-A — the shipping image enforces its budget on a workload that never yields`. It passes locally. It greps its serial capture rather than trusting the exit code, so it does not have the hole `broken-boot` and `idt-apic-unrouted` still have.

## The lesson this session earned

**A loose-end that says "the mechanism exists but is not wired up" is describing the absence of that mechanism in whatever ships, and should be read as one.**

`LE-20` was carried for two Features as *"proven in a fixture, not on the real boot path"* — phrasing that reads like a documentation defect. Removing the hook and running the shipping image against a workload that will not yield produces:

```text
xtask: kernel did not reach the isa-debug-exit port within the 15s boot time budget
```

Twenty-four bits of machine code inside an image the capability gate had already admitted, and the system stops indefinitely. That is what the register had been describing, and the wording made it sound smaller. **Go back through the open register and re-read each entry asking what it means for the shipping image**, not what it means for the codebase. `LE-12` (CI never lints target-only fixture code) and `LE-21` are the two most likely to be understated the same way.

A second, narrower lesson: **the same binary, run twice, is a cheap and underused test.** It caught clause 1's draft depending on host speed. It is also, per fact 1 above, the exact technique that diagnosed the timing gate.

## Standing constraints — do not relax these

- **TDD.** Test document when a Story starts, never pre-written for a Story that has not begun. Red before Green where the seam allows it. `TEST-P1-04-03-A` held to this without qualification — bounds fixed before implementation, host seam driven Red-to-Green — and that is the standard, not `TEST-P1-04-02-A`'s recorded deviation.
- **Tier 0 is not hardware evidence.** `LE-09` remains open. Every timing claim carries release-blocking hardware debt. `D03` still has **no measured baseline**: enforcement being *correct* and enforcement latency being *bounded in real time* are different claims.
- **Never call TinyOS a hobby OS.** It targets data-centre, local-AI, UAV, medical, edge and consumer deployment.
- **No assurance state may be claimed beyond its evidence.** All 36 functionally Verified Stories are still `baseline-debt`. The first `baseline-debt → verified` conversion is `EPIC-P1`'s explicit charge and has not happened.
- **`EPIC-P9` cannot exit at Tier 0**, and its Epic document says so. A software TPM under QEMU verifies the emulator.

## Open items, by owner

**Closed this session:** `LE-20`.

**New this session:** none.

**Open, unowned:** `LE-08`, `LE-10`, `LE-12`, `LE-18`, `LE-19` part (b), `LE-22`.
**Open, owned:** `LE-03`, `LE-09`, `LE-11`, `LE-15`, `LE-16`, `LE-21`.

Full register with origins and fix paths: [Handover 09 §Loose-ends](09-story-p1-04-01-preemption.md#loose-ends-register-canonical-as-of-this-handover).

Still unowned and unscheduled from Handover 06, unchanged: clean task termination (`ExitProcess` routes into the capability trap — contained, but a fault rather than a teardown), real Win32 subsystems, loading from storage, a compiled probe, and the ungated `D04` baseline.

## How to verify you have a good starting state

```
cd os
cargo test --workspace                                    # 399 passing
cargo fmt --all -- --check
cargo run -p xtask --quiet -- check-assurance-spine        # 22 Features / 48 Stories / 35 Tests / 42 Reports
cargo run -p xtask --quiet -- check-image-size             # os, 84,864 bytes
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=os
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=os-runaway
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=preempt
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=priority-inversion
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=wcet-restart
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=wcet-degrade
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=wcet-trip     # exit 1 is the PASS
```

To read any fixture's own diagnostic output, add `--serial-capture=PATH` — without it fixtures run with `-serial none` and report only a pass/fail bit.

**The lint command CI actually uses**, which is *not* `--workspace --lib --tests` and cannot be run as `--all-targets` on Windows (`hal_x86_64::{boot,interrupts,qemu_exit,serial}` are all `not(target_os = "windows")`):

```
cargo clippy -p <kernel|exec|os> --bins \
  --target targets/x86_64-tinyos.json -Zjson-target-spec \
  -Zbuild-std=core,compiler_builtins \
  -Zbuild-std-features=compiler-builtins-mem -- -D warnings
```

**Run it from bash, not PowerShell** — PowerShell mangles `-Zbuild-std=core,compiler_builtins` on the comma. And repeat it with `--features <the fixture>` for any fixture you touch (`fixture-os-runaway` for `os`), per `LE-12`. All must be clean before you push.

Every Tier 0 fixture should pass, with exactly three exceptions that are *supposed* to return exit 1: `broken-boot`, `idt-apic-unrouted` and `wcet-trip`.

When sweeping fixtures from PowerShell, pass arguments literally rather than splatting an array — see Handover 05 for what a splat cost.
