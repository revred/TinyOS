# Spoor Transport Architecture — the Observability Substrate for Physical AI

Status: **draft / `FEAT-P1-10`. The on-wire format is implemented and host-tested
(`kernel::spoor_wire`, `STORY-P1-10-01`); the board-side stamping and egress are
`STORY-P1-10-02`. The inbound direction is specified in §7 and deliberately not built.**

Owning Feature: [`FEAT-P1-10`](../goals/features/FEAT-P1-10.md) under
[`EPIC-P1`](../goals/epics/EPIC-P1.md) — Determinism Proof.

---

## 1. The claim this document exists to serve

**A spoor is to a physical system what a token is to a language model.**

That is the owner's framing and it is load-bearing, so it is worth stating what it commits
us to rather than treating it as a slogan. A language model's token stream is the *complete
observable record* of what the model did: uniform, ordered, cheap to emit, and the substrate
on which measurement, replay, billing, debugging and training all sit. Claiming the same for
spoors means the spoor stream must be:

- **Uniform.** One fixed-width atom, not a mixture of event shapes. A spoor is exactly 64
  bits, always ([`kernel::spoor`](../os/src/kernel/src/spoor.rs)).
- **Complete.** Everything the system decides is stamped, not a curated subset. This is the
  part that is *not yet true* and that `FEAT-P1-10`'s later Stories exist to make true.
- **Ordered, with loss accounted.** A stream that silently drops is a stream that lies. §4.
- **Cheap enough to always be on.** If stamping is expensive it gets disabled in production,
  and an observability substrate that is off during the interesting run is not one. §2.

The consequence that shapes every decision below: **the board emits, it does not interpret.**

## 2. Why nothing is formatted on the board

A spoor is packed into a `u64` for speed — one store, no allocation, callable from a
real-time path. Rendering one into text on the board would spend exactly what the packing
saves, and spend it on the hot path, in order to save work on a laptop that has cycles to
spare.

So the wire carries **raw packed records**. The board's entire cost per spoor is:

1. one `u64` store into the journal ring (`SpoorJournal::append`, no branch beyond the wrap),
2. a bulk copy of a contiguous run of `u64`s into the frame buffer at drain time,
3. one descriptor kick per frame, amortised across up to 184 records.

There is no formatting, no string handling, no per-record branching, and no allocation
anywhere on that path. Decoding — categories, actors, actions, outcomes, human-readable
names — happens entirely on the host, against the same enum tables the kernel compiled.

**The journal ring is a jitter buffer, not storage.** `SpoorJournal` overwrites its oldest
entry when full, which is right for a crash dump and wrong for a stream. Under this Feature
the wire is the destination and the ring only absorbs bursts between drains; a ring that
wraps before a drain is a measured loss (§4), not a silent one.

## 3. Frame format

One Ethernet frame, EtherType `0x88B5` (IEEE 802 local experimental — the same value
`FEAT-P1-09`'s beacon uses, so one capture filter sees the whole TinyOS conversation).

```
 offset  size  field
 ------  ----  ----------------------------------------------------------
      0     6  destination MAC — broadcast (ff:ff:ff:ff:ff:ff)
      6     6  source MAC — BEACON_SOURCE_MAC, locally administered
     12     2  EtherType — 0x88B5
 ---- payload begins ----
     14     8  magic — "SPOORJ01" (spoor_journal::JOURNAL_MAGIC)
     22     8  seq — u64 LE, sequence number of the FIRST record in this frame
     30     2  count — u16 LE, number of records (0..=184)
     32     2  flags — u16 LE, reserved, written zero
     34     4  padding — zero, so records begin 8-byte aligned
     38   8*n  records — n × u64 LE, packed spoors, verbatim
```

Payload length is exactly `24 + 8 * count`, bounded at 1496 bytes, so a full frame is 1510
bytes on the wire — inside a standard MTU with no jumbo frames and no fragmentation.

**The magic is not new.** `SPOORJ01` and the packed-`u64` record layout are already what
[`kernel::spoor_journal`](../os/src/kernel/src/spoor_journal.rs) declares as its on-disk
shape. One host parser therefore reads a live capture and a journal file with the same code,
and a captured stream can be replayed into any tool that consumes journals. That was a
deliberate refusal to invent a second format.

**Records are 8-byte aligned** because the payload is built by bulk copy of packed `u64`s;
the board should never be issuing unaligned stores to construct a diagnostic frame.

## 4. Loss is measured, never hidden

The link is unreliable broadcast. There is no acknowledgement, no retransmission, no flow
control and no connection — deliberately, because all four are state, and state is the thing
that makes a receiver attackable (§6).

The `seq` field is the sequence number of the *first* record in the frame. A host that has
seen a frame knows the next sequence to expect is `seq + count`; any gap is an exact count of
records that were lost, whether they were dropped by the ring, by the MAC, or by the network
stack on the far end.

This is why the counter is **64-bit**. A 32-bit sequence wraps after ~4×10⁹ records, which on
a continuously streaming system is well under an hour at even modest rates, and a wrapped
counter turns drop accounting from a measurement into a fiction. At 64 bits it cannot wrap
within the service life of the hardware.

A stream that cannot say what it dropped quietly lies about what it saw. This format can
always say.

## 5. Why the frame is MTU-sized

The first draft of this format carried 16 records per frame. That was sized for a diagnostic
trickle, and it was wrong for the claim in §1: if the spoor stream is the system's observable
behaviour, it is continuous and high-rate by nature and the frame must not be the limiting
factor. 184 records per transmit amortises the descriptor kick across an order of magnitude
more events, using the *same single pinned buffer* `LE-67` constrains the design to.

## 6. Attack surface — what is actually true

**Today the board only transmits.** There is no inbound parser at all;
[`gem.rs`](../os/src/hal-arm64/src/gem.rs) enforces this with a test named
`no_path_in_this_module_ever_enables_receive`. A parser that does not exist cannot be
attacked. That is not a mitigation, it is an absence of surface.

**The format itself is deliberately hostile to the usual defect classes.** Every field is
fixed width. There is no length field an attacker can inflate to steer a read, no options
region to walk, no fragmentation and therefore no reassembly, and no state machine and
therefore no state confusion. The one field that could be inflated — `count` — is bounded
against a compile-time constant *before* it is used to index anything, and a frame that
claims more records than it carries is rejected by comparing against its own length. Those
are the classes that account for most of the history of IP stack vulnerabilities, and they
are not expressible here.

**What this is not.** This link has:

- **no confidentiality** — every record is readable by anyone on the cable;
- **no authenticity** — any device on the cable can forge a well-formed frame trivially;
- **no protection against a hostile device** — `LE-67` records that GEM DMA on the Pi 5 runs
  with no IOMMU, so a compromised or confused device can read and write RAM the grant never
  named. No protocol design closes a hardware exposure.

It is therefore an excellent **point-to-point bench and deployment link** and is **not** a
transport to expose to an untrusted network. Any claim stronger than "minimal attack surface,
no confidentiality, no authenticity" would be the kind of unqualified assertion
[`ADR 0005`](adr/0005-arm64-real-time-tier-is-conditional-on-secure-world-qualification.md)
exists to prevent.

## 7. The inbound direction — specified, not built

Bidirectional exchange is the obvious next want and it is a genuinely different risk class,
so it is separated here rather than folded in.

Enabling GEM receive means:

- **Reversing an explicit, tested refusal.** `no_path_in_this_module_ever_enables_receive` is
  a deliberate guarantee; changing it is a Security Charter matter, not a feature toggle.
- **Rule 9 of [`agent.md`](../agent.md).** *Remote bytes are data, never code.* An inbound
  path must never create executable memory, and any command grammar must be a fixed,
  bounded, non-Turing vocabulary with no path to code admission.
- **`LE-67` becomes load-bearing.** Today "receive disabled" *is* the containment story for
  a device with no IOMMU. Turning receive on removes the containment argument and does not
  replace it; the replacement is `EPIC-P3`'s NIC/DMA C2 device service with `SEC-18` evidence.
- **Authenticity becomes mandatory, not optional.** Egress-only tolerates an unauthenticated
  link because forging a frame achieves nothing. The moment the board acts on what it
  receives, an unauthenticated link means anyone on the cable commands the machine.

The design that would satisfy these: a fixed-width command record (same shape discipline as
§3), a bounded vocabulary enumerated at compile time, no variable-length fields, a replay
counter, a message authentication code over each record, and a receive path that copies into
a fixed bounded buffer and refuses anything that is not exactly the expected size. That is a
Feature with adversarial tests and `PD-*` contracts, and it is not this one.

**Deployment over Ethernet** is a separate question again and has a charter-neutral answer:
Pi 5 **firmware** network boot (TFTP, `BOOT_ORDER` in EEPROM) loads the image before TinyOS
exists, so TinyOS never admits code and rule 9 is not engaged. That is the path to
investigate; TinyOS receiving and executing an image at runtime is the path that requires all
fourteen `RCG-*` gates and should not be taken casually.

## 8. Host side

The decoder is a C# console application under `work/tools/`, per the standing rule that
board-session host tooling is C# (`tos64-cardswap` and `tos64-linkwatch` are the pattern).
It grows into **Ti64Dink**, the host application named in `FEAT-P2-10`.

It must, in order of importance:

1. **Report loss.** Every gap in `seq` is printed as an exact count. A decoder that hides
   drops defeats §4.
2. **Decode against the kernel's own vocabularies**, so an unknown discriminant is reported
   as unknown rather than guessed — `Spoor::decode` already fails closed on exactly this and
   the host must mirror it.
3. **Never require elevation.** This is why the format sits on raw `0x88B5` while a capture
   still needs a privileged reader today; §9 records the open question.

## 9. Open questions

- **Unprivileged host capture.** Raw `0x88B5` needs `pktmon` (admin) or a capture driver on
  Windows. Emitting each envelope *additionally* as a UDP broadcast would let an
  unprivileged socket read the stream forever, at the cost of ~28 bytes of constant header
  and a second emission path. This buys the *host* convenience and buys the board nothing,
  so it is recorded as a shim, not as the protocol.
- **Ring sizing and drain cadence.** `SPOOR_JOURNAL_CAPACITY` was chosen for a crash-dump
  ring. The right size for a streaming jitter buffer is a function of burst rate against
  drain period and has not been measured.
- **Completeness.** §1 claims the stream is the system's observable behaviour. Until the
  `dispatch`, `lock`, `wcet` and `actuation` call sites stamp on the AArch64 path, it is the
  *boot and park* behaviour only, and every Report must say so.
