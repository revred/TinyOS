# STORY-P1-07-08 — The Lamp: Execution Announces Itself Before Anything Else Is Trusted

Status: **Verified (functional) 2026-08-05 — every criterion Green 2026-08-03: the lamp pulsed through the boxed case the same evening, the first observable effect TinyOS has produced on this board. Polarity measured active-HIGH on silicon (the confession pattern's dark gap read bright), overriding the debug listing's active-low; constant, tests and Test doc amended that evening, the measurement governing — a measurement overriding a vendor listing, with the document corrected the same night rather than the discrepancy carried. Criterion 4's "held on during boot and blinking at 1 Hz once parked" has been re-observed on every subsequent boot including 2026-08-05's netbooted run, so this is a standing property rather than one evening's result; the lamp additionally carried `STORY-P1-09-07`'s and `-11`'s blink sentences, which is the same device proven to do more than pulse. Advanced under [`06A`](../../session/hand-2026-08-05/06A-nothing-is-verified-and-the-reason-is-not-velocity.md) §4.1: "Not Verified pending the assurance pass" conflated the two ceilings `06A` §2 separates — the assurance pass is the `specified` → `verified` ladder, which is a different rung from this functional header and is closed to every Story in the project regardless. **Assurance state remains `specified` and this Story is NOT release-assured**: 0 qualified platforms.**
Feature: [`FEAT-P1-07`](../features/FEAT-P1-07.md)
Introduced in: [`session/hand-2026-08-03/07A-instruments-proven-os-indicted.md`](../../session/hand-2026-08-03/07A-instruments-proven-os-indicted.md) §"The two charges against TinyOS" — charge 1, execution itself unproven, promoted to a Story by the owner's order the same evening

## Description

Every observable this project has pointed at the board goes through a
peripheral that might itself be the fault: the UART through an adapter that
has produced five zero-byte captures, the splash through a mailbox exchange
the firmware may refuse, the beacon through a PCIe link, a reset line and a
PHY. Tonight's ground truth (`goals/reports/pios-ground-truth-2026-08-03.txt`)
removed that excuse for exactly one device: the board's ACT LED is
`2712_STAT_LED`, pin 9 of the BCM2712's own `gpio-brcmstb` bank at
`0x10_7D51_7C00` — **on the SoC side, behind nothing**. No PCIe
gates, no window validation, no firmware negotiation; the same class of MMIO
write the UART already uses, minus the UART.

This Story drives that pin twice. At `entry`, before the UART is configured
and before any register value is believed, the LED is forced on: a lit lamp
is the claim "the firmware entered our image and our first stores executed",
made with the fewest possible dependencies. In the park loop it toggles at
1 Hz beside the other channels: a blinking lamp is the claim "the park loop
is still scheduling its ticks", visible from across the room on a board whose
serial is dead and whose display path is still on trial.

The LED is an instrument, never evidence: like the splash, nothing here may
perturb a byte of the serial protocol, and no capture or timing claim ever
cites it. It answers one question — *did our code run at all* — that 07A
showed no existing channel can answer when every channel fails at once.

## Depends on

- `STORY-P1-07-01` — the entry stub this Story instruments; the LED write is
  placed immediately after the `CurrentEL` read that stub deliberately makes
  its first act.

## Acceptance criteria

1. **The transcriptions are pinned with their sources.** The GPIO block base,
   the bank-0 `DATA`/`IODIR` offsets, pin 9, and the polarity are asserted
   in pinning tests citing the on-silicon evidence
   (`pios-ground-truth-2026-08-03.txt`: `rpi-gpiomem` window base and span,
   the `/sys/kernel/debug/gpio` line naming pin 9 `ACT`) and the
   `gpio-brcmstb` register layout. Polarity is active-HIGH, pinned to the
   confession boot's bright-gap observation, which overrode the listing.
2. **Drive and toggle are exact RMWs.** Direction-to-output clears exactly
   the pin's `IODIR` bit; on/off and toggle touch exactly the pin's `DATA`
   bit; every other bit of both registers is preserved as found — pinned
   against hostile readbacks by a recording seam double.
3. **Placement is asserted.** At entry the LED turns on before the UART is
   configured; in the park loop it toggles once per second beside the
   heartbeat, and a park loop whose counter sticks freezes the lamp rather
   than hanging on it — the LED adds no wait of its own.
4. **Board: the lamp lights.** A power-on with this image shows the ACT LED
   held on during boot and blinking at 1 Hz once parked — the first
   observable effect TinyOS has ever produced on this board, ending 07A's
   charge 1 permanently.

## Named debt this Story leaves open

- The LED's state at firmware handoff is unrecorded (the EEPROM uses it
  during boot); the design is state-agnostic — force-on then toggle — so no
  assumption about the inherited state is made or needed.

## Progress, 2026-08-03

| Criterion | State |
|---|---|
| 1 — transcriptions pinned | **Green.** `stat_led.rs`: base, offsets, pin and polarity asserted with the capture cited line-by-line. |
| 2 — exact RMWs | **Green.** Recording-double tests prove single-bit discipline against hostile readbacks in both registers. |
| 3 — placement | **Green.** Entry order and park-loop cadence pinned; the lamp shares the loop's existing bounded wait and adds none. |
| 4 — board: the lamp lights | **Green.** Pulsing observed through the boxed case ~20:25, 2026-08-03 — execution proven by eye; 07A's charge 1 discharged. The same session's confession boot corrected the polarity to active-high. |

## Tests

[`TEST-P1-07-08-A`](../tests/TEST-P1-07-08-A.md) — written before
implementation, per the TDD mandate.
