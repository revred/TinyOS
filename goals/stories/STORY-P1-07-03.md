# STORY-P1-07-03 — Flat Identity MMU with Normal Cacheable RAM: the Prerequisite of Measurement

Status: **In progress — host-testable half Green 2026-08-04 (259 host tests in `hal-arm64`, from 246, Red first: descriptor words, `MAIR`/`TCR` fields, the walk itself, the probe arithmetic and the report line all pinned); acceptance criteria 2, 3, 4 and 5 blocked on a board capture. The evidence channel is the canvas plus the `mmu-fault` fixture — serial has never produced a byte on this bench (`LE-47`). Not Verified.**
Feature: [`FEAT-P1-07`](../features/FEAT-P1-07.md)
Introduced in: [`session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md`](../../session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md) §4.1

## Description

The one piece of this slice that looks like scope creep and is not.

On AArch64, with `SCTLR_EL1.M == 0`, all data accesses behave as **Device-nGnRnE** regardless of what the memory actually is: uncached, unbuffered, no speculation, caches architecturally not consulted. A dispatch path measured in that state produces a number dominated by DRAM round-trips. It is not slow-but-proportional — it is **meaningless**, and it would silently poison every number this Feature exists to produce. This is the single most likely way for the slice to produce confidently wrong numbers.

So a minimal identity map with Normal Write-Back Cacheable attributes is a **prerequisite of measurement**, not a follow-on nicety.

It is emphatically **not** the `FEAT-P1-03` port: no per-task `TTBR0`, no `EL0`, no W^X, no teardown, no generation-safe frame reuse. One flat map, caches on, done. The `SEC-03` selection on this Story's contract row is scoped to that: the Story establishes translation, and explicitly does not claim per-process isolation, which arrives with the follow-on Feature and its own adversarial evidence.

## Depends on

`STORY-P1-07-02`. Debugging an MMU configuration on a board that cannot report a translation fault is precisely the failure shape the ordering exists to prevent — the first thing a wrong `TTBR0_EL1`, a wrong `MAIR_EL1` index or a wrong `TCR_EL1` granule produces is a translation fault, and without `-02` that fault is silence.

## Acceptance criteria

1. **An identity map covering RAM and the UART MMIO with correct attributes**: Normal Write-Back Cacheable, Inner-Shareable for RAM; Device-nGnRnE for MMIO. The `MAIR_EL1` indices, `TCR_EL1` granule/address-size fields, and the table itself are built by pure, host-tested code — descriptor construction is arithmetic and belongs on the dev host, not on the board.
2. **`SCTLR_EL1.M`, `.C` and `.I` are set**, with the required barriers and TLB invalidation around the transition.
3. **Acceptance requires evidence that caches are actually on.** The same measured loop, before and after enabling the MMU, showing the expected order-of-magnitude difference, both captures quoted in the Test document. **Without this the Story cannot distinguish success from a silently-ignored write** — and a silently-ignored write here is indistinguishable from success in every other respect, which is what makes this criterion the Story rather than a nicety attached to it.
4. **The MMIO mapping is verified by the UART still working after the switch.** If the UART goes silent at the moment the MMU comes on, the attributes for the device region are wrong; that is a diagnosable outcome and it is the reason the UART region is mapped explicitly rather than left to a blanket attribute.
5. **A deliberate translation fault reports through `STORY-P1-07-02`'s handler** with a decoded `ESR_EL1` naming the data-abort exception class. This closes the loop between the two Stories and proves the fault path survives the memory-system change that most easily breaks it.

## Named debt this Story leaves open

- **No per-task address spaces, no W^X, no teardown.** `FEAT-P1-03`'s port is a follow-on Feature; nothing here may be cited as isolation evidence.
- **No `EL0`.** Everything continues to run at `EL1`.
- `LE-09` stays open.

## Tests

[`TEST-P1-07-03-A`](../tests/TEST-P1-07-03-A.md) — written before implementation, per the TDD mandate.
