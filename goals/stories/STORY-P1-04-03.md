# STORY-P1-04-03 — Preemption and WCET Enforcement on the Shipping Image

Status: **Verified** (Tier 0 + Host, 2026-07-28, [`REPORT-2026-07-28-05`](../reports/REPORT-2026-07-28-05.md); assurance `baseline-debt`)
Feature: [`FEAT-P1-04`](../features/FEAT-P1-04.md)
Introduced in: [`session/hand-2026-07-28/12-next-session-mandate.md`](../../session/hand-2026-07-28/12-next-session-mandate.md) — closing `LE-20`
Started: 2026-07-28

## Description

`STORY-P1-04-01` made dispatch preemptive and `STORY-P1-04-02` made the same tick enforce declared WCET budgets. **Neither runs in the binary this project ships.** `os` installs no `TickHook`, so the system image still dispatches cooperatively, charges nothing to any budget, and would let a workload that never yields keep the CPU forever. Two Verified Features currently describe mechanisms the shipping image does not have.

This Story puts the hook on the real boot path, and makes the embedded workload's overrun declaration a decision rather than a placeholder.

## Depends on

`STORY-P1-04-01` (the tick hook and the re-entrancy rule), `STORY-P1-04-02` (the enforcement path), `STORY-P1-03-03` (the `os` binary and its `run_once_in_space` dispatch round).

## Why this is a Story and not a refactor

Three of the four things it has to get right are *soundness* properties of the shipping image, not tidiness:

1. **The dispatcher must run with `IF` clear.** It holds `&mut Scheduler` while it selects and switches; the hook reads the same scheduler from an interrupt. A dispatcher that leaves interrupts enabled while holding that borrow is unsound, not merely racy. The rule `kernel::preempt` documents — *interrupts are enabled only while a task runs*, re-enabled by each task's own saved `RFLAGS` across the switch into it — has to hold in `os` exactly as it does in the fixtures.

2. **`os` dispatches through `run_once_in_space`, and the fixtures do not.** A preempting or enforcing tick switches into the dispatcher's suspended context, so control returns *inside* `run_once_in_space` — which does not reinstall the dispatcher's own `CR3`. Handover 06 named the dispatcher's non-restoration of its address space as a trap that becomes load-bearing under interrupt re-entry. Under this Story it stops being hypothetical: the supervisor would continue, and select the next task, with a task's address space live.

3. **The embedded workload's declared overrun policy is currently `TripToSafeState` with a 1,000-tick budget, chosen by nobody.** That declaration says a contained application that burns CPU may halt the entire system. It contradicts this project's own containment stance — `kernel::fault::Disposition::of` answers `TerminateTask` for a task-context fault and reserves `HaltSystem` for the kernel's own — and it contradicts `PD-07`/`PD-08` temporal isolation and `BND-15`, which exist precisely so no lower-criticality workload can deny the CPU to the rest of the system. Making that declaration deliberate is part of this work.

## Acceptance criteria (finalized 2026-07-28 at Story start)

1. **The shipping image installs a tick hook, before interrupts are armed.** `set_tick_hook` precedes `interrupts::init`, so no tick can arrive between arming and installation. The hook is the same one the shipping build ships — not a feature-gated variant.

2. **The hook holds the attribution rule, and holds it first.** The hook reads a dispatcher-owned "which task is on the CPU" cell *before it touches the scheduler at all*. That single check is simultaneously the `Nobody` arm of `wcet::attribute_tick` and the soundness precondition that makes it harmless for a tick to land in the dispatcher. The cell is written only by the dispatcher, only with interrupts disabled.

3. **The `os` dispatcher runs with `IF` clear**, and the image reports that it observed it clear on every round rather than asserting it in a comment. Each task re-enables interrupts across the switch into it through its own saved `RFLAGS`, and nothing in the dispatch loop re-enables them.

4. **The supervisor's address space is reinstalled after every dispatch round.** After control returns from `run_once_in_space` — whether the task yielded, was preempted, was enforced against, or was terminated by the fault path — the supervisor's own `CR3` is live again before the next selection.

5. **The workload's overrun declaration is a stated decision with a stated floor**, not a default, and it is the declaration the shipping image actually carries. Both the budget and the consequence are named constants with the reasoning recorded at the declaration site.

6. **All three disposition arms are implemented on the real boot path**, including the two this workload's declaration cannot reach. A shipping hook that handles only the arm its current workload declares is a hook that breaks the first time the declaration changes; the `match` is total and every arm does the caller's half of its contract (a `Restart` rebuilds the task's `Context` from its own entry point; a `TripToSafeState` enters the declared safe state and does not return).

7. **The boot path dispatches in a bounded loop rather than exactly once.** A preempted, restarted or degraded task is only meaningfully any of those things if it can be selected again. The bound is a property of a boot path carrying one embedded workload and no idle task, and is stated as such — it is not a scheduling policy.

8. **Enforcement on the shipping image is proven against a workload that will not give up the CPU**, loaded through the real PE64 loader into its own address space under the real capability gate — not a scenario assembled inside a fixture. The evidence is that the *same* binary, hook, dispatcher and declaration produce a detected overrun and the declared consequence, with the only difference between that run and the shipping run being which image is embedded.

9. **Enforcement charges nobody else.** The hook keeps its own count of the ticks it attributed, independently of the scheduler's books, and the image asserts the two agree task for task.

10. **No regression.** `--fixture=os`'s existing claims (`TEST-P1-03-03-A` clause 6) still hold verbatim: the granted `GetCurrentProcess` returns the correct pseudo-handle, the denied `ExitProcess` is contained at `iat::CAPABILITY_TRAP_VIRT`, the W^X audit finds no violation, and every other Tier 0 fixture is untouched.

## Explicitly out of scope

- **A measured latency for any of this.** `D03` still has no baseline; enforcement being *correct* on the shipping image and enforcement latency being *bounded in real time* are different claims, and only the first is made. `LE-09` stays open.
- **An idle task, or task termination as teardown.** `ExitProcess` still routes into the capability trap; that is `LE`-tracked and unchanged here.
- **Escalation on repeated overrun.** The kernel deliberately pins that a repeated overrun at the floor degrades again to no further effect. Inventing escalation in `os` would be scheduling policy nobody asked for.
- **Reconciling degrade with priority inheritance** (`LE-22`). Untouched.
- **The timing gate** (`LE-16` + `LE-18`). Untouched; still the highest-value unowned work.

## Tests

[`TEST-P1-04-03-A`](../tests/TEST-P1-04-03-A.md) — Tier 0 QEMU runs of the shipping image on both its shipping workload and a runaway one, plus host tests for the runaway image generator.

## Goals verified

G-RT-1 (on the shipping image), G-RT-3 (on the shipping image), G-SEC-14.
