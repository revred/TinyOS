# STORY-P1-08-02 — Typed Feedback Ownership: Axis, End-Effector, and Group/Process

Status: **Verified (Host), 2026-07-30** — assurance state `baseline-debt` (`D21` readiness `design`; stated open debt per `LE-35`, same shape as `-01`). Host tier only, by design.
Feature: [`FEAT-P1-08`](../features/FEAT-P1-08.md)
Introduced in: [`work/Derisk10usLatencyRequirement.md`](../../work/Derisk10usLatencyRequirement.md) §7/WP1 and risk `R4`, ordered by the owner; decision context in [`ADR 0011`](../../docs/adr/0011-lat-phys-10-governs-and-the-two-event-100mbit-path-is-rejected.md)

## Description

`STORY-P1-08-01` bound every feedback sample to an axis. The de-risking contract
names that a **contract gap** (§7) and a kill-rule risk (`R4`): a metrology probe, a
workpiece-frame sensor and a group thermal channel are not "auxiliary axis" feedback,
and casting them into an unrelated axis would make the Hexapod implementation look
complete while excluding exactly the signal that closes the surface-following loop.

This Story replaces axis-only identity with **typed ownership** — one atomic epoch
carries, without semantic aliases:

1. axis-owned motor/load feedback;
2. end-effector-owned probe/force/deflection feedback;
3. group/process-owned metrology and environment feedback.

Whole-epoch validation, mandatory masks, and `ADR 0010`'s group-first law are
unchanged; what changes is *who a channel may report for*, checked per channel
against the profile's declared owner.

## Acceptance criteria

1. **Typed effector identity.** `EffectorId` is a distinct bounded type
   (< `MAX_EFFECTORS`); constructors refuse out-of-range values; raw indices do not
   cross the motion boundary.
2. **Ownership is a closed sum, not a role tag.** `FeedbackOwner` is
   `Axis { axis, role }` | `EndEffector { effector, role }` | `Group { role }`, with
   role vocabularies per owner (motor/load/velocity; probe position/deflection/
   contact force; metrology/environment/process). The old axis-only
   `FeedbackRole::Auxiliary` dumping ground does not survive.
3. **The profile declares an owner per mandatory channel and validation enforces
   it.** A sample whose owner disagrees with the declared binding — including a
   probe channel cast into *any* axis owner — is a whole-epoch `IdentityMismatch`
   rejection. That is `R4`'s kill rule as a positive control, not prose.
4. **The full Hexapod sensor set is representable and validated in one epoch.**
   Three drive axes with motor- and load-side channels each, one end-effector probe
   deflection channel, and one group metrology channel share one accepted epoch; a
   missing mandatory probe bit rejects the whole epoch exactly as a missing axis
   channel does — the probe is coupled feedback, never an optional extra.
5. **Test-first, no weakening.** The migration rewrites the affected tests to the
   new contract first (observed compile-stage Red), adds ownership and Hexapod-shape
   tests, and every `-01` behaviour that still applies stays covered; host tier is
   the whole evidence tier.

## Delivered, 2026-07-30

All five criteria Green on the host: **61 tests** (the 51-test `-01` suite migrated
without weakening, plus ownership-sum, effector-bound, `R4`-cast and Hexapod-epoch
additions), written/rewritten first and observed failing as a 43-error compile-stage
Red, then made Green; `cargo fmt` and
`clippy -D warnings` clean; the crate stays `no_std`, `#![forbid(unsafe_code)]`,
allocation-free, on the no-heap gate. Evidence:
[`REPORT-2026-07-30-05`](../reports/REPORT-2026-07-30-05.md).

## Named debt this Story leaves open

- **WP1's second half**: calibrated physical quantities and frame transforms above
  raw counts (units, scaling, calibration state) — deliberately not invented here;
  it needs the probe calibration model (`R8`) and belongs with the Hexapod solver
  work (`LE-63`).
- `LE-62` (transport/hardware chain) and `LE-63` (architecture-gate artifacts)
  unchanged; the claim ladder still caps this Feature at **Code-live**.
- `D21` open-debt row per `LE-35`; no guardrail evidence filed.

## Tests

[`TEST-P1-08-02-A`](../tests/TEST-P1-08-02-A.md) — written before implementation,
per the TDD mandate.
