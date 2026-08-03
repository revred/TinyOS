# Handover 03A (2026-08-03) — Owner Direction: the Pi 5 Is Headless; the Display Is a Remote Desktop over Peer-to-Peer Ethernet

Recorded verbatim in effect: *"I will ditch a dedicated display completely for the
Pi 5 and it is always like a remote desktop on any other device that connects with
the ethernet port peer-to-peer."* Direction, not decomposition — promoted via a
Feature/ADR when the work starts.

## What this fixes

The target device ships **headless**. Any other device — laptop, tablet, the Ti64
console — connects to the Pi 5's Ethernet port **peer-to-peer** (link-local, no
switch, no DHCP, per the recorded 2026-07-27 deploy-transport decision) and gets
the full UX as a remote-desktop experience. The same authenticated link carries
the development loop, deploy, and the **spoor stream**.

Convergences, not coincidences: this is the TinySpot remote-UX concept applied to
the flagship board; the Ti64 console is already a host-side surface fed through
seams (a wire is just a longer seam); and it dissolves the on-target display
problem — no compositor, no GPU driver, no `EPIC-H2` dependency on the device,
because rendering happens where the screen already is.

## The dependency chain, in its unavoidable order

1. **Serial bring-up stays the bootstrap** — unaffected by this direction.
   Ethernet cannot debug itself into existence: the jack sits behind RP1 over
   PCIe (`LE-26`), and those drivers get debugged over the UART.
2. **`LE-26` closure as real Stories**: PCIe controller → RP1 → Ethernet MAC/DMA,
   a C2 device service with full containment contracts.
3. **The authenticated session**: link-local addressing plus the deploy-protocol /
   WCI trust model — a bare socket is never the answer.
4. **The remote UX stream**: display frames + input events over that session,
   rendered as a Ti64 console tab; spoors ride the same channel.

## Immediate consequences

- **No further local-display investment.** `STORY-P1-07-07`'s splash stays
  as-built (zero marginal cost, the honest boot indicator for whoever does plug a
  screen in during bring-up); its photograph criterion becomes opportunistic
  rather than blocking. Board-outcome note: the first adaptive-splash boot
  remains **unconfirmed** — an initial sighting was withdrawn by the owner and
  the Story records exactly that.
- The next physical milestone is unchanged: loopback-test the adapter (a COM
  port appeared and vanished again this session — likely the probe being
  wired), capture the first boot, climb the `-02`…`-06` ladder.
