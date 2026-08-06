# STORY-P1-11-01 — One Round, From the Park Loop, With Interrupts Live

Status: **Verified (functional) 2026-08-05 — all seven acceptance criteria met. Criteria 1–6 host-Green 2026-08-05 (6 tests in `kernel::board_dispatch`). **Criterion 7 met on silicon 2026-08-05** (`BOARD VERDICT 14`, image `f8133b0958d3`): `Dispatch Kernel Select Ok rung=DispatchRound` is on the wire **one per beat, in every frame**, read unelevated by Ti64Dink through Npcap — 100 records, 0 refused, 0 lost, span 1638..1738 continuous. This is the kernel driving the machine with interrupts live, which every prior board verdict could not show: `fixture_measure` has run on silicon since `BOARD VERDICT 5` but always from inside a region with IRQs masked, and criterion 1's whole point is that `tinyos_dispatch_init` runs *after* `daifclr` so the first round meets a machine with interrupts live. **The board was netbooted with no SD card in it** — the card was on the laptop for the entire run, so the firmware had no fallback and the boot proves the netboot path rather than leaving which-path-won unverified. **Two things this does not establish, stated because the capture invites both.** Every observed round returned `Ok`, so criterion 3's `Skipped` and `Failed` arms remain **host-only** — no board round has yet dispatched nothing, and `TEST-P1-11-01-A` clause 3 is the clause the test exists for. And `cost=0` is the **dispatched task index**, not a cycle count: the round's cost with interrupts live is unmeasured and stays this Story's named debt, a different number from the fixture's masked one. Getting here also cost two host-tool defects, both recorded in `BOARD VERDICT 14` and as loose ends: `ti64dink --until rung=DispatchRound` — the command [`hand-2026-08-05/07A`](../../session/hand-2026-08-05/07A-one-story-moved-and-the-gate-that-found-two-more.md) §1 printed as the next session's first act — watched 300 seconds of `DispatchRound` records and exited 1 as a timeout, because two separate hand-kept host tables had never learned the rung existed. **Assurance state remains `specified` and this Story is NOT release-assured**: 0 qualified platforms ([`06A`](../../session/hand-2026-08-05/06A-nothing-is-verified-and-the-reason-is-not-velocity.md) §2).**
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
