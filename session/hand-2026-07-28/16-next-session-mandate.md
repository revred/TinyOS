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

## Start here: push, and read the first CI run carefully

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

`--inject-regression` is the check that the gate is still awake, and it is cheap.

### After it, in order

- **The concurrent work needs a Story.** `xtask`'s `FIXTURES` table and `list-fixtures` command, and `goals/assurance/loose-ends.tsv` with its spine check, are both in `2da1ccd` and **neither has a `TEST-*` document**. Both are genuine improvements — the register in particular is better as validated data than as Handover 09's prose — and both currently sit outside the assurance discipline everything else in this repository is held to. This is the cheapest item on the list and the one most likely to be forgotten.
- **`LE-22`** — degrade and priority inheritance are unreconciled. `PriorityInheritingLock` restores a holder's pre-boost priority on unlock, so a degrade applied to a boosted holder is **silently undone**; and degrading a boosted task discards a boost a high-priority waiter depends on, reintroducing the inversion `STORY-P0-02-03` exists to prevent. More pressing than it looks, because the shipping image's own workload declares `Degrade`.
- **`broken-boot` and `idt-apic-unrouted`** — both still have the exit-code hole `wcet-trip` closed: their documented pass condition is a failure exit code that any other failure also produces. `--serial-capture=` plus a grep, per the `wcet-trip` and `os-runaway` CI steps. Cheap, and now deferred three times.
- **`FEAT-P1-05` / `FEAT-P1-06`** — the two proof Features, and `-06` is the Epic's flagship exit.
- **`FEAT-P9-01`'s two Stories** — the dump-scan audit and the staging-arena wipe. No hardware precondition. Pick these up whenever `EPIC-P1` stalls.

## The variance question, raised mid-session and not yet a work item

The question asked was: *for real-time systems we need to reduce variance — how do we make the system behave like solid-state infrastructure?* It is the right question and this session produced the first instrument capable of answering any part of it, but **no work has been scoped against it**. It deserves a decomposition rather than being absorbed piecemeal. Three things this session's data already says:

1. **Within a phase, this kernel is tight; across phases and runs, the environment is not.** A quiet-host reference run reports `p50=722 p99=734 p99.9=746 max=756` — a 1.05x spread over 1000 samples. The same run's `D05/dispatch_select` reports `p50=168 p99=176 p99.9=230 max=14728`. The p50s are stable and the **tails are where everything lives**, and at Tier 0 those tails are host preemption rather than kernel behaviour.
2. **The gate deliberately does not gate tails**, and for an RT system the tail *is* the product. `p99`/`p99.9`/`max` are printed and explicitly ungated because Tier 0 tail variance is 39–61%. That is a statement about QEMU, not about TinyOS, and it means **no evidence about this system's jitter exists yet** — which is a sharper argument for `LE-09`'s board than any latency number.
3. **The measurable variance sources in this codebase are already partly addressed and partly untouched.** Fixed-capacity `Pool` with no general heap, and a `Disposition`/`OverrunPolicy` decision table that reads exactly one declared input, are both variance-reducing by construction. Untouched: `dispatch::run_once_in_space` reloads `CR3` on every dispatch, and `STORY-P1-03-03` already measured that at ~276 cycles same-space against ~7,452 cross-space — a 27x jitter source on the dispatch path, currently ungated and attributed to TCG's TLB-flush emulation. Whether PCID/ASID removes it is a hardware question nobody can answer at Tier 0.

**Recommended shape**: a Feature under a new or existing Epic whose exit criterion is a *jitter* metric — `p99.9/p50`, or `max − min` — gated the way latency now is, plus an audit of the RT paths for data-dependent control flow. It should not start before `LE-09`, because gating a tail at Tier 0 gates QEMU.

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
