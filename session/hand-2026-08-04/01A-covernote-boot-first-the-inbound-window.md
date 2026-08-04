# 01A — Cover Note: Boot the Pi First. The Card Carries the Capture Payload, and the Inbound Window Is the Last Silent Path

Session handover, written 2026-08-04 ~02:00 at the close of the marathon that started as
hand-2026-08-03/09A's session. Read this note top to bottom, then act — the first physical
action is powering the Pi with the card that is **already in the Pi OS role, already
key-authed, already carrying everything the capture needs.**

---

## Where the ladder stands (one paragraph, all verdicts on silicon)

Three stories delivered and board-proven since 09A was written, four commits pushed, all
CI-green: `2aac812`/`adf182d` (STORY-P1-09-13 — the endpoint BARs sized, assigned, believed;
**the poison is dead**: `RP1=PRESENT ID=0x0109 PHY=0x600D84A2` on the canvas, and the wire
trained to gigabit at 01:27:03 and again at 01:40:58 — TinyOS negotiates Ethernet links now),
and `a2cbc2f`/`031962a` (STORY-P1-09-14 — the park loop speaks; its first boot read
**`STATE=STOPPED REASON=TIMEOUT`**, which in one word *cleared* the link watch, the MDIO
re-enable and the BMSR decode, and *convicted* the transmit: the watch resolved, the beacon
started, and the first frame's TX never completed). Spine: 29 Features / 89 Stories /
73 Tests, 238 hal-arm64 tests, fmt + both clippy targets clean. The blink ladder's codes now
run 1–20; the beat line speaks six distinct park verdicts.

## The one hypothesis this session exists to test

The transmit hangs because **the root complex's inbound (DMA) windows are unprogrammed** —
the third and last instance of tonight's single recurring disease: *state Linux programs and
the firmware does not, which `establish()` therefore must.* The GEM transmits by mastering
reads of its descriptor ring and frame buffer at PCI `0x10_0000_0000` (the `dma-ranges` we
captured: PCI `0x10_0000_0000` → RAM `0x0`, 64 GiB — the translation `RP1_DMA_RAM_BASE`
already encodes on the TinyOS side). Those inbound TLPs are claimed by the RC's
`RC_BARn_CONFIG` windows — which Linux writes in `set_inbound_win_registers()` and we never
touch. An unclaimed descriptor fetch means the GEM never starts, TSR never completes,
timeout. It rhymes perfectly with the endpoint BARs, and the fix will look identical: a few
register writes, believed from readback, with a refusal code.

## STOP — Step 1 already happened. The capture ran at session close, same night.

The owner booted the Pi OS card one more time before this note was committed, and the
inbound-window capture is **done and in the ground-truth file** (tail section, 2026-08-04
~02:05). The decoded result, complete and self-consistent with the captured `dma-ranges`:

| Window | RC_BAR LO/HI (0x402c+) | Decoded | UBUS remap (0x40ac+) | CPU target |
|---|---|---|---|---|
| 1 | `0x00000007 / 0x00000000` | PCI `0x0`, 4 MiB | `0x00000001 / 0x0000001f` | `0x1F_0000_0000` (RP1 periph alias) |
| 2 | `0x00000015 / 0x00000010` | **PCI `0x10_0000_0000`, 64 GiB** | `0x00000001 / 0x00000000` | **RAM `0x0` — the beacon's DMA window** |
| 3 | `0xfffff01c / 0x000000ff` | PCI `0xFF_FFFF_F000`, 4 KiB | `0x00130001 / 0x00000010` | `0x10_0013_0000` (MSI page) |

BAR4 pair and UBUS4 read zero (disabled). **The UBUS registers ARE programmed — the Pi 5
takes the BCM7712-style path, so TinyOS must write both families: twelve dwords total.**
Register offsets confirmed live: RC_BAR1/2/3 pairs at `0x402c/0x4030`, `0x4034/0x4038`,
`0x403c/0x4040`; UBUS1/2/3 pairs at `0x40ac/0x40b0`, `0x40b4/0x40b8`, `0x40bc/0x40c0`.

**The next session therefore opens directly at Step 2** — the card is back in the Pi OS
role only in the sense that it was left there after this capture; the first action is
writing STORY-P1-09-15, and the first *board* action is the TOS64 boot that tests it.
The section below is retained as written before the capture, for the record.

## Step 1 (as planned; executed same-night — see above) — boot the Pi OS card and capture

Power the Pi. The card is in the Pi OS role. Then, from the laptop:

- SSH: `ssh -o BatchMode=yes "revanur@fe80::375c:1a61:f858:2034%16"` — key auth installed,
  address stable across boots, mDNS untrustworthy (09A §6 has every trap; password `sonu`
  if the key ever fails). Confirm the link is steady before starting (linkwatch shows one
  `Down -> UP` then silence; the NM link-local fix is persistent on the card).
- Rebuild the probe first — `/tmp` does not survive reboots (verified tonight): heredoc the
  C source from 09A §6.5, `gcc -O1 -o /tmp/rp1rd /tmp/rp1rd.c`.
- **The capture: the RC's inbound-window registers, live under working ethernet**, from the
  controller block at CPU `0x10_0012_0000`. Offsets from `pcie-brcmstb.c` (`rpi-6.12.y`,
  fetched tonight):

  ```text
  PCIE_MISC_RC_BAR1_CONFIG_LO  0x402c   (LO low 5 bits = encoded size; rest = pci_offset low)
  RC_BAR1_CONFIG_HI            0x4030   (pci_offset high 32)
  RC_BAR2/3 pairs              likely 0x4034/0x4038 and 0x403c/0x4040 — the driver computes
                               them via brcm_bar_reg_offset(); VERIFY by dumping 0x402c..0x4044
  PCIE_MISC_RC_BAR4_CONFIG_LO  0x40d4 / HI 0x40d8
  PCIE_MISC_UBUS_BAR1_CONFIG_REMAP  0x40ac  (bit 0 = ACCESS_EN; value = cpu_addr low, page-masked)
  UBUS_BAR1_REMAP_HI                0x40b0  (cpu_addr high; pairs likely stride 8 — VERIFY)
  PCIE_MISC_UBUS_BAR4_CONFIG_REMAP  0x410c
  ```

  One command shape (adjust after eyeballing):

  ```sh
  echo sonu | sudo -S /tmp/rp1rd 0x100012402c 0x1000124030 0x1000124034 0x1000124038 \
      0x100012403c 0x1000124040 0x1000124044 0x10001240ac 0x10001240b0 0x10001240b4 \
      0x10001240b8 0x10001240bc 0x10001240c0 0x10001240d4 0x10001240d8 0x100012410c
  ```

  Append verbatim to the ground-truth file (a dated `-04` file is fine now; keep the header
  discipline). The size encoding to decode what you see:
  `4KB..32KB → 0x1c+(log2-12)`; `64KB..64GB → log2-15` (so 64 GiB = 36-15 = `0x15`);
  `0 = disabled`. Expect at least one window with pci_offset `0x10_0000_0000` and size code
  `0x15`, plus whatever the 4 MiB peripheral alias and the MSI page get.
- **The BCM2712 question the capture settles:** the driver writes the UBUS remap pair only
  for `soc_base == BCM7712`. If the UBUS registers read programmed under Pi OS, the Pi 5
  takes that path and TinyOS must write both families; if they read zero/disabled, the
  RC_BAR pair alone is the fix. Do not guess — read.
- While there: `poweroff`, card back to the laptop.

## Step 2 — STORY-P1-09-15, fully shaped (write it against the captured numbers)

The pattern is now rehearsed three times; -13 is the template to copy:

- **Seat:** inside `pcie::establish`, after `enumerate` (order vs endpoint work is free —
  these are controller-local registers, like the outbound window).
- **Rung:** for each inbound window the capture shows populated: write `RC_BARn_CONFIG_LO`
  (pci_offset low | encoded size) and `_HI`, plus — if the capture says the Pi 5 takes the
  BCM7712 path — the `UBUS_BARn` remap pair with `ACCESS_EN`; believe every write from its
  readback; a window already holding its pinned values sees zero writes (idempotent, same as
  the BARs and for the same reason).
- **Refusals:** next free codes 21+ (`blink_code`'s exhaustive match will force the wiring),
  reasons in the report line (`ibw-…` naming, or follow `bar-…`), decisive halves per the
  house convention.
- **Tests:** extend the pcie doubles with inbound-register latching; pin the exact write
  list; pin the size encoding against the driver's function (a pure transcription test);
  wrong/dropped readbacks refuse; already-programmed pass writes nothing.
- **Spine ritual** (the gotchas are all in 09A §8 — status-terminator grammar, LE-44
  criteria matching between story header and feature cell, footnote counts, bar %, TWO
  count strings, `emit-dashboard` splice): counts go to 90 Stories / 74 Tests.
- **Board criterion:** the beat line walks `STOPPED REASON=TIMEOUT` → `STATE=BEACONING`,
  and the beacon is on the wire. If it refuses differently (`reason=mac detail=0x…`), the
  status word is the diagnosis and the ladder continues.

## Step 3 — the exit is close; know what "done" looks like

`state=beaconing` on the beat plus the wire trained = `FEAT-P1-09`'s exit criterion within
one packet capture. The beacon-on-the-wire proof needs an **elevated** shell (`pktmon` was
access-denied tonight) or Ti64Dink's capture path (`FEAT-P2-10`, which this unblocks).
When the beacon flows: the owner's amendment activates — diagnosis starts moving onto the
cable as `TOS64-*` envelopes, and the screen goes back to being bootstrap.

## Bench facts current at close

- **Card: Pi OS role** (swap-verified, backup retained). Staged TOS64 kernel is
  `fb827f98…` (-14, the speaking park loop) — **rebuild after -15 lands** before the next
  `cardswap tos64`.
- Board powered off. Cable connected. The wire has trained twice; the PHY pair
  `0x600D/0x84A2` and gigabit negotiation are reproducible facts.
- Linkwatch: re-arm at session start (kill stale instances first); it is the training
  instrument and called every verdict tonight.
- CI green through `031962a`. Nothing uncommitted except the perpetual `_soak` log and the
  untracked `work/tools/` C# fleet (its commit-or-ignore decision is still owed — 09A
  Appendix E).
- The deep context — evidence corpus, SSH/ops runbook, transcription drills, contingency
  trees — is [hand-2026-08-03/09A](../hand-2026-08-03/09A-window-poisoned-inbound-path-indicted.md)
  (64 KB, written for exactly this purpose). This note supersedes only its "what next":
  the answer is now one register family narrower.

The method has not changed since the lamp first blinked 3: ask the board a question it can
answer with a number, believe only readbacks, and let each verdict choose the next story.
Two more of those and the cable talks.
