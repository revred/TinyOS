# Handover 13 — `STORY-P1-04-03`: the shipping image preempts and enforces, and `FEAT-P1-04` exits for real

Written at the close of 2026-07-28. `STORY-P1-04-03` is Verified, `LE-20` is closed, and **`FEAT-P1-04` now meets its exit criteria on the binary this project ships** rather than on its fixtures.

## What changed

`os` installed no `TickHook`. For two Features this project had a Verified preemptive scheduler (`STORY-P1-04-01`) and a Verified WCET watchdog (`STORY-P1-04-02`), and the binary it ships ran neither: it ticked, counted the tick, signalled end-of-interrupt, and did nothing. Dispatch was cooperative, no budget was ever charged, and a workload that never yielded kept the CPU until the machine was reset.

That was `LE-20`. It is now closed, and the shipping image:

- installs its hook **before** `interrupts::init` arms the timer;
- dispatches in a bounded loop with `IF` clear, reading the flag back from `RFLAGS` on every round rather than asserting it in a comment;
- charges every tick to whoever was on the CPU, checking the dispatcher-owned current-task cell *before* it touches the scheduler at all;
- reinstalls the supervisor `CR3` after every round;
- implements all three disposition arms, including the two its own declaration cannot reach;
- and declares a real overrun policy for its workload.

Read [`goals/tests/TEST-P1-04-03-A.md`](../../goals/tests/TEST-P1-04-03-A.md) for the captures and [`goals/reports/REPORT-2026-07-28-05.md`](../../goals/reports/REPORT-2026-07-28-05.md) for the findings. This document is the short version plus the loose-ends delta.

## The finding worth carrying: `LE-20` was not a wiring gap

The falsification that removes `set_tick_hook` and changes nothing else does not produce a failed assertion against a workload that will not yield. It produces:

```text
xtask: kernel did not reach the isa-debug-exit port within the 15s boot time budget
```

The shipping image, holding a workload the capability gate had already admitted, had no way to take the CPU back. Twenty-four bits of machine code — `EB FE`, `jmp $` — inside an image that passes every load-time check this system has, and the system stops.

`LE-20` was recorded and carried for two Features as *"proven in a fixture, not on the real boot path"*, which reads like a documentation defect. It was a denial-of-service hole in the shipping image, and no existing evidence would have shown it, because nothing ran the shipping binary against a workload that declined to yield. **The register's wording made it sound smaller than it was**, and that is the thing to generalise: an entry that says "the mechanism exists but is not wired up" is describing the *absence* of a mechanism in whatever ships, and should be read as one.

## Two more findings

**The `CR3` trap Handover 06 named was real, and only the shipping image could expose it.** `os` dispatches through `run_once_in_space`, which installs the selected task's `CR3` and does not restore the caller's on the way back; the `FEAT-P1-04` fixtures use `run_once`, which touches no `CR3` at all. With the reinstatement removed, the shipping arm reports `cr3_after_round=0x147000` — the *image's* `PML4`. The supervisor was running, and would have made its next scheduling decision, under the workload's address space. It is survivable rather than fatal, because the image space attaches the shared kernel directories, which is exactly what makes it the kind of containment defect that survives a green test run.

**Counting serviced ticks is not evidence that a hook is installed.** Clause 1 was first written to assert `ticks_unattributed >= 1`. Two consecutive runs of identical binaries reported `ticks_attributed=[1,0,0,0] unattributed=1` and then `ticks_attributed=[0,0,0,0] unattributed=2`: the total held, the split moved, and both are a function of how long boot took relative to a ~16ms tick. On a faster host the total could be zero and a hook-less build would look identical. `interrupts::tick_hook_installed()` reads the pointer back instead. **This test very nearly acquired a second instance of `LE-16`/`LE-18`'s failure mode** — a gate whose verdict depends on host speed — and it was caught by re-running the same binary rather than by reasoning.

## Design decisions that should not be re-litigated

- **The workload declares `Degrade(PRIORITY_MIN)`, not `TripToSafeState`.** The old declaration let a contained, capability-mediated application halt the whole system for burning CPU — a *strictly more severe* consequence than the same system gives the same task for a genuine CPU fault, where `Disposition::of` answers `TerminateTask` and reserves `HaltSystem` for the kernel's own context. It also handed that application precisely the denial of service `PD-07`/`PD-08` and `BND-15` exist to deny. `TripToSafeState` stays right for a task whose *failure* is a system-level event; this workload is not one, and that distinction is why the policy is per-task.
- **The runaway workload is a real PE64, not a fixture scenario.** Generated by the same `probe_pe` module, admitted by the same `check_imports` gate, mapped by the same `AddressSpace::create`, patched by the same `iat::patch_imports`. A host test asserts it differs from the shipping workload **only** in `.text`. That is what makes the run evidence about the shipping binary; `TEST-P1-04-02-A`'s lesson was that a fixture which constructs part of the effect it tests will pass without the part it is meant to test, and here nothing is reconstructed.
- **The dispatch loop's round bound is not a scheduling policy.** It is a property of a boot path with one embedded workload and no idle task. Reaching it is reported (`rounds_exhausted`), never silently swallowed.
- **All three disposition arms are implemented even though the declaration reaches one.** A hook that handles only its current workload's arm breaks the first time the declaration changes.
- **No escalation on repeated overrun.** The kernel deliberately pins that a repeated overrun at the floor degrades again to no further effect; inventing escalation in `os` would be scheduling policy nobody asked for.

## Loose-ends delta

**Closed:** `LE-20`.

**New:** none.

**Still open and unchanged:** `LE-03`, `LE-08`, `LE-09`, `LE-10`, `LE-11`, `LE-12`, `LE-15`, `LE-16`, `LE-18`, `LE-19(b)`, `LE-21`, `LE-22`.

`LE-12` got another concrete data point in its favour: the per-binary target clippy was run with `--features fixture-os-runaway`, per Handover 11's lesson. It was clean this time, but the command CI runs still would not have covered it.

## What I did not do, and why

**The timing gate (`LE-16` + `LE-18`) is untouched**, and it is now the oldest live regression in the register — red on `main` for five sessions. I judged `LE-20` to be the mandate's explicit instruction and the gate to be a session's work in its own right (it needs a Story, a Test document, and a deliberately-injected regression the new gate still catches). Handover 10 §"The one regression this session leaves behind" is still the best statement of it, and the caution against `--update-baseline` without fixing the methodology first still stands.

**`broken-boot` and `idt-apic-unrouted` still have the exit-code hole `wcet-trip` closed.** The new `os-runaway` CI step does *not* have it — it greps its serial capture for `priority_after_enforce=Some(0)` and `hook_installed=true` rather than trusting the exit code — but the two older fixtures were not revisited.

**`LE-22` is untouched.** This image takes no locks, so nothing here bears on it.

## State of the tree

**Not committed.** Everything is in the working tree; `git status` is dirty and nothing has been branched, committed or pushed. `main` is at `6b47b4d`.

Verification at the close, all green:

- **399 host tests** (395 before; 4 added — all in `xtask`'s `probe_pe`).
- `cargo fmt --all -- --check`, `cargo clippy --workspace --lib --tests -- -D warnings`.
- The per-binary target clippy for `kernel`, `exec` and `os`, **plus** `os` again with `--features fixture-os-runaway`.
- `check-assurance-spine` (22 Features / **48** Stories / **35** Tests / **42** Reports), `check-crate-sizes`, `check-performance-catalogue`.
- `check-image-size`: `os` is now **84,864 bytes** (74,568 before — the hook, the loop and the reporting). Ceiling is 8 MiB.
- **Every Tier 0 fixture re-run**, all 23. Exit 0 except `broken-boot`, `idt-apic-unrouted` and `wcet-trip`, each of whose documented pass condition is a distinguishable failure.

`check-timing-regression` was **not** run locally and is not evidence for this Story either way, for the same reason as last session: `fixture_measure` arms no timer, so none of the gated paths take a tick.

## A note on dates

**This session's work is in the `hand-2026-07-28` folder, and the actual calendar date was 2026-07-27.** So is every commit in this folder's history — `6b47b4d`, `566d3e2`, `cd67b5b` and the rest are all timestamped 27 July. The repository's document dates have been running **one day ahead of the clock** since at least the session that created this folder, and `REPORT-2026-07-28-01` through `-05` carry that same offset.

These handovers were first written into a `hand-2026-07-29` folder, on the reasoning that the document timeline should stay ordered. That was the wrong call: [`session/README.md`](../README.md) says "one folder per calendar date", and following the documents rather than the clock would have made the drift **+2 days** instead of +1. They were folded back into this folder as Handovers 13 and 14, and the Report renamed `REPORT-2026-07-29-01` → `REPORT-2026-07-28-05`, so this session carries exactly the same offset as everything around it and no new one.

**The offset itself is not fixed**, and fixing it would mean re-dating this whole folder plus four Report filenames woven into the assurance spine — 137 references across 29 files, rewriting a historical record `session/README.md` says is never edited. It is recorded here rather than repaired. **Nobody should treat a Report's date as evidence of when anything happened** until someone decides which clock is authoritative and does that cleanup deliberately.

## Standing constraints — unchanged, do not relax

- **TDD.** Test document when a Story starts. This Story held to it without the qualification `TEST-P1-04-02-A` had to record: `TEST-P1-04-03-A`'s clauses and its `MAX_TICKS_TO_ENFORCE = 1` bound were written before any implementation code, and the host seam (`probe_pe::build_runaway`) was driven Red-to-Green in the ordinary way — the tests were written against a function that did not exist and the compiler said so.
- **Tier 0 is not hardware evidence.** `LE-09` remains open. `D03` still has no measured baseline: enforcement being *correct* on the shipping image and enforcement latency being *bounded in real time* are different claims, and only the first is made.
- **Never call TinyOS a hobby OS.** It targets data-centre, local-AI, UAV, medical, edge and consumer deployment.
- **No assurance state may be claimed beyond its evidence.** All 36 functionally Verified Stories are still `baseline-debt`. The first `baseline-debt → verified` conversion is `EPIC-P1`'s explicit charge and has not happened.
- **Run the target clippy command from bash, not PowerShell** — PowerShell mangles `-Zbuild-std=core,compiler_builtins` on the comma. And repeat it with `--features <the fixture>` for any fixture you touch.
