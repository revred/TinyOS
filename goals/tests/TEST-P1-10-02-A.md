# TEST-P1-10-02-A — The Board Must Actually Leave Spoors, and Say What It Lost

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P1-10-02`](../stories/STORY-P1-10-02.md)
Tier: Host unit tests (stamping vocabulary, drain arithmetic, ring-overflow accounting) **plus** a Tier 1 hardware run whose capture carries records from more than one category, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D07`, `D11`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

## What this test is for

`LE-56` says the audit atom has never been seen leaving a running system. Everything about
spoors is well-tested and none of it has ever been *observed on hardware*. This is the test
that changes that, and it has to resist two temptations.

The first is **stamping to satisfy the test**. Rungs stamp because a reader of the stream
needs to know the system passed them, not because a count needs to be non-zero.

The second is **widening the vocabulary to fit a call site**. `Category`, `Actor`, `Action`
and `Outcome` are closed enums with decode-time validation; if a rung has no honest
vocabulary entry, the entry is added to the kernel vocabularies test-first, never by
stretching an existing one until it covers something it does not mean.

## Clauses

**Clause 1 — a run produces a stream.** A captured boot carries records from more than one
category. One category is a trickle wearing a stream's name.

**Clause 2 — records match the rungs.** Decoded records correspond to the stamps the boot is
known to make, in the order the boot makes them. The boot sequence is deterministic, so this
is checkable rather than impressionistic.

**Clause 3 — `LE-56` closes on evidence.** A captured kernel spoor exists and can be pointed
at. Until then the claim stays open however many host tests pass.

**Clause 4 — the drain is fail-safe.** A refused transmit leaves the park loop running and
the refusal is itself observable. The park loop is the last thing standing between the board
and silence; a diagnostic channel that can hang it is worse than no channel.

**Clause 5 — a wrapped ring is reported.** Drive the journal past capacity between drains on
the host and assert the sequence gap equals the records lost exactly. This is the honest-
degradation clause: the design tolerates loss and must never tolerate *unreported* loss.

**Clause 6 — the cost is stated.** The per-stamp and per-drain cost is measured through
`fixture_measure`'s existing machinery, or this Story records that it was not measured and
why. **An observability substrate whose overhead is unknown is one that gets switched off in
the run that mattered.** A stated "not measured" passes this clause; an unstated assumption
does not.

**Clause 7 — no allocation, no blocking, no unbounded work.** Asserted the way the rest of
the real-time path is: the drain is bounded by the frame's record count and does no work
proportional to anything else.

## What this test does not cover

- **Completeness.** This is boot-and-park behaviour only. `FEAT-P1-10` exit criterion 4
  requires every Report to say so, and no clause here should be read as evidence that the
  stream is the system's whole observable behaviour — it is not yet.
- **Inbound.** Receive stays disabled and nothing here tests a receive path, because there
  is none to test.
- **Sizing.** The right ring capacity and drain cadence for a streaming buffer are unmeasured
  and registered as debt rather than asserted.
