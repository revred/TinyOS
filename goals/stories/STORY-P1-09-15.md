# STORY-P1-09-15 — The Unclaimed Fetch: the Root Complex's Inbound Windows Written from the Capture, Believed from Readback

Status: **Verified (functional) 2026-08-05 — every criterion Green on silicon 2026-08-04 ~02:07, criterion 5 in its success arm: the first boot carrying the twelve dwords walked the beat line `STOPPED REASON=TIMEOUT` → `STATE=BEACONING`, and the laptop's linkwatch baseline read the wire already trained at gigabit — the transmit completes and the beacon runs every period. Criterion 5 has since been re-observed on a different image and a different boot path: 2026-08-05's **netbooted** run reached `STATE=BEACONING` with the beacon on the cable, so the inbound windows are not a property of one image. This Story closed the last of the three instances of one disease — "state Linux programs and the firmware does not" — after the outbound window (`-09`) and the endpoint BARs (`-13`); all three are now written by `establish()` and each is believed only from readback. Advanced under [`06A`](../../session/hand-2026-08-05/06A-nothing-is-verified-and-the-reason-is-not-velocity.md) §4.1. **Assurance state remains `specified` and this Story is NOT release-assured**: 0 qualified platforms.**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: [`session/hand-2026-08-04/01A-covernote-boot-first-the-inbound-window.md`](../../session/hand-2026-08-04/01A-covernote-boot-first-the-inbound-window.md) — the same-night inbound-window capture (2026-08-04 ~02:05)

## Description

The park loop's first spoken verdict read `STATE=STOPPED REASON=TIMEOUT`:
the watch resolved, the beacon started, and the first frame's transmit
never completed. The GEM transmits by mastering **inbound** reads of its
descriptor ring and frame buffer at PCI `0x10_0000_0000` — the captured
`dma-ranges` translation `RP1_DMA_RAM_BASE` already encodes on the TinyOS
side. Those inbound TLPs are claimed by the root complex's
`RC_BARn_CONFIG` windows, which Linux writes in
`set_inbound_win_registers()` and the firmware does not — the third and
last instance of the session's one recurring disease: *state Linux
programs and the firmware does not, which `establish()` therefore must.*
An unclaimed descriptor fetch means the GEM never starts, TSR never
completes, timeout.

The same-night capture read all twelve dwords live under working
ethernet: three windows (the 4 MiB RP1 peripheral alias at PCI `0x0`, the
64 GiB DMA window PCI `0x10_0000_0000` → RAM `0x0`, and the 4 KiB MSI
page), each an `RC_BAR` pair *and* a programmed `UBUS_BAR` remap pair —
the UBUS family reads programmed under Pi OS, so the Pi 5 takes the
BCM7712-style path and TinyOS must write **both families: twelve dwords
total.** Each dword is derived from the captured `dma-ranges` triple
through the driver's own size encoding (a pure transcription), written
exactly once inside `pcie::establish` after enumeration, and believed
only from its readback; a window already holding its four pinned dwords
sees zero writes — idempotent, same as the BARs and for the same reason.

## Depends on

- `STORY-P1-09-13` — the endpoint BARs this rung mirrors on the root
  complex side; the pattern (capture-pinned values, readback belief,
  idempotence, refusal codes) is copied deliberately.
- `STORY-P1-09-14` — the spoken park verdict whose `STOPPED
  REASON=TIMEOUT` convicted this path.

## Acceptance criteria

1. **The twelve dwords are the capture's, derived not guessed.** Each
   window's `RC_BAR_LO` is built from its captured PCI offset and the
   driver's size encoding (`4KB..32KB → 0x1C + (log2−12)`;
   `64KB..64GB → log2−15`; `0 = disabled` — pinned as a pure
   transcription test); each `UBUS` remap pair is built from its captured
   CPU target with `ACCESS_EN` set; the raw results equal the captured
   dwords bit-for-bit, and window 2's PCI offset equals
   `board::RP1_DMA_RAM_BASE`.
2. **Every write believed from readback.** Each dword is written exactly
   once and re-read; a readback disagreeing with the write refuses with
   the readback — the `RC_BAR` family as its own arm, the `UBUS` family
   as its own arm — and no later dword is written past a refusal.
3. **Seat and idempotence.** The inbound pass runs inside
   `pcie::establish`, after `enumerate` (controller-local registers, like
   the outbound window); a window already holding its four pinned dwords
   sees zero writes, and a settled pass over all three windows writes
   nothing.
4. **The confession speaks the new rungs.** Codes 21 (`ibw-held`, an
   `RC_BAR` dword not held) and 22 (`ibw-remap`, a `UBUS` dword not
   held) with the readback's decisive low half; `TOS64-LINK/1` names them
   with the full readback; the exhaustive match forces the wiring; every
   previously pinned line is byte-identical.
5. **Board: the transmit completes.** The next boxed boot's beat line
   walks `STOPPED REASON=TIMEOUT` → `STATE=BEACONING` and the beacon is
   on the wire — or the confession names this rung's actual readback
   (`reason=ibw-… detail=0x…`) and the ladder continues on that number.

## Named debt this Story leaves open

- Bring-up size only: three fixed windows from one capture. A real
  inbound resource map is `EPIC-P3`'s root-complex driver.
- `LE-67` unchanged: the DMA window is claimed, not contained — no IOMMU
  on this path; containment stays one pinned buffer.

## Progress, 2026-08-04

| Criterion | State |
|---|---|
| 1 — dwords derived from the capture | **Green** (host): size encoding pinned as transcription; all twelve raw dwords pinned; window 2 cross-checked against `RP1_DMA_RAM_BASE`. |
| 2 — readback belief | **Green** (host): write-once-readback-believe per dword; both refusal arms driven; nothing written past a refusal. |
| 3 — seat + idempotence | **Green** (host): establish's write list pinned with the inbound pass after enumeration; a settled window sees zero writes. |
| 4 — confession wiring | **Green** (host): codes 21/22 distinct and exhaustive; `ibw-held`/`ibw-remap` lines pinned; every prior line byte-identical. |
| 5 — board | **Green, success arm (2026-08-04 ~02:07).** The beat line read `STATE=BEACONING` where every prior boot read `STOPPED REASON=TIMEOUT` or `PARKED`; the report line's boot-time `LINK=DOWN BEACON=SKIPPED` is the expected snapshot; linkwatch's baseline read the NIC already up at 1000 Mbps. The beacon-on-the-wire byte-compare stays owed to an elevated capture or Ti64Dink (`FEAT-P2-10`) — `pktmon` is still access-denied unelevated. |

## Tests

[`TEST-P1-09-15-A`](../tests/TEST-P1-09-15-A.md) — written before
implementation, per the TDD mandate.
