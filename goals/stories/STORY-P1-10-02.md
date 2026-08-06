# STORY-P1-10-02 — The Board Starts Leaving Spoors: Boot and Park Stamping, and the Drain

Status: **Verified (functional) 2026-08-05 — all six acceptance criteria met. Implemented and **proven on silicon 2026-08-04** (`BOARD VERDICT 10`). Criteria 1, 2, 3 and 5 Green: a power cycle captured inside a listening window produced `frame seq=0 count=8` carrying `MmuEnabled cost=183974`, `GicRouted`, `TickArmed cost=1`, then park and beacon rungs — 160 records, **0 refused, 0 lost**, sequence unbroken 0..160, read **unelevated** by Ti64Dink through Npcap. `MmuEnabled`'s cost agrees with the same boot's canvas (`ON=183971`) to three cycles, so the wire now carries what the screen carried. Criterion 4 (fail-safe drain) is evidenced negatively — the beacon survived with a second transmit per pass (`BOARD VERDICT 9`) — and criterion 6 (stated cost) is **measurable but not yet measured**: the machinery landed 2026-08-05 test-first (three phases in `kernel::measure_phases` — `spoor_stamp_park_rung_per_op_of_8`, `spoor_drain_full_ring_frame_of_181`, `spoor_announce_certificate_frame_of_3` — 3 host tests Red first, timed regions stopping at the RAM buffer with the GEM transmit deliberately outside them), so the next measure boot's envelope carries the three costs; until a board emits them the cost remains stated-unmeasured. **Criterion 6 is now MEASURED on silicon 2026-08-05** — the envelope harvested off the wire carries **11 metrics, not 8**, and the three phases report `n=1000 dropped=0`: `spoor_stamp_park_rung_per_op_of_8` **p50=136** (min 132, max 143), `spoor_announce_certificate_frame_of_3` **p50=3099** (min 3088, max 3135), and `spoor_drain_full_ring_frame_of_181` **p50=122005** (min 121614, max 122223). Raw evidence: [`goals/reports/wire-meas-envelope-2026-08-05.txt`](../reports/wire-meas-envelope-2026-08-05.txt). Two things must be said with those numbers rather than after them. **The timed region stops at the RAM buffer and the GEM transmit is deliberately outside it**, so the drain figure is the cost of *filling* a full 181-record frame and not the cost of putting it on the cable — a full-ring drain is ~674 cycles per record, and no line of this may be quoted as a transmit cost. **And the stamp figure is the only one of the three on a hot path**: 136 cycles per park-rung stamp is what the park loop actually pays every beat, while the drain and announce costs are amortised across 181 and 3 records respectively. **Criterion 4 is met, and the earlier "evidenced only negatively" reading conflated two questions.** The criterion asks that the drain never block the park loop and never allocate, that a refused transmit leave the loop running, and that the refusal itself be observable. *Never allocates* is not merely tested but **compiler-enforced**: every crate in the image is `no_std` with no `#[global_allocator]`, and `check-assurance-spine` fails the build if that changes — stronger evidence than any run could give, and the reason the gate exists is so that adding an allocator withdraws the claim loudly. *A refused transmit leaves the loop running and says so* is host-driven by `TEST-P1-10-02-A` clause 4, and was then observed on hardware through the same GEM transmit path this drain uses: `BOARD VERDICT 3`'s beat line read `STATE=STOPPED REASON=TIMEOUT` while `SEQ=19,20,21,22…` kept climbing — a refused transmit, a park loop that did not stop, and a refusal a human could read. **What has never happened is a refused *drain* specifically**, as opposed to a refused beacon on the shared path, and that is stated rather than papered over; the Story's Tier declares the Tier 1 obligation against clause 1 (a capture carrying more than one category), which `BOARD VERDICT 10` discharged with 160 records across four categories. Discharge through an amended Report is **not** a third state a Story may sit in — [`06A`](../../session/hand-2026-08-05/06A-nothing-is-verified-and-the-reason-is-not-velocity.md) §4.1 rules that out explicitly, and the Report amendment is owed separately. **Assurance state remains `specified` and this Story is NOT release-assured**: the three costs above are measurements, not release-gate runs in a declared deployment profile, and `qualified-platforms.tsv` holds 0 qualified platforms (`06A` §2). **2026-08-06 — criterion 6's numbers were being read against the wrong domain's targets, and against `D11`'s they fail.** The three spoor metrics carried a `D07` label because this Story's contract selected only `D07`; `D07` is fixed-capacity pool allocation and `D11` is *"spoor stamp and journal"*, which is what they measure and nothing else. So from 2026-08-05 to 2026-08-06 the substrate's own cost sat on the wire, quoted in three documents, and **was never once compared to the gate whose subject it is**. The contract now selects `D07,D11` (`D11` is `prototype` readiness, so no debt row), the fixture emits `D11`, and all three of `D11`'s latency gates are filed from `spoor_stamp_park_rung_per_op_of_8` on the 2026-08-06 boot at `cycles_per_us=2400`: **`PERF-D11-G01` p50 = 0.0571 µs against ≤ 0.03 µs — a fail by 1.90×**, filed as one; `PERF-D11-G02` p99 = 0.0596 µs against ≤ 0.06 µs — under by **0.7%**, which is a quarter of the ~3% build-to-build movement `BOARD VERDICT 9` measured, so it is filed `refused` rather than passed, as `PERF-D03-G20` was; `PERF-D11-G03` p99.9 = 0.0600 µs against ≤ 0.1 µs — met with 40% of room and filed as met. Read together (min 131, p50 137, p99 143, p99.9 144, n=1000 dropped=0) the distribution is tight, so **the substrate's problem is its median and not its tail** — a consistently expensive hot path rather than a rare one. Criterion 6 remains met: it asks that the cost be *stated*, and it now is, against the right targets and with two of three verdicts against us.**
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
- **`PERF-D11-G01` is a recorded fail, and nothing here closes it.** 137 cycles per stamp against a 72-cycle budget (0.03 µs at 2400 cycles/µs) is the substrate's median cost on the hot path the park loop pays every beat. Making it cheaper is not this Story's work and no row claims otherwise; what this Story owes was the honest number, which is now filed.
- **`PERF-D11-G02` cannot be decided by this bench as it stands** — 0.7% of margin against ~3% observed build-to-build movement. It needs a repeated-boot campaign giving a run-to-run p99 CV for this metric, not another single capture.

## Tests

[`TEST-P1-10-02-A`](../tests/TEST-P1-10-02-A.md) — written before implementation, per the TDD mandate.
