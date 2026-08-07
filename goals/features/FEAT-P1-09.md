# FEAT-P1-09 — Ethernet as a Discovery Signal: RP1 over the Firmware-Kept PCIe Link

Status: **In progress — seventeen Stories; `-17` (the admitted verb: one command answered end to end, deny-by-default — proposed under `hand-2026-08-07/10A` §3 S2, written with its charter reading and suite specification in `11A`, and blocked on the owner's sprint-rule sentence S4: no code until it is spoken); `-16` (GEM receive: one frame, fail-closed — the board’s first inbound path, added 2026-08-06 after [`03B`](../../session/hand-2026-08-06/03B-the-arms-are-built-the-board-booted-them-and-nobody-read-the-wire.md) §5 named “the board cannot be told anything” as the one thing standing between this project and an operating system, and after the `SECURITY_CHARTER.md` read that change required); `-15` (the unclaimed fetch: the root complex's three inbound DMA windows written as twelve capture-derived dwords, believed from readback) added 2026-08-04 after `-14`'s first beat read `STOPPED REASON=TIMEOUT` and the same-night 02:05 capture showed both the `RC_BAR` and `UBUS` register families programmed under Linux and untouched by `establish()`; `-14` (the park loop speaks: the beat line carries the park verdict) added 2026-08-04 after the first trained wire exposed three park states that all printed `PARKED`; `-12` (the current: RP1's ethernet clocks switched on before the identity is asked) added 2026-08-03 night after first light held the canvas at `ID-MODULE 0xDEAD` and the same-evening Pi OS capture read `0x00070109` through the same window with two clock-enable bits set, refuted on silicon the same hour (code 16: the whole window is poisoned); `-13` (the address nobody wrote: the endpoint BARs sized, assigned from the conviction capture, believed from mask and readback) added 2026-08-04 after 09A's same-night capture proved the working chain differs from ours in exactly that register class; `-06` (the link watch), `-07` (the blink-code confession, after the first pulsing-lamp boot proved execution while the wire stayed flat) and `-08` (the probe re-read from the park loop, after the confession counted 3: `DL_ACTIVE` late or never) added 2026-08-03 evening after the ground-truth session measured ~4 s of autonegotiation against the pipeline's single 15 ms-late link read, under the owner's constraint that no one bench's number becomes the design; every host half Green with the `LE-66` scripted-seam discipline from commit one; serial demoted, the laptop NIC link watch and beacon capture the primary instruments. Nothing is labelled live until its evidence exists.

**Exit criterion MET 2026-08-05, and twelve of the sixteen Stories are `Verified` (functional) as of the same date** — the closing pass [`06A`](../../session/hand-2026-08-05/06A-nothing-is-verified-and-the-reason-is-not-velocity.md) §4.1 ordered, run against filed evidence rather than against these headers. The beacon was captured off the cable whole and compared byte for byte to the frame the host tests build, header included; that is the sentence this Feature was decomposed around and it is now a file in the tree rather than a plan. Two boots' worth of `TOS64-LINK/1` and beat lines carry the rest.

**Four Stories remain `In progress`, and each names one missing thing rather than a general lack of evidence.** Three of them are *deliberate negatives nobody has taken the trouble to observe*, not unbuilt work; the fourth is new work whose host half is Green and whose board half needs a sender that does not exist yet (`-16`: `ti64dink` captures and does not transmit). The three negatives: `-01` needs one boot with `pciex4_reset=0` removed (the absence arm), `-02` needs one boot with the cable unplugged (a persisting `link=down`, as opposed to the mid-autonegotiation snapshot this bench keeps recording), and `-05` needs either a photograph of the bouncing block or a Test-document amendment substituting the canvas heartbeat that actually delivered its outcome. Two power cycles and one edit would close all three, and until they are taken the Feature is **not Complete**: a probe whose failure arm has never been exercised is a probe that has only been shown to succeed. `-16` is **not** in that category and must not be counted with them — its criterion 4 is unobserved because the equipment to observe it has not been built, which is honest debt of a different and more expensive kind.

**Not release-assured, and that is a different ladder.** Every one of the sixteen holds assurance state `specified`; `qualified-platforms.tsv` holds zero qualified platforms, so assurance `verified` is closed to every Story in this project until an `ADR 0005` campaign runs (`06A` §2). No `PERF-*` release gate closes on anything here, and this Feature's own non-goals bar any timing claim outright.**
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
| [`STORY-P1-09-01`](../stories/STORY-P1-09-01.md) | The kept link: RP1 presence proven by validated identity readback, absence reported honestly | In progress — criteria 1, 2 and 3 met; criterion 4 is met in its present arm only — no boot has been taken with `pciex4_reset=0` deliberately removed |
| [`STORY-P1-09-02`](../stories/STORY-P1-09-02.md) | The PHY answers: management-port identification and latched link state — the device sees the cable | In progress — criteria 1, 2 and 3 met; the missing thing is the cable-out capture: a persisting `link=down` has never been observed, only the boot-time snapshot taken mid-autonegotiation |
| [`STORY-P1-09-03`](../stories/STORY-P1-09-03.md) | The beacon: one pinned broadcast frame, transmit-only, receive left disabled | Verified (functional) 2026-08-05 — criteria 1, 2 and 3 host-Green; criterion 4 met off the wire, twelve whole frames byte-identical to `beacon_frame(seq)` including the Ethernet header. This is the Feature's exit criterion |
| [`STORY-P1-09-04`](../stories/STORY-P1-09-04.md) | The release: PHY reset on RP1 GPIO 32 driven low-then-high before the scan (`LE-68`'s closure path) | Verified (functional) 2026-08-05 — criteria 1, 2 and 3 host-Green; criterion 4 Green on silicon: linkwatch `Down -> UP at 1000 Mbps` and `PHY=0x600D84A2` at address 1. `LE-68` closes here |
| [`STORY-P1-09-05`](../stories/STORY-P1-09-05.md) | The heartbeat: serial `TOS64-BEAT/1` every period and a verdict-colored bounce on the splash surface — untimed diagnosis under the owner's constraints | In progress — criteria 1, 2 and 3 met; neither arm criterion 4 names was observed — serial is dead and no capture shows the block moving; the outcome arrived through the canvas console instead and no Test amendment records that substitution |
| [`STORY-P1-09-06`](../stories/STORY-P1-09-06.md) | The watch: the park loop re-reads the link once per second and starts the beacon whenever the wire trains — no bench-tuned timing constant anywhere | Verified (functional) 2026-08-05 — criteria 1, 2 and 3 host-Green; criterion 4 met: the watch resolved on silicon (`BOARD VERDICT 3`) and the `TOS64-PRESENT/1` frames were captured 2026-08-05 |
| [`STORY-P1-09-07`](../stories/STORY-P1-09-07.md) | The confession: discovery's first refused rung counted out on the proven lamp as a pinned blink pattern — refusals only, the healthy pulse undiluted | Verified (functional) 2026-08-05 — every criterion Green on silicon 2026-08-03; criterion 4 read at three depths (3, then 16, then the unrefused plain pulse) |
| [`STORY-P1-09-08`](../stories/STORY-P1-09-08.md) | The second look: while discovery reports absence the probe re-runs each park-second — gate discipline and the exactly-once release preserved; a late `DL_ACTIVE` runs the pipeline once | Verified (functional) 2026-08-05 — criteria 1, 2 and 3 host-Green; criterion 4 Green on silicon in both arms: the confession sharpened past 3, then the chain cleared entirely |
| [`STORY-P1-09-09`](../stories/STORY-P1-09-09.md) | The window programmed, not presumed: a window-class refusal writes the capture's recorded mapping once and revalidates — belief from the readback alone, the second verdict final | Verified (functional) 2026-08-05 — criteria 1, 2 and 3 host-Green; criterion 4 Green on silicon, and the pinned mapping was independently corroborated against the live Pi OS controller registers |
| [`STORY-P1-09-10`](../stories/STORY-P1-09-10.md) | The introduction: bus numbers, forwarding window and memory decode programmed at bring-up size with both vendors verified first — refusals honest end-to-end as codes 14 and 15 | Verified (functional) 2026-08-05 — criteria 1, 2 and 3 host-Green; criterion 4 Green on silicon, and the pinned bridge dwords were corroborated by a live `setpci` capture under working Ethernet |
| [`STORY-P1-09-11`](../stories/STORY-P1-09-11.md) | The spelling: seven fixed decimal digit groups (code then sixteen decisive bits, ones first, zero as ten blinks) — the owner's readout design; supersedes the single-count pattern | Verified (functional) 2026-08-05 — criteria 1, 2 and 3 host-Green; criterion 4 Green on silicon: seven groups decoded to 16/57005, and that number redirected the Feature |
| [`STORY-P1-09-12`](../stories/STORY-P1-09-12.md) | The current: the clocks block read before it is believed, both gateable Ethernet clocks enabled by readback-validated pinned writes, running polled under an attempt budget — codes 16–18 | Verified (functional) 2026-08-05 — every criterion Green on silicon, criterion 5 observed in **both** arms: the refusal arm spelled 16/57005 and the success arm answered `0x0007` downstream |
| [`STORY-P1-09-13`](../stories/STORY-P1-09-13.md) | The address nobody wrote: each endpoint BAR sized by the architectural probe, assigned from the conviction capture, believed from mask and readback, memory-enable strictly after — codes 19–20 | Verified (functional) 2026-08-05 — every criterion Green on silicon 2026-08-04, criterion 5 in its success arm: identity 0x0109 where the poison was, PHY identified, the wire trained to gigabit |
| [`STORY-P1-09-14`](../stories/STORY-P1-09-14.md) | The park loop speaks: the beat line carries the park verdict — watch alive/dead/none, a stopped transmit with its error — the three silences the first trained wire exposed | Verified (functional) 2026-08-05 — every criterion Green on silicon 2026-08-04, criterion 4 answered first boot: `STOPPED REASON=TIMEOUT` cleared the watch and convicted the transmit's DMA inbound path |
| [`STORY-P1-09-15`](../stories/STORY-P1-09-15.md) | The unclaimed fetch: the root complex's three inbound DMA windows — both register families, twelve dwords derived from the capture — written once inside `establish` and believed only from readback — codes 21–22 | Verified (functional) 2026-08-05 — every criterion Green on silicon 2026-08-04, criterion 5 in its success arm: the beat line walked `STOPPED REASON=TIMEOUT` → `STATE=BEACONING`, and it was re-observed on the netbooted image 2026-08-05 |
| [`STORY-P1-09-16`](../stories/STORY-P1-09-16.md) | GEM receive, one frame, fail-closed: a ring of one wrapped descriptor into a **second** pinned region behind a hardware address filter and a MAC-enforced size bound, enable written strictly last, every frame classified by a total function and counted — nothing interpreted | In progress — criteria 1, 2 and 3 host-Green 2026-08-06; criterion 4 blocked on the board and on a host-side sender, which does not exist yet |
| [`STORY-P1-09-17`](../stories/STORY-P1-09-17.md) | The admitted verb: a fixed-width `TOS64-CMD/1` envelope classified by a total function over fixed offsets, a two-row deny-by-default answer-only verb table (`PING`, `STATUS`), every refusal spoken on the wire, the answer rate beat-bounded — the moment a received frame first means something, with `-16`'s expiring absence argument re-made rather than cited | Specified — proposed under `hand-2026-08-07/10A` §3 S2 and blocked on the owner's sprint-rule sentence (S4); charter reading and suite specification written, no code until it is spoken |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) ·
implementation **C1** · subject **C1/C2** · boundary tests **BND-03, -06, -07, -17** ·
**PD-07, PD-10, PD-12, PD-14** · **RCG-01, RCG-13, RCG-14**.

Hostile inputs, enumerated: the firmware's PCIe window state (kept, reset, or
half-configured); every RP1 and GEM register readback value; PHY identifier and
status registers and every MDIO response; data aborts on the kept window; clocks
or link states that never settle; and **since `STORY-P1-09-16`, raw Ethernet frames
chosen by an untrusted peer on the cable** — the first input this project has ever
taken from outside its own image. Every readback is validated before belief, every
poll is budget-bounded, and every hostile or absent answer resolves to the same
fail-safe park with an honest `TOS64-LINK/1` report.

The Ethernet block is treated as a compromisable C2 subject. It holds **two**
pinned, bounded grants that do not alias: the transmit staging region the CPU
writes and the device reads, and a separate receive region the device writes.
Remote bytes are data, never code (`RCG-01`) — and on this Feature they are still
not *read* in any sense that matters: admission compares six payload bytes, a
destination and an EtherType, then increments a counter. No value taken from a
frame selects a branch, an address, an offset or a size anywhere in the image, so
`C1` has an input path and no parser (`BND-03` satisfied by absence). That
property is the containment argument, `LE-67` records it, and it expires the
moment a received frame is allowed to *mean* something.

## Exit criteria

**A laptop connected peer-to-peer captures the board-present beacon, and the
serial protocol is unchanged.** Concretely:

- `TOS64-LINK/1` reports RP1 presence/absence, PHY identity, and link state over
  the UART after the verdict — both outcomes honest, neither hanging.
- The beacon frame appears in a stock host packet capture and is byte-identical
  to the pinned frame the host-side tests already assert. **MET 2026-08-05.**
  Twelve whole beacon frames (seq 5964–5975) were captured off the cable by
  `ti64dink --live 12 --raw` — Npcap, **unelevated**, stock host — and compared
  to `gem::beacon_frame(seq)` **byte for byte including the 14-byte Ethernet
  header**, by `gem::tests::the_captured_beacon_is_byte_identical_to_the_built_frame`.
  Raw evidence: [`goals/reports/beacon-frames-2026-08-05.txt`](../reports/beacon-frames-2026-08-05.txt),
  `include_str!`d by the test so the bytes a Report cites and the bytes the test
  asserts are one copy and cannot drift. Three properties make it evidence rather
  than arithmetic: the **header is included**, so the destination MAC, source MAC
  and EtherType are compared rather than skipped; the **sequence is an input** read
  from the file, not derived from the frame under comparison, so the test cannot
  compare a frame to itself; and it was **verified to fail** — flipping one byte of
  one captured frame fails it with the frame and offset named. Captured from a
  *late* attach at seq ~5964, so the beacon is unchanged deep into a run and not
  merely correct in the frames after boot.
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
