# Handover 10A — Owner-Ordered: The Motion Foundation's First Increment (`FEAT-P1-08`, Code-Live)

The owner ordered
[`work/case-motion-controller/foundational-motion-synchronisation-delivery.md`](../../work/case-motion-controller/foundational-motion-synchronisation-delivery.md)
executed. That delivery contract's §13 names the first promotable increment — `MFS-01`
plus `MFS-03`'s minimal conformance double — and §11 names the checklist that must land
with it. This session delivered exactly that, and nothing beyond it.

**Standing-mandate note.** [`08A`](08A-hardware-evidence-sprint-mandate.md)'s strict
rule ("no new design surface unless it directly helps produce the next raw hardware
capture") is binding for two sprints — *unless the owner reorders*. This work is the
owner reordering: an explicit order naming the delivery document. The Pi 5 sprint is
untouched and remains the next headline; `MFS-02`…`MFS-11` queue behind it, and this
handover creates no board work.

## What exists now that did not this morning

1. **[`ADR 0010`](../../docs/adr/0010-the-motion-group-is-the-unit-of-control.md)** —
   the motion group is the unit of control; EtherCAT is a transport implementation.
   Per-axis control APIs at the public motion boundary are review-rejected, not
   discouraged.
2. **[`FEAT-P1-08`](../../goals/features/FEAT-P1-08.md)** under `EPIC-P1` (the
   backlog's standing note routes `motion` to whichever Epic is current), with its
   containment contract row: implementation C1, subject C2/C3, `BND-03/-14/-15/-17`,
   `PD-05/-07/-08/-12`, `RCG-01/-13/-14`. Feedback process images are declared hostile
   input from a compromisable transport.
3. **[`STORY-P1-08-01`](../../goals/stories/STORY-P1-08-01.md) — Verified (Host)** with
   [`TEST-P1-08-01-A`](../../goals/tests/TEST-P1-08-01-A.md) written first and
   [`REPORT-2026-07-30-04`](../../goals/reports/REPORT-2026-07-30-04.md) as its dated
   record. Contract row `D21`/`SEC-19,SEC-20`/`C1,C2,C3`, state `baseline-debt`; the
   `D21` selection is stated open debt (`LE-35` rule — no field-I/O subsystem exists).
4. **The `motion` crate** (`os/src/motion/`, workspace member, on `xtask`'s
   `SHIPPED_CRATES` no-heap gate): typed `MotionGroupId`/`AxisId`(<16)/
   `FeedbackId`(<32)/wrap-aware `Epoch`/`MotionTime`; `FeedbackFrame<32>` and
   `ActuationFrame<16>` with whole-group masks; `GroupProfile` (mandatory masks +
   per-channel identity bindings); whole-epoch `validate_feedback` and whole-frame
   `validate_actuation` with one typed rejection per disposition arm;
   `MotionGroupTransport` (stage-all-or-nothing, single-use **move-consumed**
   `CommitToken`, late-commit-fails-closed); and the deterministic scripted
   `InMemoryTransport` double. `no_std`, `#![forbid(unsafe_code)]`,
   `#![deny(missing_docs)]`, zero dependencies, fixed capacity everywhere.
5. **TDD held in the recorded convention**: the full 51-test suite was written first
   and observed failing as a 119-error compile-stage Red, then went Green with no test
   edited. Every rejection arm and forbidden transition has a positive control, and
   the stage/commit arms assert against the double's observable staged/committed
   record, not the returned error.
6. **`LE-62`** registers everything between these contracts and moving metal (periodic
   release, full simulator, collector/executor/commit-on-a-timeline, process image,
   EtherCAT MainDevice, NIC/DMA — compounded by `LE-26` — CiA-402, HIL). The delivery
   contract's claim ladder caps this delivery at **Code-live**, and its status header
   now says so.

## Gates, all green this session

`cargo test -p motion` (51) · `cargo test -p xtask` (241) · `cargo fmt -p motion
--check` · `cargo clippy -p motion --all-targets -- -D warnings` ·
`check-spine-files` · `check-assurance-spine` (28 Features / 69 Stories / 53 Tests /
58 Reports, 62 loose ends 36 open, 52 dashboard badges agree) · `check-crate-sizes` ·
`check-image-size` (86,176 bytes; `motion` links into no image yet — the crate has no
consumer until `MFS-02`, exactly as the delivery contract's §11 orders).

## Diligence corrections landed with it (delivery contract §11)

- [`docs/physical-ai-reference-workloads.md`](../../docs/physical-ai-reference-workloads.md)
  no longer reads as if `PositionFeedback` and the simulated Tier 0 feedback source
  exist — both are marked design-not-yet-implemented, pointing at `FEAT-P1-08` for
  what does exist, and at `ADR 0010` for why the trait sits *below* the group-frame
  boundary.
- The console's RT tab caveat (`work/UX-V1/console-bodies.js` and the byte-identical
  `external/tauri/tinyos-poc/stage-e-console-app/ui/console-bodies.js` copy — `cmp`
  re-verified) now says "mock UI data … design, not yet implemented" instead of
  implying a live Tier 0 `PositionFeedback`.
- `APP-04` and `LZ-01` prose extended to name coherent feedback epochs, atomic group
  commits and the EtherCAT motion fabric; dashboard, traceability matrix and
  `EPIC-P1` updated everywhere the `LE-44`/`LE-30` machinery checks, together.

## What is deliberately not here

No timing figure of any kind (cycle period, skew, age, WCET, margin, latency,
jitter) — `ADR 0005` unchanged, zero qualified platforms. No EtherCAT, DC, working
counter, PDO, NIC, DMA, CiA-402, process image, scheduler binding, kinematics or
G-code. No Tier 0 fixture — this Story's evidence tier is host tests by scope; the
first fixture arrives with `MFS-02`'s periodic-release binding. No new UI surface.

## For the next session

The next headline is still 08A's: **first silicon on the Pi 5** (start at
[`09A`](09A-story-p1-07-05-host-run-path.md)'s board checklist). When motion work
resumes after the sprint, the next increment is `MFS-02` (periodic phase-aligned
release on `FEAT-P1-04`'s machinery) promoted as `STORY-P1-08-02` — contract row and
Test document first, and the crate gets its first real consumer.

## Concurrency note

Hooks installed (`core.hooksPath .githooks`), staged narrowly; the soak logger owns
`goals/reports/_soak-p0-03-01.log` and it was left unstaged. No other session's edits
were observed mid-turn.
