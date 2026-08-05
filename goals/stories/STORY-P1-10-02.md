# STORY-P1-10-02 — The Board Starts Leaving Spoors: Boot and Park Stamping, and the Drain

Status: **In progress — implemented and **proven on silicon 2026-08-04** (`BOARD VERDICT 10`). Criteria 1, 2, 3 and 5 Green: a power cycle captured inside a listening window produced `frame seq=0 count=8` carrying `MmuEnabled cost=183974`, `GicRouted`, `TickArmed cost=1`, then park and beacon rungs — 160 records, **0 refused, 0 lost**, sequence unbroken 0..160, read **unelevated** by Ti64Dink through Npcap. `MmuEnabled`'s cost agrees with the same boot's canvas (`ON=183971`) to three cycles, so the wire now carries what the screen carried. Criterion 4 (fail-safe drain) is evidenced negatively — the beacon survived with a second transmit per pass (`BOARD VERDICT 9`) — and criterion 6 (stated cost) is **measurable but not yet measured**: the machinery landed 2026-08-05 test-first (three phases in `kernel::measure_phases` — `spoor_stamp_park_rung_per_op_of_8`, `spoor_drain_full_ring_frame_of_181`, `spoor_announce_certificate_frame_of_3` — 3 host tests Red first, timed regions stopping at the RAM buffer with the GEM transmit deliberately outside them), so the next measure boot's envelope carries the three costs; until a board emits them the cost remains stated-unmeasured. Not Verified.**
Feature: [`FEAT-P1-10`](../features/FEAT-P1-10.md)
Architecture: [`docs/spoor-transport-architecture.md`](../../docs/spoor-transport-architecture.md) §2, §4
Introduced in: `session/hand-2026-08-04/04A` session

## Description

`STORY-P1-10-01` built the envelope. This Story gives it something to carry.

**The board currently mints almost no spoors.** `kernel::spoor` and `kernel::spoor_journal`
are well-tested and, on the AArch64 path, appear only inside `hal-arm64::fault`'s *test*
module. `kernel::fault::audit` stamps one per fault; nothing stamps the boot rungs, the tick,
the beacon or the park loop. That is `LE-56` exactly — the audit atom has never been seen
leaving a running system.

Two halves:

**Stamping.** The boot and park rungs each stamp a spoor as they pass: MMU enabled, GIC
routed, tick armed or refused, beacon transmitted or refused, park iteration, fault taken.
`Category::Boot` and `Category::Fault` already exist in the vocabulary; anything genuinely
missing is added to the kernel vocabularies test-first, never by widening an enum to fit a
call site.

**Draining.** The park loop drains the journal into frames and transmits them. The drain
carries the sequence counter, so a ring that wrapped between drains reports as an exact count
of lost records rather than as a gap nobody notices.

This is deliberately the *boot and park* path only. The owner's order is that the existing
`dispatch`/`lock`/`wcet`/`actuation` call sites stream afterwards, through a channel already
shown to work on hardware.

## Depends on

`STORY-P1-10-01` (the format), `STORY-P1-09-15` (the DMA path the beacon proved), and
`STORY-P1-07-10` (the park loop runs with interrupts unmasked, so a drain there is not
competing with a permanently masked core).

## Acceptance criteria

1. **A boot produces a stream, not a trickle.** Every rung named above stamps, and a captured run shows records from more than one category.
2. **Records on the wire are byte-identical to what the rung stamped.** Verified by decoding a capture and comparing against the stamps the boot is known to make.
3. **`LE-56` closes on a captured kernel spoor**, not an asserted one — the claim becomes evidence a reader can point at.
4. **The drain never blocks the park loop and never allocates.** A transmit that refuses leaves the loop running and the refusal itself is observable, per the fail-safe rule.
5. **A wrapped ring is reported, not hidden.** If the journal overwrites between drains, the host's sequence gap equals the number of records lost. Tested on the host with a ring driven past capacity.
6. **The stamping costs are stated, not assumed.** The per-spoor cost is measured on the board through `fixture_measure`'s existing machinery, or the Story records that it was not measured and why. An observability substrate whose overhead is unknown is one that will be turned off.

## Named debt this Story leaves open

- **Completeness.** This is boot-and-park behaviour only. `FEAT-P1-10` exit criterion 4 requires every Report to say so until the kernel call sites are wired.
- **Drain cadence and ring size are unmeasured** — see `FEAT-P1-10`'s named debt.
- **No inbound.** Receive stays disabled; the containment argument `LE-67` records is unchanged by this Story.

## Tests

[`TEST-P1-10-02-A`](../tests/TEST-P1-10-02-A.md) — written before implementation, per the TDD mandate.
