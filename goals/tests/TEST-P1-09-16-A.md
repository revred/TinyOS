# TEST-P1-09-16-A — One Frame In, Counted and Nothing Else

Status: **In progress — host clauses written Red first 2026-08-06, clause 10 (the hand-back) added and Green 2026-08-07; clause 8 awaits the board and the cable**
Story: [`STORY-P1-09-16`](../stories/STORY-P1-09-16.md)
Tier: Host unit tests (register order, descriptor layout, admission taxonomy, fail-closed state machine) **plus** a Tier 1 board run witnessed on the canvas
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`, `D20`
Security controls: `SEC-18`, `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-02`, `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: `D20` is selected as stated open debt
([`goals/assurance/open-debt.tsv`](../assurance/open-debt.tsv)) — the domain's
subsystem does not exist and none of its 25 guardrails can close on a single
counted frame. This Test raises no timing, throughput, measurement or
qualification claim.

## What this test is for

This is the first change in the project's history that lets bytes chosen by
something outside the image reach the board's RAM. Every other input the kernel
has ever validated came from a register the board itself owns. So the test's
subject is not "does receive work" — it is **what the board refuses**, and the
refusals are enumerated rather than sampled.

`TEST-P1-09-03-A` clause 4 asserted an *absence*: receive was never enabled and
no receive register was touched. That clause remains true of `gem.rs` and its
double still enforces it. This Test is where the absence ends, and it replaces it
with a stronger and much more specific claim: receive is enabled through exactly
one function, in exactly one order, into exactly one bounded region, and every
byte that arrives is classified by a total function before anything counts it.

## Specification

### 1. The enable order is the containment, and it is pinned (`SEC-18`, `BND-07`, `PD-10`)

**Given** the receive-enable sequence over the scripted seam,
**then** the register write order is asserted exactly: address filter bottom
word then top word, `DMACFG` (64-bit addressing **and** the receive buffer-size
bound) , receive queue base low then high, stale status cleared, and **`NCR.RE`
strictly last**.

The order is the property. A `RE` set before `RBQP` hands the MAC whatever
address that register happened to hold at reset and lets it write there; a `RE`
set before the buffer-size field lets it write past the end of a region that is
correctly addressed. Both are single-write mistakes with no symptom on a bench
where the register happens to read zero, which is why the order is a test and
not a comment.

### 2. The buffer-size bound is derived, refused when underivable, and never rounded (`SEC-20`, `BND-07`)

**Given** the buffer-size encoder,
**then** a size that is not a positive multiple of 64 bytes, or that will not fit
the field, is **refused** (`None`) rather than rounded to something the field can
hold; and the encoded value for the region's actual size is pinned.

A rounded-up bound is a grant the argument in the Story did not make.

### 3. The ring is one descriptor, wrapped, pointing at one region (`BND-07`, `PD-10`)

**Given** the ring builder,
**then** it produces exactly one descriptor: the buffer's low address with the
`WRAP` bit set and the ownership bit **clear** (the MAC owns it), the high half
of the address in the third word, and nothing else. A ring of one with `WRAP` is
the smallest structure the MAC can walk, and it means the second frame cannot
land anywhere until software has explicitly handed the descriptor back.

**And** a buffer address whose low two bits are not zero is **refused**: those
bits are the ownership and wrap flags, so an unaligned address silently becomes a
different address *and* a different ownership state.

### 4. Descriptor classification is total, and every refusal is distinct (`SEC-19`, `BND-03`)

**Given** the descriptor classifier as a pure function of the two descriptor
words,
**then** every input maps to exactly one of: the MAC still owns it; a whole frame
of a stated length; or one named refusal — a fragment missing start-of-frame or
end-of-frame, a zero length, or a length larger than the region the buffer-size
field bounds.

The over-length arm is the one that matters and it is deliberately kept even
though clause 1 makes it unreachable: it is the assertion that this code does not
*trust the device to have obeyed the bound it was given*. A classifier that
believes the length word is a classifier that will index out of a buffer the day
the device is wrong.

### 5. Admission reads six bytes and interprets none of them (`RCG-01`, `BND-03`, `SEC-19`)

**Given** the admission filter as a pure function of the frame bytes,
**then** a frame is admitted only if it is at least a header plus the envelope
prefix (20 bytes — short enough that the prefix comparison can run at all), its
destination is broadcast or this board's own address, its EtherType is `0x88B5`,
and its payload begins `TOS64-`; and each failing condition is a **distinct named
refusal**, counted separately, with the check order fixed so a frame wrong in two
ways reports the first thing wrong rather than whichever check happened to run.

**And** the test asserts what admission does *not* do: no field beyond those is
read, no length inside the payload is believed, no value from the frame selects a
branch, an address, an offset or a size anywhere in the image. Admitting a frame
increments a counter. That is the whole of it, and clause 9 records that keeping
it that way is the Story's entire safety argument.

### 6. The state machine is fail-closed, and every error arm is terminal (`SEC-20`, `PD-07`, `RCG-13`)

**Given** the receive status reader over the scripted seam,
**then** a receive overrun and a buffer-not-available are each a **distinct
driven rejection that permanently disables receive** — `NCR.RE` cleared, no
re-arm attempted on that pass or any later one, and the refusal spoken on the
canvas rather than relabelled as quiet.

Same discipline as the beacon (`STORY-P1-09-14`): a channel that reports its
failures as waiting diagnoses nothing. Unlike the beacon, this one also *stops
listening*, which is the fail-closed half — the safe state for an input path is
deaf, not retrying.

### 7. Promiscuous mode is never enabled, and that absence is tested (`BND-06`, `SEC-18`)

**Given** the full receive sequence,
**then** the seam double asserts that no write to `NCFGR` ever sets the
copy-all-frames bit, and that the receive path touches only the registers it
names. The hardware address filter is *part of* the containment argument, so
turning it off is exactly as much a defect as pointing the ring at the wrong
address.

### 8. Board: the board counts a frame the host sent (`BND-17`)

**Given** the board on the peer-to-peer cable and a host transmitting one raw
`0x88B5` frame whose payload begins `TOS64-`,
**then** the canvas `TOS64-RX/1` row moves from `accepted=0` to `accepted=1`
while `TOS64-LINK/1`, the beat line, the transcript rows and the splash stay
unchanged; and a frame with any other EtherType leaves `accepted` at zero and
increments `refused`.

**Both arms are required.** An accepted count with no refused arm proves the
board can hear; it does not prove the board can decline, and the declining is
the part this Story is answerable for.

### 10. The descriptor is handed back, and no error arm ever hands it back (`SEC-20`, `PD-07`)

**Given** the beat decision as a pure function of the receive status and the
descriptor state,
**then** exactly the healthy arms return the descriptor to the MAC — a whole
frame, and a descriptor whose contents cannot be a frame — and **neither error
arm does, on that pass or any later one**; and the hand-back preserves the
buffer address, keeps `WRAP`, and clears the ownership bit.

The claim is exhaustive rather than sampled: every combination of the four
status outcomes with all five descriptor states is enumerated, so "the error
arm does not re-arm" is a count and not a reading. It is a *pure function*
because the alternative is a branch inside the aarch64 glue, which is the one
part of this path no host test can reach — and this is precisely the claim that
must not live somewhere unreachable.

Added 2026-08-07. `TOS64-RX/1 STATE=STOPPED REASON=NOBUFFER ACCEPTED=0
REFUSED=0` (`hand-2026-08-07/07F` §7c) is the wire's own proof that a ring of
one wrapped descriptor that is never handed back holds exactly one frame for
the life of a boot. Clause 6's refusal is untouched by this clause and the
enumeration above is what proves it.

### 9. What this test explicitly does **not** establish

- **No IOMMU, and receive does not pretend otherwise.** `LE-67` is re-argued in
  the Story rather than inherited, and the argument's honest core is that a
  malicious device already had bus-master DMA for transmit; what receive newly
  admits is a *remote peer* as an input source. The containment for that is the
  address filter, the size bound, the one-descriptor ring, and the total
  classifier — not device isolation, which does not exist on this path.
- **No parser exists**, so no claim about parsing hostile formats is made or
  needed. `BND-03` is satisfied by absence: `C1` gained an input path and no
  parser, and clause 5 is the test that keeps those two facts apart.
- No throughput, latency, loss, ordering or `D20` guardrail claim — the domain
  is open debt and one frame per park beat is a bounded poll, not a data path.
- No command is acted on. Step 2 of the ordered path
  ([`hand-2026-08-06/03B`](../../session/hand-2026-08-06/03B-the-arms-are-built-the-board-booted-them-and-nobody-read-the-wire.md)
  §5) is where a received frame first causes the board to do something, and it is
  deliberately not this Story.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/gem_receive.rs`),
written Red first per the TDD mandate; then the Tier 1 board run witnessed on
the canvas.

## Implementation location

- `os/src/hal-arm64/src/gem_receive.rs` — register map, buffer-size encoder,
  ring builder, descriptor classifier, admission filter, enable/disable and
  status state machine. Pure over the `Mmio` seam, host-tested.
- `os/src/hal-arm64/src/ethernet.rs` — the aarch64 glue: the **second** pinned
  region, cache maintenance in the device-writes-to-CPU direction, the
  once-per-beat bounded poll, and the canvas row.
- `os/src/hal-arm64/src/canvas.rs` — the `TOS64-RX/1` row position.

## Reports

To be filed with the board run; the canvas photograph or transcription and the
host-side send record are the raw evidence.
