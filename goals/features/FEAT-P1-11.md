# FEAT-P1-11 — The Kernel Drives the Board

Status: **In progress — added 2026-08-05. `STORY-P1-11-01` implemented and host-Green; no board evidence yet.**
Epic: [`EPIC-P1`](../epics/EPIC-P1.md) — Determinism Proof
Introduced in: `session/hand-2026-08-05/05A`, after a correction to the claim that the kernel had never run on silicon

## Why this Feature exists, stated against the correction that produced it

Two handovers asserted that "the board has never run TinyOS's kernel". **That was wrong.**
`kernel::fixture_measure_arm64` creates a task, switches context and runs a dispatch round on
the Pi 5 a thousand times per boot, and has since `BOARD VERDICT 5`; the numbers are in every
`TOS64-MEAS/2` envelope. `kernel::context` has carried an AArch64 context switch throughout.

What is genuinely absent is narrower: **the kernel runs, but it does not run the machine.**
Dispatch happens only inside `fixture_measure` — a timed region, interrupts masked, the whole
scheduler discarded when the fixture returns. The board then falls into `hal-arm64`'s park
loop, where the tick increments a counter and no task owns anything.

This Feature closes exactly that gap and claims nothing wider.

## Exit criteria

1. **A dispatch round runs outside any measured region**, from ordinary code, with interrupts live.
2. **It is paced by the tick and not called from the handler.** A context switch inside an interrupt handler swaps the stack underneath the frame that will `eret`, and the handler is not reentrant.
3. **Every round is observable.** A `Dispatch` spoor per round, including the rounds that dispatch nothing — a silent no-op is indistinguishable from success to a reader of the stream.
4. **The board evidence is a capture**, not an assertion: `Dispatch Kernel Select` records arriving from a running system.
5. **Nothing claims more than it is.** No preemption, no `EL0`, no protection domain, no second task, and no scheduling claim beyond "a round ran".

## Stories

| Story | What it does | Status |
|---|---|---|
| [`STORY-P1-11-01`](../stories/STORY-P1-11-01.md) | One task, dispatched from the park loop with interrupts live, stamping a `DispatchRound` spoor | In progress — host-Green 2026-08-05 (6 tests); image `f8133b0958d3` built and served; awaiting one power cycle |

## What this Feature deliberately does not do

- **No preemption.** Cooperative only. Preemptive scheduling is a different Story with its own hazard argument, and it needs the tick handler to be reentrant-safe first.
- **No `EL0` and no per-task address spaces.** The task runs in the kernel's own domain, so no containment evidence may be claimed for it.
- **No second task.** One is enough to make the claim true, and small enough that a misbehaviour on silicon has an unambiguous cause.

## Named debt

- **The task does no work**, deliberately — a task that computed something would put its own correctness between the claim and the evidence. That also means this Feature measures dispatch, not throughput.
- **The dispatch round's cost in the park loop is unmeasured.** `dispatch_run_once_cooperative_round` is measured inside the fixture with interrupts masked; the same round with interrupts live is a different number and is not yet taken.
