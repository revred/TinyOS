# STORY-P1-09-09 — The Window Is Programmed, Not Presumed

Status: **In progress — host half Green 2026-08-03 (the five-register mapping pinned against the on-silicon capture, window-class refusals program once and revalidate, link-class refusals never write, the second verdict is final); criterion 4 awaits the board. Not Verified.**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: the 2026-08-03 second-look boot — the lamp counted **4**: `DL_ACTIVE` now clears (the `STORY-P1-09-08` re-probe or a faster settle) and the outbound window's CPU base is the next refused rung

## Description

The confession advanced one rung: the data link is up, and the window
validation now refuses — the firmware keeps the PCIe link but does not
leave outbound window 0 mapping CPU `0x1F_0000_0000` onto PCI `0x0`. The
ground-truth capture (`pios-ground-truth-2026-08-03.txt`) shows exactly what
the working system does about it: Linux programs the window itself from the
device tree's `ranges` (`MEM 0x1f00000000..0x1ffffffffb -> 0x0`). This Story
does the same, at bring-up size.

The posture is unchanged from `STORY-P1-09-01`: the window is believed only
from readback. When the probe's link gates pass but its window validation
refuses, the five `WIN0` registers are written with the recorded mapping and
the probe validates *again* — the second verdict is final, whatever it says.
A link-class refusal never writes anything: programming a window on a dead
controller is exactly the kind of hopeful write this Feature's hostile-input
posture exists to forbid. The write-then-verify pair happens inside each
probe pass, so the `STORY-P1-09-08` re-probe cadence keeps working unchanged
if the controller needs settle time before accepting the mapping.

## Depends on

- `STORY-P1-09-01` — the probe and window validation this Story extends
  with a programming fallback.
- `STORY-P1-09-08` — the re-probe cadence that gives each pass its chance.

## Acceptance criteria

1. **The mapping is pinned against the capture.** The five register values
   (`WIN0_LO`/`HI` zero, `BASE_LIMIT`, `BASE_HI`, `LIMIT_HI`) decode — via
   the existing pinned decoder — to CPU base `0x1F_0000_0000`, PCI `0x0`,
   and a limit covering the peripheral span; asserted as arithmetic citing
   the dmesg `ranges` line.
2. **Only window-class refusals program, and exactly once per pass.** A
   window-base/-pci/-span refusal writes the five registers once and
   revalidates; every link-class refusal (`PortNotRc`, `PhyDown`,
   `LinkDown`) returns without a single write — pinned with a panicking
   write path.
3. **The second verdict is final.** A window that still refuses after
   programming reports that refusal (with its readback) and is not written
   again within the pass; belief never comes from the write, only from the
   re-read.
4. **Board: the confession moves past 4.** The next boot's lamp either
   reaches the plain pulse (window programmed, identity read, PHY found —
   the NIC watch takes the story) or counts a rung beyond the window —
   either way the window rung is closed on silicon.

## Named debt this Story leaves open

- The programmed mapping is the capture's, hardcoded-and-verified like
  every other board constant (`BND-03` — still no DT parser in this slice).

## Progress, 2026-08-03

| Criterion | State |
|---|---|
| 1 — mapping pinned | **Green.** Decode-back assertion cites the dmesg line. |
| 2 — window-class only, once | **Green.** Link-class refusals proven write-free; window-class writes pinned in order. |
| 3 — second verdict final | **Green.** Still-refusing double: one write burst, final honest report. |
| 4 — board | **Blocked on the next power-on.** |

## Tests

[`TEST-P1-09-09-A`](../tests/TEST-P1-09-09-A.md) — written before
implementation, per the TDD mandate.
