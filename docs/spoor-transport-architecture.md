# Spoor Transport Architecture — the Observability Substrate for Physical AI

Status: **draft / `FEAT-P1-10`. The on-wire format is implemented and host-tested
(`kernel::spoor_wire`, `STORY-P1-10-01`); the board-side stamping and egress are
`STORY-P1-10-02`, proven on silicon 2026-08-04 (`BOARD VERDICT 10`). The boot epoch and
the retained certificate (§4.1) are `STORY-P1-10-04`, host-Green with no board evidence
yet. The inbound direction is specified in §7 and deliberately not built.**

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
3. one descriptor kick per frame, amortised across up to 181 records.

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
     30     2  count — u16 LE, number of records (0..=181)
     32     2  flags — u16 LE, bit 0 = RETAINED (§4.1), rest reserved
     34     4  epoch — u32 LE, the boot that emitted this frame (§4.1); 0 = undeclared
     38   8*n  records — n × u64 LE, packed spoors, verbatim
```

Payload length is exactly `24 + 8 * count`, bounded at 1472 bytes, so a full frame is 1486
bytes on the wire — inside a standard MTU with no jumbo frames and no fragmentation.

**The `epoch` field is where the reserved padding went.** The original format held four zero
bytes at 34–38 explicitly *"so a future field does not have to move the records"*.
`STORY-P1-10-04` is that future field: it spends the padding and moves nothing, so a stream
captured before it existed still decodes record-for-record against a decoder built after it.
That is the entire return on having reserved the space, and it is worth naming, because the
alternative was a format version number and two parsers.

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

## 4.1 What a late listener cannot know, and the two things that fix it

§4 makes loss countable. It does **not** make the stream joinable, and those are different
properties. Two holes, both found by the owner reading `BOARD VERDICT 10`:

**If frame 0 is lost, the boot rungs are gone forever.** `MmuEnabled`, `GicRouted`,
`TickArmed` stamp exactly once, the drain clears the ring, and nothing re-sends them. A host
that missed that frame learns from the gap *how many* records it lost and never *what they
were* — and boot state is the least repeatable, most diagnostic part of the whole stream.
`BOARD VERDICT 10` exists only because a capture happened to be running across a power
cycle. Evidence by luck is not a channel property.

**A listener joining late cannot tell which boot it joined.** At `seq=25138` nothing
distinguishes "continuing normally" from "joined after a reboot I never saw". A sequence
number is a position *within* a boot and cannot express which boot it is a position in. A
host that assumed continuity across an unseen reboot would read a fresh stream as a
continuation and report tens of thousands of losses that never happened — §4's accounting
turned against itself.

### The boot epoch

Every frame carries a 32-bit `epoch`, fixed once at boot and identical on every frame that
boot emits — drained or retained. A change of epoch is a reboot, and a host reads it as one
rather than as loss.

**What it honestly is: a change detector, not an identifier.** The board has no persistent
store and no RTC. The epoch is derived from `CNTVCT_EL0` at kernel entry, so what varies
between boots is how many counter ticks firmware spent before reaching the kernel — real
variation, but the firmware's, not ours. A host can conclude *"this is a different boot"*
with high probability; it can never conclude *"this is boot number N"* or *"I missed exactly
two boots"*. `LE-74` records the limit and names what would remove it (the BCM2712 hardware
RNG, or firmware-persisted state — each its own bring-up). Zero is reserved for **not
declared**, so an unseeded board or an older image reads as an honest absence.

### The retained boot certificate

The boot prologue is held in a fixed buffer **outside the journal ring** and re-emitted every
`ANNOUNCE_EVERY` park passes (five, so roughly every five seconds). Any listener, joining at
any time, learns the boot state within a stated window.

Three properties make it honest rather than merely convenient:

- **Verbatim, not a summary.** The same packed `u64`s with the sequence numbers they were
  originally sent under. A host that missed frame 0 and one that saw it decode identical
  bytes. A re-stamp would carry fresh sequences and a fresh cost and would be a *different
  event* wearing the same name.
- **A consecutive run from `seq = 0`.** The certificate takes once-per-boot rungs while the
  run from zero is unbroken and closes permanently at the first record that is not one. A
  frame header carries one sequence and implies the rest follow consecutively, so a
  certificate assembled from scattered records would make its own header lie.
- **Marked on the wire.** `FLAG_RETAINED` is set, and a decoder must not apply `seq + count`
  to such a frame. `spoor_wire::FrameHeader::expected_next` returns nothing for a retained
  frame, so the phantom gap is *unreachable* rather than documented — the same posture as
  bounding `count` before using it to index.

**This is all egress.** No receive path, no new pinned buffer, no charter change, no
widening of `LE-67`'s exposure: one small frame every few seconds on the transmit path
`STORY-P1-10-02` already proved.

### Why this comes before asking

Two-way query/response (§7) is genuinely better than broadcasting hopefully, and it costs
enabling GEM receive — the one thing `gem.rs` enforces against with a dedicated test, and the
thing `LE-67` records as *the* containment story while there is no IOMMU. A retained,
re-announced, epoch-tagged stream means a listener never *has* to ask. Establishing that
first is what keeps the expensive answer an option rather than a necessity.

## 5. Why the frame is MTU-sized

The first draft of this format carried 16 records per frame. That was sized for a diagnostic
trickle, and it was wrong for the claim in §1: if the spoor stream is the system's observable
behaviour, it is continuous and high-rate by nature and the frame must not be the limiting
factor. 181 records per transmit amortises the descriptor kick across an order of magnitude
more events, using the *same single pinned buffer* `LE-67` constrains the design to.

The number is 181 rather than 184 because the payload is sized by the **larger** of the two
framings it travels in: raw `0x88B5` gives `14 + 24 + 181*8 = 1486`, and the IPv4/UDP wrapper
(`kernel::udp_wire`, §9) gives `14 + 20 + 8 + 24 + 181*8 = 1514` — exactly a maximum Ethernet
frame before the FCS. One constant keeps both inside an MTU, so fragmentation stays
unreachable on either path rather than merely unused on one.

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
4. **Report a reboot as a reboot.** An epoch change resets the host's expectation instead of
   producing a loss figure, and a retained frame is excluded from the arithmetic entirely.
   A decoder that got either wrong would turn §4's honest accounting into the loudest lie the
   tool can tell.
5. **Say what it could not learn.** A window with no frame 0 and no certificate in it means
   the boot state is unknown, and the tool says so rather than reporting the oldest record it
   happened to see as though it were the beginning.

Ti64Dink does all five. It also **checks the verbatim claim** when a capture holds both a
live frame 0 and a certificate for the same epoch, comparing them record for record and
reporting a disagreement as a defect rather than picking one.

## 9. Open questions

- **Unprivileged host capture.** Raw `0x88B5` needs `pktmon` (admin) or a capture driver on
  Windows. Emitting each envelope *additionally* as a UDP broadcast would let an
  unprivileged socket read the stream forever, at the cost of ~28 bytes of constant header
  and a second emission path. This buys the *host* convenience and buys the board nothing,
  so it is recorded as a shim, not as the protocol.
- **Ring sizing and drain cadence.** `SPOOR_JOURNAL_CAPACITY` was chosen for a crash-dump
  ring. The right size for a streaming jitter buffer is a function of burst rate against
  drain period and has not been measured.
- **Epoch entropy** (`LE-74`). The epoch distinguishes boots because firmware timing varies,
  which is borrowed entropy. Two boots that reached the kernel on the same counter tick read
  as one. A hardware RNG or persisted state would fix it; neither exists on this path yet,
  and no document may describe the field as a boot count until one does.
- **Announcement cadence.** `ANNOUNCE_EVERY = 5` is chosen, not measured — a trade between
  wire share and how long a session tolerates not knowing which boot it is watching. The
  same class of debt as ring sizing, and stated for the same reason.
- **Completeness.** §1 claims the stream is the system's observable behaviour. Until the
  `dispatch`, `lock`, `wcet` and `actuation` call sites stamp on the AArch64 path, it is the
  *boot and park* behaviour only, and every Report must say so.
