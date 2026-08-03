# STORY-P1-07-09 — The Firmware's Canvas: the Report Becomes Text on the Monitor

Status: **In progress — host half Green 2026-08-03 (RGB565 conversion pinned, the canvas surface bounds-honest, the font total over the report charset, the console composition pure); criterion 4 awaits the board. Not Verified.**
Feature: [`FEAT-P1-07`](../features/FEAT-P1-07.md)
Introduced in: the 2026-08-03 evening board session — the owner's order after four blink-counted diagnoses: "fix the display of the HDMI and not just an LED"

## Description

Every diagnosis tonight traveled through a one-bit channel because the
mailbox framebuffer ask goes unanswered on this firmware. The ground truth
shows what the mailbox was hiding: **the firmware already scans out a
framebuffer** — `simple-framebuffer at 0x3f800000, 0x3f4800 bytes,
r5g6b5, 1920x1080x16, linelength 3840` (captured live from Pi OS the same
evening, twice: the boot dmesg and `/sys/class/graphics/fb0`). The monitor
was never dark because there was nothing to show; it was dark because
nobody painted the canvas that was already there.

This Story paints it. The constants are hardcoded-and-verified like every
board fact in this slice (`BND-03` — still no device-tree parser; the
values cite the capture, and the painted screen is itself the on-silicon
verification). A new surface implements the existing pure `Surface` seam
over the RGB565 canvas, so every painter the splash already has works
unchanged; a full report font (A–Z, 0–9, and the report punctuation) joins
the six splash glyphs; and the park loop's composition writes what the
serial line has been saying into the void: the `TOS64-LINK/1` line as
text, the live heartbeat state, and any refusal spelled as a **readable
sentence** — `CODE 09 DETAIL 00002` — instead of counted blinks.

The lamp stays: it is the zero-dependency execution proof and the boxed
board's fallback. The mailbox path stays too, untouched, still validated
hostile; this Story adds the canvas beside it rather than replacing either.

## Depends on

- `STORY-P1-07-07` — the `Surface` seam, the pure painters, and the splash
  discipline (bounded, silent-continue, never evidence) this Story extends.
- `STORY-P1-09-07`/`-11` — the refusal vocabulary now rendered as text.

## Acceptance criteria

1. **The canvas constants are pinned against the capture.** Address, size,
   width, height, stride and the RGB565 format — each cited to the dmesg
   and sysfs lines; the color conversion (`u32` → r5g6b5) pinned as
   arithmetic at the corners (black, white, pure channels, the splash
   navy).
2. **The surface is bounds-honest.** Out-of-range pixels are ignored, the
   stride is respected exactly (a row never bleeds), and the whole-canvas
   clear touches every addressable pixel and nothing beyond the pinned
   size — proven on a host slice surface with the same geometry.
3. **The font and console are total over the report language.** Every
   character the report lines and spelled refusals can emit renders (case
   folded, unknowns as a visible block, never skipped); the console
   composition renders the `TOS64-LINK/1` bytes, the heartbeat state and
   the refusal sentence at pinned positions — pure over the seam.
4. **Board: the monitor speaks.** The next boot shows the title, the
   discovery report line and the live status as text on the connected
   display — the owner reads the diagnosis, nobody counts anything. This
   is also `STORY-P1-07-07`'s "dark on the proven chain" question answered
   by data: the canvas paints where the mailbox refused.

## Named debt this Story leaves open

- The canvas geometry is this board + firmware + display's, pinned; a
  different EDID or firmware split moves it, and the constants change with
  their citation (the same posture as every other board constant).
- No cursor, scrolling or input — a status panel, not a terminal.

## Progress, 2026-08-03

| Criterion | State |
|---|---|
| 1 — constants and conversion pinned | **Green.** Corners and citation asserted. |
| 2 — bounds-honest surface | **Green.** Host slice surface, same geometry: rejects, stride, full clear. |
| 3 — total font and console | **Green.** Report charset covered; composition pinned at positions. |
| 4 — board | **Blocked on the next power-on.** |

## Tests

[`TEST-P1-07-09-A`](../tests/TEST-P1-07-09-A.md) — written before
implementation, per the TDD mandate.
