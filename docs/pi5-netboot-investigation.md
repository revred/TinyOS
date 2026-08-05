# Pi 5 Firmware Netboot — the Investigation That Ends the Card Shuffle

Status: investigation, deliberately **not** a Feature and carrying no code — per
[`hand-2026-08-05/01A`](../session/hand-2026-08-05/01A-the-need-for-speed-and-what-must-not-be-traded-for-it.md)
§4, which names the card shuffle the single largest remaining multiplier on the
board loop, and per this project's own rule that ground truth precedes design
(`01A` §2: twenty minutes of evidence is what stops a day in the wrong direction).
This document exists so the next bench session can settle the open questions in
one boot instead of re-deriving them, and so nobody builds the server before the
board has said what it wants.

## Why this path and not any other

[`spoor-transport-architecture.md`](spoor-transport-architecture.md) §7 already
records the decision that matters: **Pi 5 firmware network boot (TFTP,
`BOOT_ORDER` in EEPROM) loads the image before TinyOS exists, so TinyOS never
admits code and rule 9 is not engaged.** The bytes arrive while the Raspberry Pi
bootloader owns the machine; by the time TinyOS runs, its own posture is
unchanged — GEM receive stays disabled, `LE-67`'s containment story stays intact,
and none of the fourteen `RCG-*` gates is in play. TinyOS receiving an image at
runtime is the path that requires all fourteen, and it is not this one.

What it buys, concretely: today every board experiment costs power-down → card
out → card into laptop → `tos64-cardswap` stage → card back → power-up. With
netboot the loop is *copy `kernel8.img` into a served folder, cycle power* — the
step `01A` §1's one-hour benchmark still spends its only human minutes on.

## What the bench already knows

- **Bootloader EEPROM:** `086b83e3332dfc8927c56762771d082f3077a1ae` (release),
  2026-05-26, `BOOTLOADER: up to date`
  ([`pios-ground-truth-2026-08-03.txt`](../goals/reports/pios-ground-truth-2026-08-03.txt),
  firmware + eeprom section). A 2026 release bootloader long postdates Pi 5
  network-install support, so the capability class exists on this board; which
  *modes* it exposes is question 1 below.
- **The physical link works at firmware-relevant speeds:** the direct
  laptop↔board cable trains at 1 Gbps under both Pi OS and TinyOS
  (`FEAT-P1-09`, board-proven).
- **The laptop can watch the wire unelevated:** Npcap + Ti64Dink, and since
  2026-08-05 with `--until`, so "what does the bootloader broadcast" is a
  15-second capture, not a Wireshark session.
- **The host-tool pattern is settled:** C# console apps under `work/tools/`,
  zero package dependencies (`sdprep`, `cardswap`, `linkwatch`, `ti64dink`).
  If a serving tool is warranted it will be `tos64-netboot` in that fleet.

## The open questions — each with the step that closes it

Stated as *believed, unverified*; nothing below may be treated as fact until its
verifying step has run. This list is the whole reason the document exists.

1. **What `BOOT_ORDER` does this EEPROM currently hold, and what does its config
   schema expose?** Believed: a nibble sequence read right-to-left, with a
   network-boot mode and a restart nibble, plus `TFTP_IP`/`TFTP_PREFIX` options.
   *Verify:* boot the ground-truth card (`tos64-cardswap pios`), run
   `rpi-eeprom-config` and `vcgencmd bootloader_config`, and paste both verbatim
   into the ground-truth register. Zero risk, read-only.
2. **Does the Pi 5 bootloader require DHCP on this link, or can the client side
   be static?** Believed: it DHCPs (the EEPROM can pin the TFTP *server*
   address, not its own), which on a point-to-point cable with no router means
   **the laptop must answer DHCP**. *Verify:* set `BOOT_ORDER` to try network
   with SD fallback, power up with no server and capture what the board
   broadcasts — the DHCPDISCOVER (or its absence) and any TFTP RRQ are the
   ground truth that sizes the server. `--until text=` watching is not usable
   here (the bootloader speaks IP, not TOS64), so this one capture is Wireshark
   or a raw pcap; it is also the only step that needs one.
3. **What file set does the firmware request over TFTP?** Believed: the FAT
   contents the SD boot uses — `config.txt`, `kernel8.img`, and whatever the
   Pi 5 firmware pulls besides (firmware is in EEPROM on Pi 5, so likely *not*
   `start*.elf`) — under a serial-number-derived prefix. *Verify:* first served
   boot's TFTP request log **is** the answer; write it down and serve exactly
   that.
4. **Does a failed netboot always fall back to SD?** This is the bench-safety
   question: the chosen `BOOT_ORDER` must retain SD (or restart) so a dead
   server can never brick the loop. *Verify:* pull the network cable mid-boot
   on the candidate order and confirm the board still comes up from card.
5. **Does netboot coexist with the TOS64 wire?** The bootloader uses the MAC
   and link before TinyOS re-initialises the GEM. Believed harmless — TinyOS
   brings the controller up from scratch — but `STORY-P1-09-*` proved this
   board punishes assumed device state. *Verify:* first netbooted TinyOS run
   must show the beacon and spoor stream unchanged (`ti64dink --until
   rung=ParkIteration` is the 15-second check).

## The decision this investigation feeds — and does not make

If 1–5 verify: a `tos64-netboot` C# tool (DHCP answer scoped to the board's MAC
on the bench adapter + TFTP serving one read-only folder), a runbook section,
and retiring `tos64-cardswap` to the ground-truth-card role only. That is a
Feature-shaped amount of work and gets a Feature — with its contract row naming
the laptop-side listening posture honestly (the *laptop* opens a UDP surface on
the bench link; the board's charter posture is untouched).

If 2 refuses (no DHCP the laptop can satisfy, or Pi 5 netboot needs
infrastructure this bench should not grow): write that down here, close the
investigation, and the card shuffle keeps its runbook — a stated limit beats a
half-built server.

## Cost to run this investigation

One bench session: one Pi OS boot (questions 1, and §6 of the
[runbook](pi5-board-session-runbook.md) — the `NOPASSWD` probe prep — in the
same boot), one EEPROM write, two power cycles with a capture running. Perhaps
twenty minutes, none of it code.

---

## Answers from the bench, 2026-08-05

Three of the five questions are closed on this board, one is closed with a
correction to its own premise, and the fifth is untested. Evidence is the
`tos64-probe` output and two `ti64dink --any` captures taken across live
reboots.

### Q1 — what `BOOT_ORDER` does this EEPROM hold? **Answered.**

```
bootloader 2026/05/26, version 086b83e3332d… (release), capabilities 0x0000007f
[all]
BOOT_UART=1
BOOT_ORDER=0xf461          ← as found
NET_INSTALL_AT_POWER_ON=1
eth0 MAC   88:a2:9e:11:4e:cc
serial     d7bd1a077bf18f79
```

`0xf461` reads right-to-left: **SD, USB, NVMe, restart**. Network (`0x2`) was
**not in the sequence at all**, so this board had never once attempted a netboot
— the capability was present and simply never exercised. `NET_INSTALL_AT_POWER_ON=1`
is the network *installer*, a different feature, and was a red herring.

**Changed to `BOOT_ORDER=0xf12`** — network, then SD, then restart — with the
prior config backed up to `/home/revanur/eeprom-backup.txt` and applied through
`rpi-eeprom-config --apply`. Note the running EEPROM continues to report the old
value until the next boot: the A/B update commits at boot, so *verify after
reboot, not after apply*.

### Q4 — does a failed netboot always fall back to SD? **Answered, and it was the right question to ask first.**

**Yes, twice, unattended.** With `0xf12` live and no DHCP or TFTP server
anywhere on the link, the board attempted the network, failed, fell through to
the card and reached a Pi OS login in **~41 s**, then repeated the same
behaviour on a second reboot (**~27 s** to SSH).

This is the bench-safety result the whole investigation was gated on: **a
missing or bad netboot image now costs a reboot, not a card swap.** Nothing may
depend on netboot before this is established, and now it is.

### Q2 — does the bootloader require DHCP, or can the client side be static? **Answered.**

It issues a **PXE-style DHCP DISCOVER**, captured off the wire:

```
source MAC 88:a2:9e:11:4e:cc, 32 frames
vendor class:  PXEClient:Arch:00000:UNDI:002001
```

Worth noting `Arch:00000` — the architecture code is the legacy x86 value, not
an AArch64 one. A server that dispatches on architecture must not expect `00011`
from this board.

### Q3 — what file set does the firmware request over TFTP? **Not answerable yet, and the question's premise needed correcting.**

**No TFTP traffic appeared at all.** The investigation paired Q3 with "capture
the TFTP file set", but the bootloader never reaches TFTP: without a DHCP
*offer* it has no server address and no boot file to ask for, so the exchange
stops at DISCOVER.

So Q3 is not an observation that can be made before building anything — it is
gated behind a **minimal DHCP responder** that answers this MAC with a
`next-server` and a boot file name. That responder is small and is now
well-specified by Q2's capture. The sequencing in this document was wrong and is
corrected here rather than quietly worked around.

### Q5 — does netboot coexist with the TOS64 wire? **Untested.**

Both captures were taken while the board booted Pi OS, so no `0x88B5` frames
appeared. The question stands.

## What remains to end the card swap

One component: a **minimal DHCP responder plus TFTP server** on the laptop,
answering `88:a2:9e:11:4e:cc`. Per the standing rule it is C# under
`work/tools/`. Q2 sizes it and Q4 makes it safe to experiment with. After it
exists, deploying a TinyOS build is `xtask pi5` plus a reboot, with the firmware
doing the loading — and **rule 9 is never engaged, because TinyOS neither parses
nor admits anything.**

## Hazard found while answering these — do not repeat

**Reading the AVS monitor's debugfs regmap reset the board.**

```
/sys/kernel/debug/regmap/dummy-avs-monitor@0x000000107d542000/registers
```

The syscon region is `0xf0000` bytes, so that file materialises ~983,000
register reads. The SSH connection dropped mid-read and `/proc/uptime` showed
110 s afterwards — the machine had restarted. `head -c` does not help; the
kernel generates the content before anything truncates it.

This also removed the last route to a raw AVS reading under Linux: `/dev/mem`
returns `EINVAL` under Pi OS's `STRICT_DEVMEM`, and the regmap path is now
known-dangerous. The `LE-75` calibration therefore stands as **published
constants corroborated on this board, not independently derived** — the raw
datum falls monotonically as the die warms and TinyOS's derived 40.9→46.7 °C
brackets Linux's 38.05 °C idle sensibly, which is corroboration and is not the
same thing as a two-point fit.

---

## RESOLVED 2026-08-05 — TinyOS booted over the wire

`tos64-netboot` served the image and the board ran it. **The card swap is over.**
Every question in this document is now answered on this board.

### Q3 — what file set does the firmware request? **Answered, in order.**

```
7bf18f79/config.txt              served  45 bytes
7bf18f79/pieeprom.sig            not found  (optional; boot continued)
7bf18f79//config.txt             served    (note the doubled slash, twice)
7bf18f79/kernel8.img             abandoned at block 1, then RETRIED
7bf18f79/bcm2712-rpi-5-b.dtb     not found  (TinyOS needs no device tree)
7bf18f79//cmdline.txt            not found
7bf18f79/armstub8-2712.bin       not found
7bf18f79/kernel8.img             SERVED COMPLETE — 293,481 bytes, 574 blocks
```

`7bf18f79` is the low 8 hex digits of the board serial `d7bd1a077bf18f79`, exactly
as predicted. **Five of the eight requests are for files that do not exist and the
boot proceeded anyway** — the firmware treats them as optional, which means a
netboot root needs only `config.txt` and `kernel8.img`.

Note the **doubled slash** (`7bf18f79//config.txt`). A TFTP server that normalises
paths naively, or one that refuses anything unusual, will fail here. Ours resolves
it through `Path.GetFullPath` and it lands on the same file.

### Q2 — DHCP required? **Yes, and PXE option 43 is not optional.**

The exchange stalled at DISCOVER→OFFER→DISCOVER→OFFER, forever, until the offer
carried **option 43 sub-option 6 (discovery control) = 3**. The board announces
`PXEClient:Arch:00000:UNDI:002001` and, having done so, will not accept an offer
that gives it an address but no boot-server guidance. With option 43 present the
handshake completed on the first attempt:

```
DHCP DISCOVER → OFFER → REQUEST → ACK
```

Also required in practice: a **non-empty boot file name**. An empty one left the
client with nothing to request. `bootcode.bin` was named as a prompt; the board
ignored it and asked for its own file set, which is exactly what the log was for.

### Q5 — does netboot coexist with the TOS64 wire? **Answered: yes.**

The netbooted image is emitting spoors on the same cable it was delivered over:

```
frame seq=222 count=3 epoch=0x04C7D0FF
    Thermal Kernel Observe Ok  rung=ThermalSample  avs=0x000106CB ~61.8C
== boot certificate (epoch 0x04C7D0FF) — 3 record(s), re-announced ==
96 TOS64 frames in 30 s
```

A fresh epoch (`0x04C7D0FF`), the boot certificate re-announcing on its period, and
the thermal rung sampling — from an image the board fetched over TFTP. **The
delivery path and the observability path share one wire and do not interfere.**

## The loop this replaces

Before: build, power down, pull the card, swap files on the laptop, reinsert,
power up. Two card moves and a power cycle per experiment.

Now:

```
cargo run -p xtask -- pi5 --fixture=measure
cp os/target/pi5/{kernel8.img,config.txt} <tftproot>/7bf18f79/
tos64-netboot --mac 88:a2:9e:11:4e:cc --root <tftproot>
# power-cycle the board
```

**Rule 9 is never engaged.** The firmware fetches and jumps before TinyOS exists;
TinyOS neither receives, parses, nor admits anything, `gem.rs` keeps
`no_path_in_this_module_ever_enables_receive`, and `LE-67`'s containment story is
untouched.

## Recovery, and why it is safe

`BOOT_ORDER=0xf12` is network-then-SD. **Stop the server and the board boots the
card** — proven twice before the server existed and unchanged by any of this. So
a bad image costs a reboot with the server stopped, never a card swap.

One consequence worth stating: once TinyOS is netbooted there is no SSH, so a
remote reboot is not available. Returning to Pi OS means stopping `tos64-netboot`
and power-cycling by hand — one physical action, against the two card moves it
replaces.

## Known rough edge

The first `kernel8.img` request was **abandoned at block 1** and only succeeded on
the client's retry. The transfer logic is classic 512-byte lock-step TFTP with a
five-attempt bound; something about the first exchange — most likely the client
switching source port between the request and the first ACK — loses it. It
self-heals through the retry, so it is a slow start rather than a failure, and it
is recorded here rather than left for someone to rediscover.
