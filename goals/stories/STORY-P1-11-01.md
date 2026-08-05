# STORY-P1-11-01 — One Round, From the Park Loop, With Interrupts Live

Status: **In progress — implemented and host-Green 2026-08-05 (6 tests in `kernel::board_dispatch`). Image `f8133b0958d3` built, every gate green, staged and served over netboot. **No board evidence: the run needs one power cycle.****
Feature: [`FEAT-P1-11`](../features/FEAT-P1-11.md)
Introduced in: `session/hand-2026-08-05/05A`

## Description

`kernel::board_dispatch` holds a `Scheduler`, one task, its context and its stack as statics,
and exposes two `extern "C"` symbols across the same seam the spoor stream already uses —
`hal-arm64` cannot import `kernel`, because on AArch64 the dependency runs the other way.

`tinyos_dispatch_init` creates the task **after `daifclr`**, so the first round runs on a
machine with interrupts live rather than a masked one. That ordering is the entire difference
between this and what `fixture_measure` has done on silicon since `BOARD VERDICT 5`.

`tinyos_dispatch_round` runs one cooperative round from the park loop, once per beat.

**Paced by the tick, not called from it.** A context switch inside an interrupt handler swaps
the stack underneath the frame that will `eret`, and the handler is not reentrant — a second
tick arriving mid-switch is a fault with no resume path. The park loop already runs at the
beat, so the beat paces the round and the switch happens on the park stack as ordinary code.

## Acceptance criteria

1. **The task is created after interrupts are unmasked**, and the ordering is visible in the boot path rather than implied.
2. **A round runs from the park loop**, outside any measured region and outside any handler.
3. **Every round stamps a spoor**, including rounds that dispatch nothing — `Skipped` when the scheduler is empty, `Failed` when a round returns no task or the task comes back `Running` rather than `Ready`.
4. **`Dispatch`/`Select` is the taxonomy**, matching what the x86_64 path already stamps, so one host decoder reads either architecture.
5. **Re-initialisation is refused, not repeated.** A second `init` would rebuild a context that may be suspended mid-switch.
6. **The sentinel cannot collide with a task index.** `NO_TASK` is `u16::MAX`, pinned on both sides of the seam by a parity test.
7. **Board evidence**: a capture carrying `Dispatch Kernel Select Ok rung=DispatchRound` from a running system.

## Named debt

- **The dispatch round's cost with interrupts live is unmeasured**, and is a different number from the fixture's masked one.
- **One task, no preemption, no `EL0`, no containment claim.**
- The task yields immediately and does no work, so this measures dispatch and not throughput.

## Tests

[`TEST-P1-11-01-A`](../tests/TEST-P1-11-01-A.md)
