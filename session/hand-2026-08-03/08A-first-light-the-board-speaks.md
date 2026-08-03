# Handover 08A (2026-08-03, evening) — First Light: the Board Speaks, Rung by Rung

The staged boot from 07A ran, and the evening it opened became the most
productive board session in this project's history: **TinyOS produced its
first observable effect on silicon** (the ACT LED, pulsing through the boxed
case), then diagnosed its own Ethernet chain three rungs deep by blinking
refusal codes a human counted in a dark room, and closed with the monitor
about to become a real console. Eight Stories landed, every one contract-first
and spine-green; three of them were **proven on silicon the same evening they
were written**.

## The evening's arc, compressed

1. **07A's staged boot**: screen *signal-but-dark* (new — every earlier boot
   was cold), NIC flat for 8 minutes. Verdict deferred to better instruments.
2. **Ground truth captured from inside the working system** over SSH
   (`goals/reports/pios-ground-truth-2026-08-03.txt`, two visits): PHY
   bring-up timings (~4 s autonegotiation), `/proc/iomem`, `lspci -vv`
   (bridge bus numbers, forwarding window, `Mem+ BusMaster+`), the DRM state,
   the EEPROM version — and two gifts: the ACT LED is **SoC-side**
   (`gpio-brcmstb@107d517c00` pin 9, behind no suspect peripheral), and the
   firmware **already scans out a framebuffer**
   (`0x3f800000, 1920x1080, r5g6b5, stride 3840`).
3. **`STORY-P1-07-08` — the lamp.** Forced on at entry, 1 Hz in the park
   loop. **Pulsing observed ~20:25 — 07A's charge 1 (execution unproven) is
   dead forever.** Polarity measured active-HIGH on silicon, overriding the
   debug listing; constant and tests amended (the measurement governs).
4. **`STORY-P1-09-06` — the link watch.** The owner's design rule, now
   structural: *no bench-tuned timing constant anywhere*. The park loop
   re-reads the PHY once a second forever; the beacon starts whenever the
   wire trains. (Also fixed latently: the old single link read happened
   15 ms after reset release against a measured ~4 s autonegotiation.)
5. **`STORY-P1-09-07` — the confession.** Discovery's first refused rung as
   a repeating blink count. **Proven immediately: the lamp counted 3** —
   `DL_ACTIVE` clear with RC mode and PCIe PHY up.
6. **`STORY-P1-09-08` — the second look.** The probe joins the park-loop
   watch cadence (gate discipline and the exactly-once PHY release preserved
   across retries). **Proven: the next count was 4** — the data link now
   clears; the outbound window is the next refusal.
7. **`STORY-P1-09-09` — the window programmed, not presumed.** On a
   window-class refusal the five `WIN0` registers get the capture's mapping,
   then are believed only from readback. **Proven: the next count was 9** —
   window accepted; the GEM identity read now answers *wrong-module*.
8. **`STORY-P1-09-10` — the introduction.** Bring-up-size enumeration from
   the `lspci` values: root-port vendor gate, bus numbers `0/1/1`, 5 MiB
   forwarding window, memory decode both ends, RP1's vendor verified before
   its decode. **Both vendor gates passed on silicon** (no 14/15 blink) —
   config-space access works and RP1 answered `0x1de4` — but identity still
   refuses with a *flickering* 8/9: reads reach something unstable. Prime
   suspect for next session: RP1's internal clock/reset tree (the `rp1`
   driver programs it before touching peripherals).
9. **`STORY-P1-09-11` — the spelling.** The owner redesigned the readout:
   decimal digits, ones first, fixed seven groups (code ×2, sixteen-bit
   detail ×5). First field attempt garbled → root-caused to sentences
   swapping mid-read → **the latch** (a sentence in flight is never
   replaced). Zero's rendering went flicker → **1.5 s solid burn** after the
   flicker read as a 1 on the board. Image `a832774d…` carries all of that
   — **never booted** (superseded by the pivot below).
10. **`STORY-P1-07-09` — the firmware's canvas (the owner's pivot: "fix the
    HDMI, not just an LED").** The captured simple-framebuffer painted
    through the existing pure `Surface` seam: RGB565 conversion, a
    bounds-honest surface, a full report font, and the console — title, the
    `TOS64-LINK/1` line as text, the live heartbeat line, any refusal as
    `CODE NN DETAIL NNNNN` in amber. Host-Green; **the staged card carries
    it** (`db5218bf…`, hash-verified).

## Instruments, new and standing

- **`work/tools/` gained two C# tools** (owner convention): `tos64-linkwatch`
  (wired-NIC transition log with ms timestamps — it, not serial, called every
  verdict tonight) and `tos64-cardswap` (one physical card serves both roles:
  TOS64 ↔ Pi OS ground-truth by swapping `config.txt`+`kernel8.img` with
  hashes verified; the Pi OS backup lives on the card).
- **Pi OS card half**: user `revanur` exists, SSH works over IPv6 link-local
  (`raspberrypi.local`), remote poweroff works. The dead serial adapter is
  fully demoted; nothing tonight needed it.
- The lamp (execution + refusal channel) and now the canvas (text) are the
  board's voices. The wire is next.

## Where the next session starts

**The card is staged with `db5218bf…` and sits ready.** Boot it:

1. **Read the monitor.** First boot ever with on-screen text. Expected:
   `TINYOS`, the report line (`rp1=absent reason=id-…` or better), the
   ticking heartbeat line, and the refusal `CODE NN DETAIL NNNNN`. The
   `DETAIL` digits are the identity readback we've been chasing — the number
   that picks the next fix. (Canvas geometry is a pinned bet from the
   capture; a shear/blank outcome moves constants, not goalposts.)
2. **If the detail implicates RP1's clock/reset tree**, transcribe the
   `rp1` clocks/reset bring-up from ground truth (SSH the Pi OS card for
   `/sys/kernel/debug/clk_summary` and the rp1 driver sources) into the next
   rung Story. If identity clears: release → scan → the watch starts the
   beacon on the wire's schedule — then **capture `TOS64-PRESENT/1` on the
   laptop and FEAT-P2-10 (Ti64Dink link-watch) unblocks.**
3. **Standing queue behind that**: the SD evidence recorder (full readback
   dumps to the card's unpartitioned sectors 1–16383) if any rung needs more
   than sixteen bits; the mailbox-splash question is moot in practice now
   but `STORY-P1-07-07`'s board criteria still want their honest answer.

## State of record

221 hal-arm64 host tests green · spine green at **29 Features / 86 Stories /
70 Tests** (8 new Stories, 8 new Test docs tonight) · host and cross-target
clippy clean · dashboard synced including the narrative UPDATE · `LE-68`
still open (the wire has not trained; its closure path now runs through the
identity rung) · no new loose ends registered — every night-debt is named
inside its Story. The workspace-wide clippy failure on this Windows host
remains the recorded `cfg(not(windows))` blindness in `kernel`/`shell`
(untouched tonight); Linux CI is the real gate there.
