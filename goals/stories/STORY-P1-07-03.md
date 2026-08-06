# STORY-P1-07-03 — Flat Identity MMU with Normal Cacheable RAM: the Prerequisite of Measurement

Status: **Verified (functional) 2026-08-05 — host-testable half Green 2026-08-04 (259 host tests in `hal-arm64`, from 246, Red first: descriptor words, `MAIR`/`TCR` fields, the walk itself, the probe arithmetic and the report line all pinned). **Acceptance criteria 2, 3 and 4 Green on silicon** — `BOARD VERDICT 5` (measure boot 2026-08-04, kernel `0c709197ed26`) recorded `TOS64-MMU/1 sctlr=0000000030D01805 off=75213055 on=183180`: `M` (bit 0), `C` (bit 2) and `I` (bit 12) all set, read back from the register rather than assumed from the write, and the same loop over the same memory 410× faster with caches on than off. That ratio is the Story's whole argument in one pair of numbers, and it is what licenses every measurement `STORY-P1-07-06` takes above this line to be about TinyOS rather than about the bus. **Criterion 5 Green on silicon** — `BOARD VERDICT 8` (mmu-fault boot 2026-08-04, kernel `fde0f2ce3f91`) recorded `ESR=0x96000005` decoding to `EC=0x25` (data abort with no exception-level change), `DFSC=0b000101` (translation fault, level 1), `WnR=0` (read), and `FAR=0x20_0000_0000` — the unmapped guard address to the bit. The flat identity map does not cover it, the hardware said so, and the OS could name which address and why. `HALTED REASON=NO-RESUME-PATH` is the fail-safe rule on silicon: slot 4 has no resume path, so the handler reported and stopped rather than looping on the faulting instruction. **Every acceptance criterion of this Story now has silicon evidence.** The evidence channel is the canvas plus the `mmu-fault` fixture — serial has never produced a byte on this bench (`LE-47`).

**Advanced to `Verified` (functional) 2026-08-05** in [`06A`](../../session/hand-2026-08-05/06A-nothing-is-verified-and-the-reason-is-not-velocity.md) §4.1's closing pass, after two things the header had been asserting were actually checked rather than inherited. **Criterion 3 said "both captures quoted in the Test document" and they were not**: `TEST-P1-07-03-A`'s capture section still read *Pending* while this header claimed the criterion Green, so the Story's own Test document contradicted it. The `SCTLR`/`OFF`/`ON` line and the decoded fault frame are now quoted verbatim there, which is what the criterion asked for. **And criterion 4 was not met as written and could not be**: it asks that the MMIO mapping be "verified by the UART still working after the switch", and the UART has never worked on this bench at all — a channel that was never alive cannot be shown to survive anything, so the clause was permanently unmeetable rather than merely unmet, and the 2026-08-04 amendment had substituted the evidence channel for the other clauses without touching this one. `TEST-P1-07-03-A` now records the substitution with its reason: the clause's purpose is that a wrong Device-nGnRnE attribute makes a device stop answering the moment the MMU comes on, and two device regions demonstrably keep answering after the switch — the STAT GPIO block (the lamp toggles in the park loop, which is after the switch) and the RP1 window at `0x1F_0000_0000` (`ID=0x0109 PHY=0x600D84A2` read through it, with `BOARD VERDICT 1`'s `0xDEADDEAD` as the counter-example proving an unreadable window presents loudly). **What that substitution does not establish is stated in the Test document and is not claimed here**: whether the PL011's own mapping is correct is still unknown, and stays with `LE-47`. **Assurance state remains `specified` and this Story is NOT release-assured** — 0 qualified platforms, so assurance `verified` is closed to every Story in this project (`06A` §2).**
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
