# STORY-P1-10-04 — The Retained Boot Certificate, and the Epoch a Late Listener Can Read

Status: **In progress — written 2026-08-04 from the owner's closing question in [`session/hand-2026-08-04/05A`](../../session/hand-2026-08-04/05A-the-board-speaks-in-spoors-and-what-a-late-listener-cannot-know.md) §5, implemented test-first (14 tests Red-verified against an unwritten implementation), and **proven on silicon 2026-08-05** across `BOARD VERDICT 11`, `12` and `13`. Criteria 1–5 and 7 Green: a capture opened at record 74 with frame 0 long gone still read `MmuEnabled cost=184052`, `GicRouted`, `TickArmed` out of a retained certificate; the epoch changed `0x049F8B28` → `0x04B328BC` → `0x04B32825` across power cycles; one window held two boots with **0 records lost** across the restart; and a window holding both a live frame 0 and a certificate for one epoch compared them record for record and reported **byte-identical**. Announcement cadence measured at one frame per ~5 s on two separate boots, matching `ANNOUNCE_EVERY`. Criterion 6 (bounded, write-once) is host-Green only — no board run has yet stamped enough certificate rungs to reach the buffer's ceiling. **`LE-74` was amended by these runs rather than merely cited**: two consecutive boots produced epochs 151 counter ticks apart, so the entropy is measurably thinner than the design-time caution assumed. Not Verified.**
Feature: [`FEAT-P1-10`](../features/FEAT-P1-10.md)
Architecture: [`docs/spoor-transport-architecture.md`](../../docs/spoor-transport-architecture.md) §3, §4, §4.1
Introduced in: `session/hand-2026-08-04/05A` mandate

## Description

`STORY-P1-10-02` put a stream on the wire. This Story closes the hole the owner found in it,
which is not a polish item: **the stream cannot be joined.**

Two facts about the substrate as built:

**If frame 0 is lost, the boot rungs are gone forever.** `MmuEnabled`, `GicRouted`,
`TickArmed` stamp exactly once, the drain clears the ring, and nothing re-sends them. A host
that missed that frame sees a sequence gap and learns *how many* records it lost but never
*what they were* — and boot state is the least repeatable and most diagnostic part of the
whole stream. `BOARD VERDICT 10` exists only because a capture happened to be listening
across a power cycle. That is evidence by luck, and luck is not a channel property.

**A listener joining late cannot tell which boot it joined.** At `seq=25138` there is nothing
distinguishing "continuing normally" from "joined after a reboot I never saw". A sequence
number is a position within a boot and cannot express which boot it is a position in; a
listener that assumes continuity across an unseen reboot will read a fresh stream as a
continuation and compute a gap of tens of thousands of records that were never lost.

Three answers exist and §5 of the handover orders them. **This Story is answers 1 and 2.**
Answer 3 — two-way query/response — requires enabling GEM receive, which is the one thing
`gem.rs` enforces against with a dedicated test and the thing `LE-67` records as *the*
containment story while there is no IOMMU. It is deliberately not taken here, and the
sequencing is the point: **a retained, re-announced, epoch-tagged stream removes most of the
reason to ask**, and that is worth establishing before spending charter work on making the
board listen.

### The retained boot certificate

The boot prologue is held in a small fixed buffer **separate from the journal ring** and is
re-emitted every few park passes. The ring keeps overwriting; the certificate never changes
after it is written.

Two properties make it honest rather than merely helpful:

- **It is a verbatim re-send, not a summary.** The records are the same packed `u64`s
  carrying the same sequence numbers the original frame carried. A host that missed frame 0
  and a host that saw it decode identical bytes.
- **It is a consecutive run beginning at `seq = 0`.** The buffer takes boot-epoch rungs while
  the run from zero is unbroken and closes permanently at the first record that is not one.
  A frame header carries one sequence and implies its records are consecutive, so a
  certificate assembled out of scattered records would make the header lie.

### The epoch field

`spoor_wire` reserved a `flags` `u16` and four bytes of padding at offsets 18–24 with the
comment *"so a future field does not have to move the records"*. This is that field, and it
costs nothing: the four padding bytes become a 32-bit epoch and the `flags` word keeps its
reserved role, minus one bit that marks a retained frame.

Every frame therefore self-identifies. A host can tell boot #7 from boot #8, knows when it
has joined mid-stream, and knows that a change of epoch is a reboot rather than a loss of
`seq - expected` records.

**What the epoch honestly is.** It is a per-boot *distinguisher*, not a unique identifier and
not a boot count. The board has no persistent store and no RTC; the epoch is derived from the
generic counter at kernel entry, which differs between boots because firmware timing does.
A host can therefore conclude "this is a different boot" with high probability, and can never
conclude "this is boot number N" or "I missed exactly two boots". The Story records that
limit rather than letting a field named `epoch` imply more than it carries — see `LE-74`.

**And the board measured that limit sharper than the design anticipated.** Two consecutive
power cycles (`BOARD VERDICT 12` and `13`) produced `0x04B328BC` and `0x04B32825` — **151
counter ticks apart, 2.8 µs at 54 MHz.** Only the low byte moved. The variation is not
uniform (an earlier pair differ by ~23 ms), but a pair that close makes collision a plausible
event rather than a theoretical one, and a collision reads as "same boot" to every host.
Every criterion below still passed on those runs — the epochs did differ and were read as
different — so this bounds the field's future use rather than retracting a result. `LE-74`
carries the measurement.

## Depends on

`STORY-P1-10-01` (the format the field sits in) and `STORY-P1-10-02` (the stamping this
retains and the drain it rides beside).

## Acceptance criteria

1. **Every frame carries a boot epoch.** Every frame from one boot carries the same value; a fresh boot carries a different one with high probability. `0` is reserved to mean *not declared* and a seeded stream never emits it, so an old image is distinguishable from a silent one.
2. **The boot prologue survives every drain.** After the ring has been drained and refilled arbitrarily many times, the certificate is still emittable and still holds the rungs the boot stamped.
3. **The re-announcement is verbatim.** Its records are byte-identical to those the original drain sent, carrying their original sequence numbers — not a re-stamp, not a summary, not a re-numbering.
4. **A retained frame is marked and is not stream continuation.** It sets the `RETAINED` flag, and a host applying `seq + count` accounting to it must not produce a false gap or a false backwards jump. The flag is on the frame because a host cannot infer it from a sequence that legitimately repeats.
5. **Announcement is bounded and egress-only.** One small frame every N park passes, no receive path, no new pinned buffer, no charter change and no new `LE-67` exposure. A listener joining at any time learns the boot state within a bounded window, and that window is stated.
6. **Nothing is retained without bound.** The certificate is a fixed-size buffer that fills once and never grows, and a rung that repeats every park pass can never displace what a boot established.
7. **The host reads it.** Ti64Dink reports an epoch change as a reboot rather than as loss, decodes a retained frame without double-counting its records, and states the boot state it learned from a certificate when it never saw frame 0.

## Named debt this Story leaves open

- **The epoch's entropy is the firmware's, not ours** (`LE-74`). Two boots whose firmware took the same number of counter ticks to reach the kernel produce the same epoch. The right fix is a hardware entropy source or a persisted counter; both are their own bring-up.
- **A listener still cannot ask.** Answer 3 remains unbuilt, and any window between joining and the next announcement is a window with no boot state in it. Bounded, but not zero.
- **Completeness is unchanged.** This is still boot-and-park behaviour only, per `FEAT-P1-10` exit criterion 4.
- **The per-announce cost is unmeasured**, exactly as `STORY-P1-10-02` criterion 6 records for the per-stamp and per-drain cost.

## Tests

[`TEST-P1-10-04-A`](../tests/TEST-P1-10-04-A.md) — written before implementation, per the TDD mandate.
