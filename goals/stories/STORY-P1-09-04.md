# STORY-P1-09-04 — The Release: the PHY Exists Only After Software Lets It Out of Reset

Status: **Verified (functional) 2026-08-05 — all four acceptance criteria met. Criteria 1, 2 and 3 host half Green 2026-08-03 (sequence pinned and glitch-ordered, addresses derived-and-pinned, stuck-counter aborts, pipeline placement asserted). **Criterion 4 Green on silicon 2026-08-04**, both halves of it: the laptop's link watch logged `01:27:03.446 TRANSITION Down -> UP at 1000 Mbps` under TinyOS (`BOARD VERDICT 2`) — the first Ethernet link this project has ever trained — and the same boot's canvas read `TOS64-LINK/1 RP1=PRESENT ID=0x0109 PHY=0x600D84A2`, the BCM54213PE identifier pair at address 1 where a week of `phy=absent` had been. The criterion's second half is written "serial permitting", and serial has never produced a byte on this bench (`LE-47`), so the readback is quoted from the canvas — the same `TranscriptSink` bytes the UART would have carried. Two further boots trained the wire in a row (`BOARD VERDICT 3`, `4`), so this is a repeated observation and not one lucky negotiation. **`LE-68` closes on this criterion**, which the Story names as its purpose. **Assurance state remains `specified` and this Story is NOT release-assured**: 0 qualified platforms, so assurance `verified` is closed to every Story in this project ([`hand-2026-08-05/06A`](../../session/hand-2026-08-05/06A-nothing-is-verified-and-the-reason-is-not-velocity.md) §2).**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: [`session/hand-2026-08-03/05A-ethernet-discovery-signal.md`](../../session/hand-2026-08-03/05A-ethernet-discovery-signal.md) §"Board attempt" — the evening's dead-flat PHY, promoted from `LE-68`

## Description

The first physical attempt answered `STORY-P1-09-02`'s named risk: the board
parks green while the laptop's NIC sees a dead-flat wire across power cycles,
because the BCM54213PE sits behind an **active-low reset on RP1 GPIO 32**
(`bcm2712-rpi-5-b.dts`: `phy-reset-gpios = <&rp1_gpio 32 GPIO_ACTIVE_LOW>`,
`phy-reset-duration = <5>`) that only an OS driver ever releases. This Story is
that release, at bring-up size: drive the pin low through RP1's registered-IO
peripheral, hold the documented 5 ms, drive it high, give the PHY its settle
time, and only then run the MDIO scan that has been finding nobody.

GPIO 32 is pin 4 of RP1's **bank 1** (banks carry 28/6/20 pins at a `0x4000`
stride): `io_bank1` at RP1 `0x0d4000`, `sys_rio1` at `0x0e4000`, `pads_bank1`
at `0x0f4000` — all inside the window `STORY-P1-09-01` already validates
before any use. The sequence is glitch-ordered: pad output-disable cleared and
RIO level/direction staged *before* the pin's function select is switched to
RIO, so the reset line's first driven value is the assertion, never a float or
a spike. Every wait is the bounded counter-tick wait the beacon already uses.

Closes `LE-68` when its board criterion passes; until then the loose end stays
open — a release sequence nobody has watched work is a hypothesis with good
provenance.

## Depends on

- `STORY-P1-09-01` — the release drives registers behind the same validated
  window; no window, no release, and `rp1=absent` already says why.

## Acceptance criteria

1. **The register sequence is exact and glitch-ordered.** Pad enable (RMW,
   output-disable cleared), RIO out-low and output-enable via the atomic
   aliases, function select to RIO (RMW preserving every field it does not
   own), the 5 ms hold, the high release, the settle wait — pinned in order
   by a recording seam double, with the transcribed bank-1 addresses asserted
   against their sources in `board.rs`-style pinning tests.
2. **The waits are real and bounded.** The hold and settle durations come from
   the counter-tick wait with its spin bound; a stuck counter aborts the
   release and reports rather than hanging the boot.
3. **The pipeline runs the release between identity and the scan.** The GEM
   identity readback still gates everything; the release runs exactly once,
   after identity, before the management port opens — asserted by ordering in
   the pipeline tests.
4. **Board: the wire wakes up.** With the release in the image, the laptop
   NIC trains (link watch shows a transition) and — serial permitting — the
   capture's `TOS64-LINK/1` line reports `phy=0x600d…` at address 1 instead
   of `phy=absent`. This criterion is what closes `LE-68`.

## Named debt this Story leaves open

- `LE-68` — until criterion 4, by its own definition.
- The 10 ms post-release settle is a chosen engineering margin, not a
  datasheet transcription; if the board shows the PHY needs longer, the
  number changes in one named constant with its test.

## Progress, 2026-08-03

| Criterion | State |
|---|---|
| 1 — exact, glitch-ordered sequence | **Green.** `rp1_gpio.rs`: the five writes pinned in order with RMW preservation proven against hostile readbacks; every address derived from the pin-4-of-bank-1 arithmetic and asserted inside the validated window. |
| 2 — bounded waits | **Green.** Hold and settle run on the bounded counter wait; a stuck counter aborts with the line still asserted (never a half-released PHY), reported as `phy=unreleased`. |
| 3 — pipeline placement | **Green.** Release runs exactly once between identity and the scan; a failed gate or refused identity never touches the GPIO registers. |
| 4 — board: the wire wakes up | **Blocked on the next board attempt.** This is `LE-68`'s closure criterion: the laptop NIC trains where every earlier watch was flat. |

## Tests

[`TEST-P1-09-04-A`](../tests/TEST-P1-09-04-A.md) — written before
implementation, per the TDD mandate.
