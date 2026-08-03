# FEAT-P1-09 — Ethernet as a Discovery Signal: RP1 over the Firmware-Kept PCIe Link

Status: **In progress — three Stories added and host-implemented 2026-08-03 on the owner's order (this session's objective, reordering the 08A queue: the card image gains honest network smarts); every host half Green with the `LE-66` scripted-seam discipline from commit one; every board criterion open, blocked on the same serial adapter as `FEAT-P1-07`. Nothing is labelled live until its evidence exists.**
Epic: [`EPIC-P1`](../epics/EPIC-P1.md)
Introduced in: [`session/hand-2026-08-03/04A-tinyremote-vision.md`](../../session/hand-2026-08-03/04A-tinyremote-vision.md) (direction) and [`session/hand-2026-08-03/05A-ethernet-discovery-signal.md`](../../session/hand-2026-08-03/05A-ethernet-discovery-signal.md) (this promotion; the host application named there as **Ti64Dink**)

## Description

Handover 04A fixed the product shape: the cable is a **discovery** signal, never an
authority statement. This Feature delivers the device half of that signal at the
smallest honest size — not a network stack, not a deploy transport, not a session:
the boot image learns to (1) prove the RP1 southbridge is reachable over the PCIe
link the firmware kept alive, (2) identify the Ethernet PHY and see the cable, and
(3) transmit one pinned, bounded, broadcast **board-present beacon** a laptop can
capture. The beacon is deliberately the evidence channel: it is visible to a stock
packet capture on the host before any serial adapter works, which makes it the
earliest visible win 04A named, and the aliveness ping the dev loop needs.

The scope trick that makes this a bring-up Feature rather than a driver Epic:
Pi 5 firmware brings the PCIe x4 link to RP1 up itself and normally resets it
before kernel handoff. `config.txt` gains `pciex4_reset=0`, the reset is skipped,
and RP1's peripherals stay visible through the firmware-established window at CPU
`0x1F_0000_0000` (RP1 `0x4000_0000`). The window's configuration is **recorded as
evidence, never retained as authority** — the same posture `FEAT-P1-07` takes to
firmware handoff state — and a window the firmware did *not* keep (old firmware,
missing config line) must report `absent` and park fail-safe, never hang.

`LE-26` is routed one rung further, not closed: a real PCIe root-complex driver,
RP1 as a restartable C2 device service with containment contracts, and a NIC/DMA
class driver remain `EPIC-P3` / `EPIC-P1_5` territory, and the transport decision
`LE-26` re-opens stays open.

## Crate(s) involved

- `os/src/hal-arm64` — the guarded-window probe, GEM management-port (MDIO)
  access, PHY identification and link state, frame builder, bounded transmit path.
- `os/src/xtask` — the `pi5` pipeline's `config.txt` contents gain
  `pciex4_reset=0`; capture vocabulary learns the `TOS64-LINK/1` line.

## Depends on

- [`FEAT-P1-07`](FEAT-P1-07.md) — everything here runs strictly after the
  `TOS64-RESULT/1` verdict and the splash, through the same boot, fault-vector,
  MMU and UART path; a network probe may never perturb the evidence the board
  session exists to capture.
- The deploy-protocol/WCI trust model ([`docs/deploy-protocol.md`](../../docs/deploy-protocol.md))
  governs everything this Feature deliberately does **not** do: the beacon is
  unauthenticated presence, carries no authority, and opens no session.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P1-09-01`](../stories/STORY-P1-09-01.md) | The kept link: RP1 presence proven by validated identity readback, absence reported honestly | In progress — host half Green 2026-08-03; criterion 4 awaits a board capture |
| [`STORY-P1-09-02`](../stories/STORY-P1-09-02.md) | The PHY answers: management-port identification and latched link state — the device sees the cable | In progress — host half Green 2026-08-03; criterion 4 awaits a board capture |
| [`STORY-P1-09-03`](../stories/STORY-P1-09-03.md) | The beacon: one pinned broadcast frame, transmit-only, receive left disabled | In progress — host half Green 2026-08-03; criterion 4 awaits a board capture |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) ·
implementation **C1** · subject **C1/C2** · boundary tests **BND-03, -06, -07, -17** ·
**PD-07, PD-10, PD-12, PD-14** · **RCG-01, RCG-13, RCG-14**.

Hostile inputs, enumerated: the firmware's PCIe window state (kept, reset, or
half-configured); every RP1 and GEM register readback value; PHY identifier and
status registers and every MDIO response; data aborts on the kept window; clocks
or link states that never settle. Every readback is validated before belief, every
poll is budget-bounded, and every hostile or absent answer resolves to the same
fail-safe park with an honest `TOS64-LINK/1` report. The Ethernet block is treated
as a compromisable C2 subject: it receives exactly one pinned, bounded transmit
grant, and its receive path stays disabled — remote bytes are data, never code
(`RCG-01`), and in this Feature they are not even read.

## Exit criteria

**A laptop connected peer-to-peer captures the board-present beacon, and the
serial protocol is unchanged.** Concretely:

- `TOS64-LINK/1` reports RP1 presence/absence, PHY identity, and link state over
  the UART after the verdict — both outcomes honest, neither hanging.
- The beacon frame appears in a stock host packet capture and is byte-identical
  to the pinned frame the host-side tests already assert.
- Every protocol line `FEAT-P1-07` pinned (entry report, READY, `vbar`,
  `TOS64-RESULT/1`) is byte-identical with this Feature's code present.
- Every wait on the path is budget-bounded; a reset window, absent PHY, or
  never-settling link parks fail-safe with the failure named.

## Explicit non-goals

- **No IP.** No ARP, no DHCP, no IPv4/IPv6 link-local addressing, no TCP/UDP —
  the beacon is a raw Ethernet II broadcast frame with a local-experimental
  EtherType. Link-local addressing arrives with the deploy-protocol session work.
- **No receive.** GEM RX stays disabled; no byte from the wire is parsed, stored,
  or acted on. Board-side discovery of the *host* is out of scope.
- **No authenticated session, no deploy, no spoor stream** — deploy-protocol/WCI
  territory, queued exactly as 04A's dependency chain orders.
- **No PCIe root-complex driver, no RP1 C2 device service, no NIC class driver,
  no IRQ path** — `EPIC-P3`/`EPIC-P1_5`; this Feature only walks a window the
  firmware already built, polled, never interrupt-driven.
- **No timing claim of any kind** (`ADR 0005` discipline unchanged), and no
  Ti64Dink host-application work — that promotion is `EPIC-P2`'s (`FEAT-P2-10`
  is the first free slot) and happens on its own schedule.

## Named debt this Feature does not touch

- `LE-26` — stays open; the transport decision is still owed its re-opening.
- `LE-67` — raised by this Feature: the beacon's transmit DMA runs with no IOMMU
  on this path; containment is one pinned buffer and `BND-07` evidence is
  correspondingly narrow until the real device-service work lands.
- `LE-57`, `LE-64` — untouched.
