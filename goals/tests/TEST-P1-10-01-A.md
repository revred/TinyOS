# TEST-P1-10-01-A — A Record Must Arrive Unchanged, and a Gap Must Be Countable

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P1-10-01`](../stories/STORY-P1-10-01.md)
Tier: Host unit tests (`kernel::spoor_wire` — encode/decode, bounds, malformed frames) **plus** a Tier 1 hardware run whose captured frames decode to the records the board stamped, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D07`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

## What this test is for

Two properties, and they pull in opposite directions.

**Fidelity.** A spoor is 64 bits for speed, so the wire must not touch it. If a record is
formatted, re-packed or reordered anywhere on the board-side path, the design has paid the
cost of packing and thrown away the benefit. The test is byte equality against what
`Spoor::stamp` produced.

**Refusal.** The same format must reject anything that is not exactly itself. This is where
the "almost unhackable" claim earns or loses its keep: the strength is not cleverness, it is
that there is nothing to be clever *with* — no length field to steer a read, no options to
walk, no fragments to reassemble, no state to confuse. The tests below exist to prove the
absence, because an absence is exactly the thing that silently stops being true.

## Clauses

**Clause 1 — the record is untouched.** Encode stamped spoors, read them back, compare bit
patterns. Any transformation is a failure, including a benign-looking one.

**Clause 2 — one parser, two sources.** The frame opens with `spoor_journal::JOURNAL_MAGIC`
and lays records out as the journal does, so a capture and a journal file decode with the
same code. Tested by asserting against the journal's own constant, never a copy of it.

**Clause 3 — loss is exact.** Two frames with a deliberate sequence gap; the host computes
`seq + count` and the difference from the next `seq` equals the records lost. A stream that
cannot report its drops reports a partial run as a complete one.

**Clause 4 — the counter outlives the run.** A sequence beyond `u32::MAX` round-trips. A
32-bit counter wraps in under an hour of continuous streaming and turns clause 3 into a
fiction.

**Clause 5 — a frame fills an MTU and cannot exceed one.** A full frame is 1510 bytes on the
wire; asserted at both ends, so the frame is neither a trickle nor something the MAC must
fragment.

**Clause 6 — malformed input is refused over fixed-width fields.** Four cases, each one early
return: an empty frame, a foreign magic, a `count` above the format's bound, and a `count`
the frame is too short to carry. The third is the only field an attacker could inflate and it
is bounded against a compile-time constant *before* it indexes anything.

**Clause 7 — refusal writes nothing.** An encode that cannot fit leaves the output buffer
untouched. A half-written buffer that returns an error is a buffer someone will transmit.

**Clause 8 — on silicon, a captured frame decodes to what the board stamped.** The clause no
host can satisfy, and the one that makes the rest evidence rather than arithmetic.

## What this test does not cover

- **Authenticity and confidentiality.** There is none of either. Any device on the cable
  reads every record and can forge a well-formed frame. Safe only while the board does not
  receive; see [`docs/spoor-transport-architecture.md`](../../docs/spoor-transport-architecture.md) §7.
- **`LE-67`.** GEM DMA runs with no IOMMU. No format-level test says anything about a device
  writing RAM the grant never named.
- **Cost.** What a stamp and a drain cost on the board belongs to `TEST-P1-10-02-A`.
