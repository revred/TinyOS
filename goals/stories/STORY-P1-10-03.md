# STORY-P1-10-03 — The Same Records, Readable Without a Driver

Status: **In progress — host-Green (8 tests in `kernel::udp_wire`) and **not on any transmit path**: `encode` has no callers, so nothing on the board has ever emitted one of these datagrams. Filed 2026-08-05 to close [`LE-73`](../assurance/loose-ends.tsv), which found the module joined to the spine by a citation resolving to nothing. The Story is written to the code that exists, not to the code the citation implied.**
Feature: [`FEAT-P1-10`](../features/FEAT-P1-10.md)
Architecture: [`docs/spoor-transport-architecture.md`](../../docs/spoor-transport-architecture.md) §9 (recorded there as a shim, not as the protocol)
Introduced in: filed in the `session/hand-2026-08-05/05A` session; the module it documents predates it

## Description

A shim, and it is important that it is named one. `STORY-P1-10-01` is the protocol; this is a
second framing of the identical payload that exists for one reason: **the host.**

**The prerequisite it removes.** The canonical spoor frame is raw EtherType `0x88B5` and stays
that way. Windows demultiplexes incoming frames by EtherType inside NDIS, in the kernel, and
discards anything with no registered protocol driver — so `0x88B5` never reaches user mode at
*any* privilege level. Reading it needs a signed kernel driver or a capture stack (Npcap,
`pktmon`), which is a large prerequisite to put in front of every future diagnostic session on
every future machine. Wrapping the same bytes in IPv4/UDP costs 28 bytes of almost entirely
constant header and removes that prerequisite completely: the host's own IP stack delivers the
stream to an unprivileged socket with nothing installed.

**What it deliberately is not.** There is no IP stack here — no ARP, no routing, no
fragmentation, no connection, no state. The destination is the limited broadcast address, which
needs no address resolution, and every header field is a constant or a length. It adds no
attack surface, because attack surface is a property of what the board *parses* and this module
only writes: receive is disabled and `gem.rs` enforces that with a test. The minimal-surface
argument for the raw format is untouched, because it was never an argument about egress.

**One field is a safety property.** `TTL` is 1. These datagrams are deliverable on the local
segment and can never be forwarded off it by any conforming router. A diagnostic stream with no
confidentiality and no authenticity should not be able to leave the cable it was meant for, and
one byte enforces that better than a policy does.

## Acceptance criteria

1. **The payload is the `STORY-P1-10-01` frame, byte for byte.** One decoder reads a raw capture, a UDP datagram and a journal file alike. A wrapper that reformats its payload has forked the format.
2. **The frame's own fields survive the wrapping.** `seq`, `count` and `epoch` decode through the wrapper exactly as encoded, so a host on this path can join a stream and tell which boot it is reading as well as one on the raw path can.
3. **The datagram cannot leave the local segment.** `TTL` 1, destination `255.255.255.255`; never a unicast route.
4. **Fragmentation and options are unreachable, not merely unused.** `IHL` 5 and a zero flags/fragment word, asserted.
5. **The IPv4 header checksum is correct by the receiver's rule and against an outside answer.** The failure mode is a silent drop by the host's own stack, which presents as "the stream never appeared" with no diagnosis anywhere.
6. **Nothing is written on refusal.** An encode that cannot fit leaves the buffer untouched rather than half-filled.
7. **A board emits one and an unprivileged host socket reads it.** *Not met, and not attempted* — see the debt below.

## Named debt this Story leaves open

- **`encode` has no callers.** The module is complete, host-Green and wired to nothing. Criterion 7 is unmet because no code path was ever built to meet it. Stated plainly because a fully tested module reads as a delivered capability, and this one is a capability that is available rather than in use.
- **The 181-record cost is already paid for a path nobody walks.** `spoor_wire::MAX_RECORDS` was reduced from 184 to 181 so that the *UDP* framing would fit a maximum Ethernet frame (`14 + 20 + 8 + 24 + 181*8 = 1514`), because it is the larger of the two framings. So every raw `0x88B5` frame the board has ever transmitted carries three fewer records than it could, to accommodate a wrapper that has never been used. A real cost, correctly reasoned at the time, and worth naming rather than leaving as an unexplained constant.
- **Emitting both framings doubles the transmit cost per drain.** The second emission is a second GEM transmit, and `STORY-P1-10-02` criterion 6 (stated cost) is not yet measured even for the first. Whether the shim is worth its wire share is a measurement nobody has made.
- **No authenticity and no confidentiality — and this framing makes the stream easier to read on purpose.** Any device on the cable already reads every record; the wrapper lowers the bar from "install a capture driver" to "bind a socket". That is the intent, and it is safe only while the board does not receive. See `FEAT-P1-10`'s exclusions and architecture §7.
- **`LE-67`** — GEM DMA with no IOMMU. A hardware exposure no framing closes.
- **The `PayloadTooLarge` arm is unreachable from any real frame** and therefore untested. `spoor_wire::MAX_PAYLOAD` makes a payload above `u16::MAX` unconstructible.

## Tests

[`TEST-P1-10-03-A`](../tests/TEST-P1-10-03-A.md) — **written after the implementation**, which is
the `LE-73` defect and not the TDD cycle this project requires. Recorded that way in the Test
document's own status header rather than backdated, because a spine that lets a retrospective
document present itself as a prior one is a spine that cannot be read as evidence.
