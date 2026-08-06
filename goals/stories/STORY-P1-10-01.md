# STORY-P1-10-01 — The On-Wire Spoor Format: Raw Records, Measured Loss

Status: **Verified (functional) 2026-08-05 — all six acceptance criteria met. Host half Green 2026-08-04 (13 tests in `kernel::spoor_wire`, Red-verified against an unwritten encoder: 5 of 13 failed). Every criterion here is a property of the *format* rather than of a board run, and each is host-provable — but three of them have since been corroborated on silicon, which is why this advances rather than resting on host tests alone. **Criterion 1** (records byte-identical to what `Spoor::stamp` produced, no re-packing anywhere on the board-side path) is the one that most needed hardware, and `BOARD VERDICT 13` checked it the hard way: a window holding both a live frame 0 and the retained certificate for the same epoch compared them **record for record and reported byte-identical** — the same packed `u64`s carrying the same sequence numbers, not a re-stamp. **Criterion 5's** refusal path was exercised across 400+ records decoded off the wire (`BOARD VERDICT 10`–`14`) with **0 refused**: every `Category`/`Actor`/`Action`/`Outcome` discriminant and every rung resolved against the kernel's own tables, so the closed vocabulary agrees not merely in the parity test but on the wire. **Criterion 3's** 64-bit sequence and exact loss accounting held across a capture spanning a power cycle — two boots, sequence 244 → 0, **0 records lost**, read correctly as a reboot by the epoch rather than as a backwards jump. **Criterion 4's** 181-record bound is the constant `STORY-P1-10-03` explains: the frame is sized to the larger of the two framings, so the UDP wrapper cannot overflow it and fragmentation is unreachable rather than merely unused. Egress and its costs are [`STORY-P1-10-02`](STORY-P1-10-02.md)'s, not this Story's. **Assurance state remains `specified` and this Story is NOT release-assured**: 0 qualified platforms ([`06A`](../../session/hand-2026-08-05/06A-nothing-is-verified-and-the-reason-is-not-velocity.md) §2).**
Feature: [`FEAT-P1-10`](../features/FEAT-P1-10.md)
Architecture: [`docs/spoor-transport-architecture.md`](../../docs/spoor-transport-architecture.md) §3–§6
Introduced in: `session/hand-2026-08-04/04A` session

## Description

The format a spoor stream travels in. Field-by-field specification is §3 of the architecture
document; this Story is the decisions and the evidence.

**Records travel raw.** A spoor is 64 bits *for speed* — one packed store, cheap enough to
stamp from a real-time path. Formatting one into text on the board would spend exactly what
the packing saves, on the hot path, to spare a laptop work it has cycles for. So the board
copies bytes and the host decodes them.

**The format is not new.** `SPOORJ01` and the packed-`u64` layout are already what
`kernel::spoor_journal` declares as its on-disk shape, so one host parser reads a live
capture and a journal file identically, and a captured stream replays into any tool that
consumes journals. Inventing a second format would have been the drift this project keeps
finding.

**Loss is counted, not hidden.** The link is unreliable broadcast with no acknowledgement,
retransmission or flow control — all four are state, and state is what makes a receiver
attackable. Instead each frame carries the sequence number of its first record, so the next
expected sequence is `seq + count` and any gap is an exact count of lost records.

## Acceptance criteria

1. **Records are byte-identical to what `Spoor::stamp` produced.** No formatting, no re-packing, no reordering anywhere on the board-side path.
2. **A frame reuses `spoor_journal::JOURNAL_MAGIC` and its record layout**, so a capture and a journal file parse with one decoder.
3. **The sequence counter is 64-bit and loss is exactly countable.** A 32-bit counter wraps in under an hour on a continuously streaming system, and a wrapped counter makes drop accounting a fiction rather than a measurement.
4. **A frame fills a standard MTU and cannot exceed one.** 181 records per transmit (184 as first written; reduced when `udp_wire` made the UDP framing the larger of the two the payload must fit), so the frame is not what limits a stream that is continuous by nature — and so fragmentation is unreachable rather than merely unused.
5. **Every malformed frame is refused, never partially parsed.** A wrong magic, a `count` above the format's bound, a `count` the frame is too short to carry, and a read past the end each return an error over fixed-width fields.
6. **Nothing is written on refusal.** An encode that cannot fit its output leaves the buffer untouched rather than half-filled.

## Named debt this Story leaves open

- **No authenticity and no confidentiality.** Any device on the cable can read every record and forge a well-formed frame. Safe only because the board does not yet receive; see `FEAT-P1-10`'s exclusions and architecture §7.
- **`LE-67`** — GEM DMA with no IOMMU. A hardware exposure no format closes.
- **The `flags` word is reserved and read by nobody.** It exists so a future field need not move the records; an unused field is a small cost against a format migration.
- **Ring sizing is unmeasured.** A ring that wraps before a drain is a measured loss, not a silent one, but the capacity was chosen for a crash dump rather than a stream.

## Tests

[`TEST-P1-10-01-A`](../tests/TEST-P1-10-01-A.md) — written before implementation, per the TDD mandate.
