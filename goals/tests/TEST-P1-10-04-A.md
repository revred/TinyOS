# TEST-P1-10-04-A — A Listener Joining Late Must Learn What It Missed

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P1-10-04`](../stories/STORY-P1-10-04.md)
Tier: Host unit tests (`kernel::spoor_wire`, `kernel::spoor_stream`) **plus** a Tier 1 hardware run whose capture is taken *without* a power cycle inside it, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D07`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

## What this test is for

The board-side half of `STORY-P1-10-02` was proven by a capture that *happened* to be
listening across a power cycle. This test exists so that the next such capture does not have
to be lucky: it drives the case where the listener joined afterwards, which is the case every
deployed listener will actually be in.

The hardware clause is deliberately the **opposite** of `TEST-P1-10-02-A`'s. That one needed
a power cycle inside the capture window. This one needs a capture started against a board
that has been running for some time, because the whole claim is about what such a capture can
still learn.

## Clauses

**Clause 1 — every frame self-identifies.** Encode and decode carry a 32-bit epoch, and a
frame built by a seeded stream never carries `0`. `0` is reserved for *not declared*, so a
host reading an unseeded or older image sees an honest absence rather than a plausible value.

**Clause 2 — one boot, one epoch.** Every frame a stream emits — drained or retained —
carries the same epoch value for the life of that stream, and re-seeding changes it. A field
that varied within a boot would make an epoch change unusable as a reboot signal.

**Clause 3 — the certificate survives the ring.** Drain the stream, overrun the ring past
capacity, drain again, repeatedly. The retained frame still carries the boot rungs. This is
the clause the whole Story exists for: the ring is a jitter buffer and the certificate must
not live in it.

**Clause 4 — the re-announcement is verbatim.** Capture the first drained frame's records,
then take a retained frame after arbitrary further traffic, and assert the record bytes and
their sequence numbers are identical. Not a re-stamp — a re-stamp would carry fresh sequence
numbers and a fresh cost field, and would be a *different event* wearing the same name.

**Clause 5 — a retained frame is marked, and marking it is what prevents a lie.** The
`RETAINED` flag is set on an announced frame and clear on a drained one. Applying the host's
own `seq + count` arithmetic across a drained frame, a retained frame, and the next drained
frame produces **no gap and no backwards jump** — verified by running exactly that
arithmetic in the test rather than by asserting the flag alone.

**Clause 6 — the certificate is bounded and write-once.** Stamp a park rung many times more
than the buffer holds; the buffer's contents are unchanged and its length has not grown. A
birth certificate that the ten-thousandth park iteration can overwrite is not one.

**Clause 7 — the run is consecutive from zero.** The certificate holds a run of records
beginning at sequence 0 with no holes, so the single `seq` in its header describes every
record it carries. Verified by stamping a non-boot rung early and asserting the certificate
closed at that point rather than skipping it.

**Clause 8 — announcement is periodic and bounded.** The announce entry point emits at most
one frame per N calls, and the bound N is a stated constant a host test can read, not a
number scattered through the park loop. A listener therefore has a *stated* worst-case window
before it learns the boot state.

**Clause 9 — the board's cost is a copy, not a computation.** Announcement copies a fixed
buffer into a frame. No formatting, no allocation, no work proportional to uptime, and no
second pinned DMA region — it rides the transmit path `STORY-P1-10-02` already proved.

**Clause 10 — Tier 1: a capture with no power cycle in it.** Start a capture against a board
that has already been running, and read the boot rungs out of it. The run must also show the
epoch identical across every frame in the window. Until that capture exists, criteria 1–5 are
host-Green only and the Story says so.

## What this test does not cover

- **Boot counting.** The epoch distinguishes boots; it does not number them. No clause here
  should be read as evidence a host can say how many boots it missed — it cannot, and `LE-74`
  records why.
- **Inbound.** Receive stays disabled. Nothing here tests, needs, or moves toward a receive
  path; that is architecture §7 and a different risk class.
- **Loss of the announcement itself.** The announcement is broadcast on the same unreliable
  link as everything else. It can be lost; the next one comes. The claim is a bounded expected
  window, not a guarantee, and stating it that way is the point.
- **Completeness.** Boot-and-park behaviour only, unchanged by this Story.
