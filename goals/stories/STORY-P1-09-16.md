# STORY-P1-09-16 — GEM Receive: One Frame, Fail-Closed

Status: **In progress — criteria 1, 2, 3 and 5 host-Green (5 added and Green 2026-08-07: the descriptor is handed back on the healthy path and on no error arm, decided by a pure `beat_plan` the host tests exhaust); **criterion 4's ACCEPT arm met on silicon 2026-08-08** ([`19A`](../../session/hand-2026-08-08/19A-the-ear-is-deaf-on-arrival.md)): five `ti64dink --send ping` frames moved the wire-visible row to `state=listening accepted=5 refused=19 dropped=19` — the board counted a frame the host sent, and then counted four more, which is the difference between an ear and a fluke. Its four **refusal arms** (`unicast`, `ethertype`, `prefix`, `notforus`) remain: four commands and one capture. **Criterion 3 was amended the same day on the owner's ruling** — `BNA` is a counted drop and only `OVR` is terminal (`LE-118`), because the ear was otherwise deaf on arrival on any segment carrying broadcast; `ReceiveError::BufferUnavailable` is removed rather than left unreachable, and the drop is counted and spoken. Assurance state `specified`.**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: [`session/hand-2026-08-06/03B-the-arms-are-built-the-board-booted-them-and-nobody-read-the-wire.md`](../../session/hand-2026-08-06/03B-the-arms-are-built-the-board-booted-them-and-nobody-read-the-wire.md)

## Description

The board can speak and cannot listen. Everything downstream of that — that
Ti64Dink cannot start, that `0/98` Stories are assurance-verified, that every
interaction with this OS costs a rebuild and a power cycle — follows from it,
and `03B` §5 named it as the one thing standing between this project and an
operating system rather than an instrumented machine.

This Story is step one of the short path it laid out, and deliberately only
step one: **the receive ring, one pinned region, an EtherType filter, and a
bound on what is accepted. Nothing above it.** The success criterion is one
`TOS64-*` frame arriving from the host and being *counted* on the canvas. Not
parsed, not answered, not acted on — counted. Step 2 (a command answered end to
end) is a separate Story and needs this one's containment argument to hold
first.

**Receive is enabled through exactly one function, in exactly one order, into
exactly one bounded region, and every frame is classified by a total function
before anything counts it.** The Feature's other fourteen Stories validate
readbacks from a device the board owns. This one validates bytes chosen by
something else, which is a different kind of input and is treated as one.

## The containment argument, re-argued rather than inherited

`LE-67` recorded the transmit path's containment as *"the single static buffer
plus receive disabled, not device isolation"*, and its owner path said **receive
stays disabled** until the `EPIC-P3` C2 device service lands. That sentence was
load-bearing, this Story falsifies half of it, and the `SECURITY_CHARTER.md`
read that `03B` §5 required is the reason the rest of this section exists rather
than a line saying `LE-67` still applies.

**What does not change, and it is more than it looks.** The GEM is already a
bus-master with no IOMMU. A *malicious* device on this path could already read
and write RAM the grant never named — that is `LE-67`'s whole content and it was
true the moment transmit worked. Receive does not widen what a compromised
device can reach, because "arbitrary" has no wider setting. Charter PD-10's
device isolation is unmet on this path today and is unmet by exactly the same
amount tomorrow.

**What does change, and it is the thing worth the paragraph.** Until now every
byte in the image originated inside the image. Receive makes a **remote peer** an
input source for the first time in this project's history — the charter's
"hostile bytes" arrow, at its very first instance. The containment for *that* is
not device isolation and cannot be, so it is built from four things that do
exist, each of which is a test and not a comment:

1. **A second, separate pinned region.** Receive does **not** share
   `BEACON_MEMORY`. A device write can land in `RECEIVE_MEMORY` and nowhere
   else the receive grant names, so a confused write cannot corrupt the frame
   the board is about to transmit — which would turn an inbound fault into an
   outbound lie, and this project's entire evidence chain is outbound frames.
   The grant is now two regions rather than one; that is a real widening,
   stated here rather than left for a reader to notice, and it is the narrowest
   widening that keeps the two directions from aliasing.
2. **A hardware address filter.** `SA1B`/`SA1T` are programmed with the board's
   own MAC, and copy-all-frames is asserted never set. On a shared segment the
   board will not even DMA a frame that was not addressed to it or broadcast.
3. **A size bound the MAC itself enforces.** `DMACFG`'s receive buffer-size
   field is programmed from the region's actual size, refused rather than
   rounded if it will not encode, and written **before** `NCR.RE`. A ring of one
   descriptor with `WRAP` set means the MAC cannot walk to a second address, and
   cannot place a second frame until software explicitly hands the descriptor
   back.
4. **A total classifier that does not trust the device.** The frame length the
   descriptor reports is bounded-checked against the region before it is used
   as a length, even though point 3 makes an over-length report impossible. A
   classifier that believes the length word is a classifier that indexes out of
   the buffer the day the device is wrong, and "the day the device is wrong" is
   the premise `FEAT-P1-09` contracts.

**And the part that makes the whole argument cheap: nothing interprets the
bytes.** Admission compares six payload bytes to `TOS64-`, the EtherType to
`0x88B5`, and the destination to two known addresses. No value taken from the
frame selects a branch, an address, an offset or a size anywhere in the image.
`C1` gains an input path and gains no parser, which is `BND-03` satisfied by
absence and is why this Story can be honest about having no IOMMU: there is no
reachable behaviour for a crafted frame to reach. **That property is the
containment.** The moment step 2 makes a frame *mean* something, the argument
above stops being sufficient and has to be made again — and it should be made
again, not cited.

`LE-67` is updated, not closed: its owner path now records that receive is
enabled under this Story's four-part argument, that the DMA grant is two pinned
regions, and that the `EPIC-P3` C2 device service with `SEC-18` evidence remains
the real discharge.

## Depends on

- `STORY-P1-09-03` — the transmit path, the DMA offset, and the staging
  discipline this Story mirrors in the opposite direction.
- `STORY-P1-09-06` — receive is armed only once a link has resolved up. A
  receiver on a dead wire is an enabled DMA engine with nothing to show for it.

## Acceptance criteria

1. **The enable order is pinned, and `NCR.RE` is strictly last.** Address filter
   bottom then top, `DMACFG` with 64-bit addressing and the size bound, queue
   base low then high, stale status cleared, then receive enable — asserted as
   an ordered list over the scripted seam, because every one of those writes is
   a grant and the last one is what makes the grants live.
2. **Everything the device reports is bounded before it is believed.** The
   buffer-size encoding refuses rather than rounds; a misaligned buffer address
   is refused (the low two bits are ownership and wrap flags); the descriptor
   classifier is total, with a fragment, a zero length and an over-length each a
   distinct named refusal.
3. **Admission is a six-byte comparison with a distinct refusal per condition,
   and it is fail-closed.** Too short, not addressed here, wrong EtherType and
   not an envelope are counted separately; a receive overrun or a
   buffer-not-available permanently disables receive and says so on the canvas.
   No path re-enables receive after an error, on that pass or any later one.
4. **Board: the board counts a frame the host sent.** The canvas `TOS64-RX/1`
   row moves to `accepted=1` when the host transmits one `TOS64-` framed
   `0x88B5` packet, and a frame of any other EtherType increments `refused`
   instead — **both arms**, because an accepted count proves only that the board
   can hear, and the declining is what this Story is answerable for.

   The host side of this criterion exists (`LE-93`, closed 2026-08-06):
   `ti64dink --send <arm>` for `ping`, `unicast`, `ethertype`, `prefix` and
   `notforus`. `notforus` is the arm worth reading twice — it expects **neither**
   counter to move, because the GEM's hardware address filter should drop the
   frame before DMA, and a moved `refused` count would mean the filter is not
   containing what the argument above assigns it. Two of the board's refusals
   are recorded as unreachable from a host rather than quietly omitted:
   `TooShort` cannot be put on a wire because the NIC pads to 60 octets, and
   the three descriptor refusals are reachable only by a lying device.

5. **The descriptor is handed back, and only on the healthy path.** After a
   frame is classified and counted, the single descriptor is returned to the
   MAC — address preserved, `WRAP` kept, ownership cleared — **at most once per
   park beat**. One frame classified, one descriptor re-armed, per beat; a
   second frame arriving inside a beat has nowhere to land until the next
   hand-back, which is the same bounded-poll discipline every other channel on
   this board keeps.

   Added 2026-08-07 as an amendment rather than a new Story, because the
   Story's subject was always *"the board can be reached, fail-closed"* and an
   ear that stays deaf after one frame fails that subject's own sentence.
   `TOS64-RX/1 STATE=STOPPED REASON=NOBUFFER ACCEPTED=0 REFUSED=0`
   ([`07F`](../../session/hand-2026-08-07/07F-the-relay-was-never-the-roadblock.md)
   §7c) is the wire's own proof: a ring of one wrapped descriptor that is never
   handed back is a doorbell, not an ear, and no conversation survives it.

   **What this does not weaken, asserted rather than assumed.** Criterion 3's
   refusal stands untouched: an overrun and a buffer-not-available are still
   *terminal*, and a host test exhausts the four status arms against all five
   descriptor states to prove that no error arm hands the descriptor back — on
   that pass or any later one. The decision is a pure function
   (`gem_receive::beat_plan`) rather than a branch inside the aarch64 glue,
   precisely because the glue is the one part of this path no host test can
   reach and "the error arm does not re-arm" is the claim that must not live
   somewhere unreachable.

   A second correction rides the same amendment: a descriptor refusal is now
   counted under its own name instead of being relabelled `TooShort` by the
   glue, which was a small lie about which refusal had occurred.

## The absence argument, and the date it expired

This Story's containment rested on a property with an expiry stated in its own
text: *"nothing interprets the bytes … the moment step 2 makes a frame mean
something, the argument above stops being sufficient and has to be made again —
and it should be made again, not cited."*

**That moment arrived on 2026-08-07.**
[`STORY-P1-09-17`](STORY-P1-09-17.md) admits a verb, so a received frame now
means something, and the argument above is superseded by that Story's charter
reading — a fixed-width envelope classified over fixed offsets, one
input-derived selection bounded by a deny-by-default table's own length, two
answer-only rows justified against `PD-02`, every refusal spoken, and the answer
rate beat-bounded against amplification. Recorded here, dated, so no future
reader inherits an absence that no longer holds (`STORY-P1-09-17` criterion 5).

The four-part containment of this Story — separate pinned region, hardware
address filter, MAC-enforced size bound, total classifier — is *unchanged* and
still load-bearing. It is the fifth part, "and nothing interprets the bytes",
that expired.

## Named debt this Story leaves open

- `LE-67` — updated, not closed. Two pinned regions and no IOMMU; `BND-07` and
  `SEC-18` evidence stays narrow until the `EPIC-P3` C2 device service lands.
- `LE-26` — unchanged; a counted frame is not a transport.
- **Nothing answers.** The board still cannot be *told* anything — it can now be
  *reached*. Step 2 of `03B` §5 (host sends a framed request, board acts, board
  answers) is where the round trip exists, and until it does, the honest claim
  for this Story is bidirectional wiring with a unidirectional protocol.
- The received count reaches the canvas and not the wire. Every other channel on
  this board reports over the cable, and this one deliberately does not yet: an
  extra transmit per beat is a change to the beacon cadence a capture window is
  sized against (`hand-2026-08-06/03B` §3a), and it belongs with step 2's
  answer frame rather than ahead of it.

## Progress, 2026-08-06

| Criterion | State |
|---|---|
| 1 — enable order pinned, `RE` last | **Green (host).** Register order asserted as an exact ordered list; a double that fires on any `NCFGR` write setting copy-all-frames. |
| 2 — bounded before believed | **Green (host).** Size encoding refuses non-multiples and overflow; misaligned buffer addresses refused; the classifier is total with three distinct refusals. |
| 3 — admission and fail-closed | **Green (host).** Four distinct admission refusals; overrun and buffer-not-available each disable receive terminally. |
| 5 — the descriptor is handed back, healthy path only | **Green (host), 2026-08-07.** `beat_plan` is total over four status arms × five descriptor states; the two error arms hand nothing back on every one of them, and the hand-back preserves address and `WRAP` and returns ownership. |
| 4 — board counts a host frame | **Blocked on hardware only.** The sender exists as of the same day (`LE-93` closed): `ti64dink --send <arm>` transmits five named arms covering both halves, and every arm's predicted verdict is asserted against `gem_receive::admit` by a host test, so the criterion's expectations are machine-checked before a board is powered. What remains is one power cycle and five commands. |

## Tests

[`TEST-P1-09-16-A`](../tests/TEST-P1-09-16-A.md) — written before
implementation, per the TDD mandate.
