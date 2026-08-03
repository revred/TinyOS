# Handover 07A (2026-08-03) — Every Instrument Proven, TinyOS Indicted: the Work Queue Is Now Code, Not Cables

The owner cut through four branches of fog with one argument: two independent
cables both being faulty is one-in-a-billion; the OS being faulty is likely.
One boot of stock Raspberry Pi OS on the same board, same cables, same
monitor proved it. **Every piece of hardware works. Every silence was ours.**

## What was proven on silicon today, with evidence

| Channel | Under Raspberry Pi OS (trixie lite arm64, hash-verified) | Verdict |
|---|---|---|
| Ethernet peer-to-peer | Link trained **1 Gbps** ~89 s after power; `raspberrypi.local` resolved via mDNS to IPv6 link-local; **sub-millisecond pings both directions** | Cable, PHY, RP1, GEM path: **good**. Dead-flat under TinyOS = our bring-up never ran or never worked |
| micro-HDMI (HDMI0, beside USB-C) | Stable picture (first-boot console UI, photographed) after firm reseating — connector is touchy but the chain works | Cable, port, monitor, input: **good**. Dark under TinyOS = our display path (or firmware entry) at fault |
| Power | Green LED flicker = SD activity, normal; no red under-voltage at any point | Supply adequate |
| Serial debug UART | Fifth zero-byte capture (COM5 healthy post-reboot, pinned 115200, 150 s window spanning a power cycle, plus a looping twelve-baud sweep — all silent) | **Still unproven as an instrument.** Loopback infeasible (owner). Do not build plans that depend on serial |

The one open hardware question is the serial wiring/adapter — and it no
longer gates anything: **the laptop NIC is now the primary instrument**
(link-state watch catches PHY bring-up with zero packets), and the proven
monitor is the second.

## The two charges against TinyOS

1. **Execution itself is unproven.** The green LED only proves firmware. No
   TinyOS build has ever produced an observable effect on this board. The
   firmware's entry into `kernel8.img` (`os_check=0` semantics — divergence
   record §3, the constant no test can check) has never been confirmed on
   this firmware version. Everything below assumes it; nothing has tested it.
2. **If it executes, both output paths fail silently**: the mailbox
   framebuffer exchange (`STORY-P1-07-07`'s named blind-flight risk) and the
   PHY release + link bring-up (`STORY-P1-09-04`, implemented, never
   verified). Pi OS proves the hardware would have answered a correct driver.

## The decisive experiment, already staged

The TOS64 card carries `28682388…4558f30` (118,012 bytes, hash-verified on
card) — the `fb=granted|refused` heartbeat image. Boot it on the proven
chain (same cable, HDMI0, same input, monitor on before power) and watch two
instruments that now have known-good baselines:

- **Screen**: navy splash + bouncing block → display path vindicated, block
  color = Ethernet verdict. Dark on this proven chain → the Pi 5 firmware
  refuses the legacy mailbox path — confirmed, not suspected.
- **Laptop NIC**: any link training during TinyOS boot → `STORY-P1-09-04`'s
  PHY release verified on silicon (`LE-68`'s criterion), no serial needed.
  Flat while Pi OS trains in 89 s → our release sequence is wrong; fix it
  against ground truth (below).

This boot was queued when the owner ordered this handover; its outcome is
the first line of the next session.

## Ground truth on tap: the Pi OS card is an instrument now

The Pi OS card (user `tinyos`) stays. With SSH enabled
(`sudo systemctl enable --now ssh`, one-time at its console) the laptop can
read the working system from inside over the proven link: `dmesg` for the
exact PHY reset/MDIO sequence and timings, `/sys/class/drm/*` for
HDMI hotplug/EDID/mode state, device-tree as actually loaded, firmware
version. Every TinyOS driver fix should be written against what the working
system *measurably does*, not against folklore. Host tools for this live in
`work/tools/` and are C# by owner rule (sdprep, tos64-serialwatch,
tos64-imgwrite — the last wrote today's Pi OS card, rails intact,
hash-verified source image).

## Next actions — all of them code-facing, in order

1. **Run the staged boot.** Record screen + NIC outcome. This splits charge
   1 from charge 2 for the display, and verifies or convicts the PHY release.
2. **If both channels stay silent, attack charge 1 directly**: prove
   execution with the smallest possible observable — the NIC link itself is
   that observable once the PHY release runs early enough; alternatively a
   deliberate minimal image whose only job is PHY-release-then-park. One
   observable effect ends the "does our image even run" era permanently.
3. **Ethernet to Ti64Dink (owner priority 1):** verify/fix
   `STORY-P1-09-04` against Pi OS `dmesg` ground truth → `LE-68` closes on
   an observed link train → `TOS64-PRESENT/1` beacon captured on the laptop
   NIC → `FEAT-P2-10` link-watch is unblocked. The wire is proven; only our
   driver stands between the board and the discovery signal.
4. **Splash → OS on HDMI (owner priority 2):** if the staged boot shows the
   splash, harden and move on. If it proves firmware refusal, mine the Pi OS
   card for how this firmware actually exposes the display and decide the
   Pi 5-native path with data — `STORY-P1-07-07`'s board criterion finally
   closes either way.
5. **Serial**: demoted to a convenience. If it ever decodes, fine; nothing
   waits on it anymore.

## Session hygiene

`work/tools/` gained `serialwatch` and `imgwrite` (C#, untracked by the
same convention as sdprep). `STORY-P1-09-05`'s progress table was corrected
to show the `fb=` field its criterion 1 pins. Spine green, 182 `hal-arm64`
tests green. No kernel code changed this session — the next change happens
with a proven instrument watching it.
