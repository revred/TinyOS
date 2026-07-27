# Handover 16 — Next-Session Mandate

Written at the close of 2026-07-28, after `STORY-P1-01-04` closed out the timing gate. This is the start-here document.

**On the folder date, before anything else.** The actual calendar date was 2026-07-27, and so are the commits. This repository's document dates run one day ahead of the clock and have since this folder was created. Handover 13 §"A note on dates" records why and why it was not repaired. Do not read a Report's date as evidence of when anything happened.

## Where the project stands

**`EPIC-P1` is four Features of six.** `FEAT-P1-01` through `-04` are complete; `FEAT-P1-05` and `-06` are Specified and untouched.

Two Stories landed this session and **both are committed and merged**, which is a change from how the last two sessions ended:

- `STORY-P1-04-03` (`dbc9b01`) — the shipping-image enforcement work Handover 14 left in the working tree. Committed first, unchanged.
- `STORY-P1-01-04` (`7b548b4`) — the timing gate now compares same-run ratios, and **the oldest live regression in the register is closed**. `main`'s CI had been red on that step for five sessions.

`main` is at `7b548b4`. **Nothing has been pushed.**

Read, in this order:

1. [`15-story-p1-01-04-ratio-timing-gate.md`](15-story-p1-01-04-ratio-timing-gate.md) — what landed, the four-quadrant evidence, the design decisions not to re-litigate, and the concurrent-work warning in §"A concurrent body of work is in these commits".
2. [`13-story-p1-04-03-shipping-image-enforcement.md`](13-story-p1-04-03-shipping-image-enforcement.md) — the other Story in `main`.
3. **[`goals/assurance/loose-ends.tsv`](../../goals/assurance/loose-ends.tsv) is now the canonical loose-ends register**, and the assurance spine validates against it. Handover 09's prose register is superseded; use the TSV.

## The push happened, CI is green, and `LE-23` is confirmed rather than ruled out

**Read this before the section below, which was written before the run and is kept because its reasoning is what the run then tested.**

`main` was pushed at `22c269f`. **CI run `30294647525` is green on every job** — the first fully green run in five sessions, and the timing gate passed on a Linux runner against a baseline recorded on a Windows dev box. That is the headline. The numbers underneath it are the more useful result, and they are not the comfortable ones.

Reference: **572 cycles (Windows baseline) → 441 (CI)**, a runner 1.30x faster. Per-metric p50, baseline → observed:

| Metric | ratio shift | absolute shift | verdict |
|---|---|---|---|
| `D05/dispatch_select_highest_priority_ready` | **+80%** | 112 → 155 | ok, limit +100% |
| `D07/pool_u64x4_alloc_denied_exhausted` | +25% | 26 → 25 | ok |
| `D04/context_switch_yield_roundtrip_2switches` | −23% | 332 → 207 | ok |
| `D05/dispatch_run_once_cooperative_round` | −40% | 438 → 259 | ok |
| `D02/fault_ud2_capture_terminate_kernel_context` | −46% | 1174 → 467 | ok |

**Two findings, and both matter more than the green tick.**

1. **The ratios shifted across hosts by as much as the absolutes did.** Ratio spread across metrics is **3.36x** (0.535 → 1.799); absolute spread is **3.45x** (0.40 → 1.38). On this datum, normalising bought **essentially nothing** for cross-host transfer. Everything `STORY-P1-01-04` demonstrated remains true and remains about **same-host load**, which is what it was scoped to after correction — but nobody should carry away the impression that a ratio baseline is portable. It is not, on this evidence.

2. **`D05/dispatch_select` passed with 20 points of margin on unchanged code.** Had the tolerance been the 60% that was drafted before the quiet-to-loaded excursion was computed, **this push would have gone red** — the same failure mode, one turn later. The 100% constant is doing real work and should not be trimmed by anyone who thinks it looks loose.

**One thing that got better, unexpectedly.** `D07/pool_u64x64_alloc_free_round_trip` measured **25 cycles on CI**, not 0. `LE-24` is a property of the *Windows host*, where the operation costs less than the calibrated `rdtsc` overhead — on the Linux runner it is measurable and would be gateable. The ungating is a consequence of where the baseline was recorded, which ties `LE-24` directly to `LE-23` and means both should be examined by the same Story.

**Consequence for the agenda**: the `LE-23` Story is no longer "rule this out". It is "this is real, here are the numbers, choose a fix". The two candidate directions are unchanged and now have data behind them — re-record the baseline from a CI run, or add a **second reference of different composition** (memory-bound alongside the ALU-bound one) and normalise each metric against whichever it resembles. The second is more work and is the one the +80%/−46% split actually points at, since the sign of the shift tracks what the metric is made of.

## Start here (written pre-push, retained): push, and read the first CI run carefully

**This is a short first task with a specific trap, and it should be done before anything else is started.**

Nothing has been pushed, so `STORY-P1-01-04`'s central claim is untested. `LE-23` states it: **the committed ratios were recorded on a Windows dev host and have never met a Linux CI runner.** Ratios *should* transfer where absolutes never could — that is the whole reason the baseline is now a set of ratios — but it is a claim, not a result.

**If the gate goes red on that first push, read the reference metric's own cycle count before concluding anything about the code.** The gate prints it before any verdict, for exactly this reason:

```text
This run's reference measured p50=NNNN cycles — that is how fast the machine was,
and a ratio gate is what keeps that number out of the verdict.
```

Two distinguishable outcomes, and they need different responses:

- **Everything moves together and the reference moves with it** → the design is working and something else is wrong. Read the ratios.
- **The ratios themselves shift systematically between hosts** → `LE-23` is real, the baseline needs re-recording from a CI run, and that is a Story. Do **not** reach for `--update-baseline` locally to make it green; that reproduces exactly the mistake Handover 05 recorded, on a gate that has only just stopped making it.

**Expect the second one. Do not treat it as the surprise case.** Handover 15's four-quadrant table varies **load on one machine** and therefore does not test `LE-23` at all, and there is a named mechanism for cross-host failure: the reference is a dependent integer-multiply chain and is ALU-bound, while `D05/dispatch_select_highest_priority_ready` walks a ready queue and is memory- and branch-bound. A runner that scales ALU throughput and memory latency differently shifts that ratio with **no regression present**. The metrics closest to the reference in magnitude and composition — `D05/dispatch_run_once`, `D04/context_switch` — are the least exposed; the small and the fault-path metrics are the most.

So the `LE-23` Story should be **pre-scoped on the assumption that a systematic cross-host shift is what will be found**, and its job is to rule that out or to re-record from CI with the evidence attached. Candidate directions if it is real: record the baseline from a CI run rather than a dev box; or measure a *second* reference of a different composition (a memory-touching one alongside the ALU one) and normalise each metric against whichever it resembles. Neither should be chosen before the first run's data exists.

`--inject-regression` is the check that the gate is still awake, and it is cheap.

**Also fold into this push**: `CLAUDE.md` was untracked, which meant it did nothing for anyone who cloned — an entry point that looks solved locally and is absent everywhere else. It is now committed.

### After it, in order

- **The assurance-debt Story for `2da1ccd`'s tooling. Higher stakes than "cheapest item".** `xtask`'s `FIXTURES` table and `list-fixtures` command, and `goals/assurance/loose-ends.tsv` with its spine check, are both in `2da1ccd` and **neither has a `TEST-*` document**. Both are genuine improvements — the register in particular is far better as validated data than as Handover 09's prose. But shipping them this way **bypassed rule 3 (test-driven, no exceptions) and rule 8 (nothing bypasses the spine)**, and a bypass left standing licenses the next one. That is the actual cost, not the missing paperwork.

  Its Test document should assert the **deliberate-violation cases that already exist in the code**: register gaps, a loose end closed without evidence, an out-of-vocabulary status value, `Complete` not matching `Completely`, and the CI-drift guard that caught `--fixture=` having become two namespaces. Those checks were written and are unproven; a checker nobody has seen fail is `TEST-P1-04-02-A`'s lesson waiting to happen.

  **Bundle `broken-boot` and `idt-apic-unrouted`'s exit-code hole into this Story** — same `xtask`/CI shape, and it has now been deferred three times. Their documented pass condition is a failure exit code that any other failure also produces; `--serial-capture=` plus a grep, per the `wcet-trip` and `os-runaway` steps.
- **`LE-22`** — degrade and priority inheritance are unreconciled. `PriorityInheritingLock` restores a holder's pre-boost priority on unlock, so a degrade applied to a boosted holder is **silently undone**; and degrading a boosted task discards a boost a high-priority waiter depends on, reintroducing the inversion `STORY-P0-02-03` exists to prevent. More pressing than it looks, because the shipping image's own workload declares `Degrade`.
- **`FEAT-P1-05` / `FEAT-P1-06`** — the two proof Features, and `-06` is the Epic's flagship exit.
- **`FEAT-P9-01`'s two Stories** — the dump-scan audit and the staging-arena wipe. No hardware precondition. Pick these up whenever `EPIC-P1` stalls.

## The variance question, raised mid-session and not yet a work item

The question asked was: *for real-time systems we need to reduce variance — how do we make the system behave like solid-state infrastructure?* It is the right question and this session produced the first instrument capable of answering any part of it, but **no work has been scoped against it**. It deserves a decomposition rather than being absorbed piecemeal. Three things this session's data already says:

1. **Within a phase, this kernel is tight; across phases and runs, the environment is not.** A quiet-host reference run reports `p50=722 p99=734 p99.9=746 max=756` — a 1.05x spread over 1000 samples. The same run's `D05/dispatch_select` reports `p50=168 p99=176 p99.9=230 max=14728`. The p50s are stable and the **tails are where everything lives**, and at Tier 0 those tails are host preemption rather than kernel behaviour.
2. **The gate deliberately does not gate tails**, and for an RT system the tail *is* the product. `p99`/`p99.9`/`max` are printed and explicitly ungated because Tier 0 tail variance is 39–61%. That is a statement about QEMU, not about TinyOS, and it means **no evidence about this system's jitter exists yet** — which is a sharper argument for `LE-09`'s board than any latency number.
3. **The measurable variance sources in this codebase are already partly addressed and partly untouched.** Fixed-capacity `Pool` with no general heap, and a `Disposition`/`OverrunPolicy` decision table that reads exactly one declared input, are both variance-reducing by construction. Untouched: `dispatch::run_once_in_space` reloads `CR3` on every dispatch, and `STORY-P1-03-03` already measured that at ~276 cycles same-space against ~7,452 cross-space — a 27x jitter source on the dispatch path, currently ungated and attributed to TCG's TLB-flush emulation. Whether PCID/ASID removes it is a hardware question nobody can answer at Tier 0.

**Recommended shape, and it splits rather than blocking whole on `LE-09`** — the same separation `EPIC-P9` already uses, decomposing early precisely so the unblocked slice is separable from the gated one:

- **The gating half — a jitter metric (`p99.9/p50`, or `max − min`) gated the way latency now is — waits for the board.** Gating a tail at Tier 0 gates QEMU. No reservation about this.
- **The audit half — RT paths reviewed for data-dependent control flow — needs no hardware and can start now.** Error paths that cost differently from success paths, loops whose trip count depends on data, alternate flows taken only under contention.

**One concrete item for the audit half, verified in the source rather than inferred.** [`dispatch::switch_plan`](../../os/src/kernel/src/dispatch.rs#L63) takes only the target's address space:

```rust
pub const fn switch_plan(address_space: Option<u64>) -> SwitchPlan {
    match address_space {
        None => SwitchPlan::Plain,
        Some(cr3) => SwitchPlan::InstallAddressSpace(cr3),
    }
}
```

It has no knowledge of the currently-loaded `CR3`, so **any task with an address space gets `InstallAddressSpace` unconditionally** — a full TLB flush even when that same space is already live. Threading the current `CR3` in and returning `Plain` when unchanged is a small, host-testable, pure-function change, independent of PCID and independent of whether TCG's emulation reflects real silicon.

**The caveat is stronger than "depends on the dispatch pattern", and it is the reason this is a Story rather than a patch.** [`os/src/os/src/main.rs:934`](../../os/src/os/src/main.rs#L934) writes `SUPERVISOR_PML4` **unconditionally after every dispatch round**. So on the shipping path `CR3` provably alternates task → supervisor → task, the current-`CR3` check would hit **zero** times, and the pure-function change alone is **inert there**. It would pay only for callers that do not reinstate — the `FEAT-P1-04` fixtures dispatching the same task repeatedly — until the reinstatement is *also* made conditional.

And that reinstatement is not incidental: `TEST-P1-04-03-A` clause 4 asserts it, and Handover 13 records that removing it leaves the supervisor making its next scheduling decision under the workload's address space, survivable only because the image space attaches the shared kernel directories. **So the real item is a containment-versus-jitter trade on the dispatch path, not a free win.** Scope it as one. The ~276-vs-~7,452-cycle figure `STORY-P1-03-03` measured is the size of the prize and is itself Tier 0, so it is an argument for the board as much as for the change.

## CI

**Not run this session** — nothing was pushed. The last observed run is `30285899382` on `566d3e2`, every job green except the timing gate. **That step's history is now void as a reference**: it gated a different quantity, against a baseline file whose header no longer parses. Do not compare against it.

Two CI steps have never run: `TEST-P1-04-03-A` (the shipping image's runaway workload, from the previous session) and the re-named timing gate step, `TEST-P1-01-04-A`.

## How to verify you have a good starting state

```
cd os
cargo test --workspace                                    # 435 passing
cargo fmt --all -- --check
cargo run -p xtask --quiet -- check-assurance-spine        # 22/49/36/43, 24 loose ends (14 open)
cargo run -p xtask --quiet -- check-image-size             # os, 84,864 bytes
cargo run -p xtask --quiet -- check-timing-regression --runs=3
cargo run -p xtask --quiet -- check-timing-regression --runs=3 --inject-regression   # exit 1 is the PASS
cargo run -p xtask --quiet -- list-fixtures                # new this session, and undocumented by any Test
```

Then sweep the fixtures. `list-fixtures` now enumerates them, which is easier than the previous method of reading `main.rs`'s match arms. Every Tier 0 fixture should pass, with exactly three exceptions that are *supposed* to return exit 1: `broken-boot`, `idt-apic-unrouted` and `wcet-trip`.

To read any fixture's own diagnostic output, add `--serial-capture=PATH` — without it fixtures run with `-serial none` and report only a pass/fail bit.

**The lint command CI actually uses**, which is *not* `--workspace --lib --tests` and cannot be run as `--all-targets` on Windows:

```
cargo clippy -p <kernel|exec|os> --bins \
  --target targets/x86_64-tinyos.json -Zjson-target-spec \
  -Zbuild-std=core,compiler_builtins \
  -Zbuild-std-features=compiler-builtins-mem -- -D warnings
```

**Run it from bash, not PowerShell** — PowerShell mangles `-Zbuild-std=core,compiler_builtins` on the comma. Repeat it with `--features <the fixture>` for any fixture you touch, per `LE-12`; for the measurement work that means `fixture-measure` *and* `fixture-measure-regression`.

## The lesson this session earned

**A verdict is only as meaningful as the quantity it is computed from, and "the test is failing" is not evidence that anything is wrong.**

The timing gate was red for five sessions and two consecutive mandates named it the highest-value unowned work. The diagnosis — that it was measuring the runner rather than the code — had already been written down in Handover 10 and was correct. What it cost to leave was not just the false alarm: **`main`'s CI carried a red step for five sessions, and a red step nobody can act on trains everybody to stop reading it.** That is `LE-07`'s lesson arriving from a direction it had not been expected from.

A second, narrower lesson, and it bit twice this session: **check that your instrument measures what you think it does.** The reference workload's first draft optimised down to 16 cycles because the release profile closed-formed the recurrence, and the first analysis of the calibration data used a statistic the gate does not use and produced a conclusion that was wrong in a direction that would have narrowed the gate further than the evidence required. Both were caught by looking at a number that seemed off rather than by reasoning forward.

## Open items, by owner

The register is now [`goals/assurance/loose-ends.tsv`](../../goals/assurance/loose-ends.tsv) — **24 rows, 14 open**, validated by `check-assurance-spine`. Query it rather than trusting this summary.

**New this session:** `LE-23` (the ratios have never met a Linux runner), `LE-24` (a metric below the harness's own measurement floor).

**Restated, not closed:** `LE-16`, `LE-18` — the gate catches roughly 2x-or-worse ratio regressions at Tier 0, and no Tier 0 work improves that.

Still unowned and unscheduled from Handover 06, unchanged: clean task termination (`ExitProcess` routes into the capability trap — contained, but a fault rather than a teardown), real Win32 subsystems, loading from storage, a compiled probe, and the ungated `D04` baseline.
