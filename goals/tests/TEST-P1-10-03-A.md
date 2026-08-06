# TEST-P1-10-03-A — The Wrapper Must Be Transparent, and Must Not Be Routable

Status: **Specified — written retrospectively 2026-08-05 to close `LE-73`, and the retrospection is itself recorded: the implementation and its 8 tests existed before this document, which is the defect the loose end names rather than a clean TDD cycle**
Story: [`STORY-P1-10-03`](../stories/STORY-P1-10-03.md)
Tier: Host unit tests (`kernel::udp_wire` — header construction, checksum, transparency, refusal). **No hardware tier**, because nothing on the board calls this module; see the Story's debt section
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D07`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

## What this test is for

The wrapper exists to remove a host prerequisite, and it must do that **without becoming a
protocol of its own**. Two properties carry it, and a third bounds it.

**Transparency.** The payload is a `STORY-P1-10-01` frame and must arrive as one. If the
wrapper reformats, pads, truncates or reorders anything, then a host on the UDP path and a
host on the raw path are reading two different formats and the single-decoder property that
`STORY-P1-10-01` clause 2 established is gone.

**Non-routability.** A stream with no authenticity and no confidentiality must not be able to
leave the cable it was meant for. This is one byte, and one byte is a better enforcement than
a policy sentence.

**Bounded absence.** The reason this wrapper adds no attack surface is not that its parser is
careful — it is that there is no parser. The module only writes. The tests assert the absence
of the things a receiver could be steered by: no options region, no fragmentation, no state.
An absence is exactly the property that silently stops being true, which is why it is tested
rather than asserted in prose.

## Clauses

**Clause 1 — the checksum is right by the receiver's own rule.** Summing the header
*including* its checksum yields zero. Tested as the property a receiver checks, not against a
hand-copied constant, because the failure mode is silent: a wrong checksum means the host's IP
stack drops the datagram and the stream simply never appears, with nothing saying why.

**Clause 2 — the checksum is held to an outside answer.** A published worked IPv4 header
(`192.168.0.1` → `192.168.0.199`, TTL 64, DF set) whose checksum is `0xB861`. Clause 1 alone
would pass for an implementation that is self-consistently wrong; this clause is what makes it
arithmetic rather than tautology.

**Clause 3 — the datagram cannot be routed off the segment.** `TTL` is 1 and the destination
is the limited broadcast address `255.255.255.255`. Never a unicast route, never forwarded by
a conforming router.

**Clause 4 — there is never a fragment and never an option.** `IHL` is 5, so no options region
exists to walk, and the flags/fragment-offset word is zero, so reassembly is unreachable
rather than merely unused.

**Clause 5 — the three lengths agree.** The returned total, the IPv4 total-length field and
the UDP length field must all agree with each other and with the payload. A length that
disagrees with its buffer is the one field an attacker could steer a read with, so the
agreement is asserted rather than assumed.

**Clause 6 — the payload is the spoor frame unchanged.** A real `spoor_wire` frame is wrapped,
the payload region is compared byte for byte, and the frame is then decoded *through* the
wrapper: `seq`, `count` and `epoch` must read exactly as they were encoded. The epoch is
included deliberately — a host on this path must be able to tell which boot it is reading,
exactly as one on the raw path can (`STORY-P1-10-04`).

**Clause 7 — both ports are the spoor port.** Source and destination are both `6404`, so a
host binds one port and needs no knowledge of an ephemeral source.

**Clause 8 — refusal writes nothing.** A buffer too small returns `BufferTooSmall` with the
output untouched. A half-written buffer that returns an error is a buffer someone will
transmit.

## What this test does not cover

- **Anything on silicon.** There is no hardware clause because there is no hardware path:
  `kernel::udp_wire::encode` has **no callers**. The module is host-proven and unwired, and no
  Report may cite it as evidence that the board has ever emitted a UDP datagram.
- **The `PayloadTooLarge` arm.** Reachable only through a payload above `u16::MAX`, which
  `spoor_wire::MAX_PAYLOAD` makes unconstructible from any real frame. Untested and stated,
  rather than tested with a synthetic buffer that proves nothing about the caller.
- **Authenticity and confidentiality.** There is none of either, and the wrapper makes the
  stream *easier* to read by design — that is its entire purpose. Safe only while the board
  does not receive; see [`docs/spoor-transport-architecture.md`](../../docs/spoor-transport-architecture.md) §7.
- **The UDP checksum.** Written as zero, which IPv4 permits. The payload's integrity is not
  this layer's job and a wrong checksum here would cost a silent drop for no benefit.
- **`LE-67`.** GEM DMA runs with no IOMMU. No framing-level test says anything about a device
  writing RAM the grant never named.
