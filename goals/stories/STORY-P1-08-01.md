# STORY-P1-08-01 — Motion-Group Data Contracts and the Deterministic Transport Double

Status: **Verified (Host), 2026-07-30** — assurance state `baseline-debt` (`D21` readiness `design`: no field-I/O subsystem exists, so no guardrail can be closed; the selection is stated open debt per `LE-35`). Host tier only, by design and by the 08A mandate.
Feature: [`FEAT-P1-08`](../features/FEAT-P1-08.md)
Introduced in: [`work/case-motion-controller/foundational-motion-synchronisation-delivery.md`](../../work/case-motion-controller/foundational-motion-synchronisation-delivery.md) §13, the first promotable increment (`MFS-01` plus `MFS-03`'s minimal conformance double)

## Description

The first real programming asset for motion synchronisation: the `motion` crate's typed
contracts and the deterministic in-memory transport double that proves them. This is
deliberately a data-and-contract Story — no EtherCAT, no interpolation, no scheduler
binding, no physical I/O, no kinematics. [`ADR 0010`](../../docs/adr/0010-the-motion-group-is-the-unit-of-control.md)
governs: the motion group is the unit of control, frames are accepted or rejected
whole, and the transport is a narrow contract the coupled control code will depend on
without ever seeing a PDO index.

The epoch rule is the delivery contract's §3: `sample N → validate N → calculate from
N → stage N+1 → apply N+1`, every record names its epoch, a command is never silently
relabelled for a later epoch, and epoch wrap is an explicit protocol event that must
never make an old frame appear current.

## Acceptance criteria

1. **Typed identity that cannot be interchanged.** `MotionGroupId`, `AxisId` (< 16),
   `FeedbackId` (< 32), `Epoch` (ordered, wrap-aware) and `MotionTime` are distinct
   types; constructors refuse out-of-range values; raw integer indices do not cross
   the public motion boundary.
2. **Fixed-capacity frames.** `FeedbackFrame<32>` (group, epoch, sample time, validity
   mask, samples with per-sample identity/role/quality) and `ActuationFrame<16>`
   (group, `based_on` epoch, `apply_epoch`, validity mask, commands with per-axis
   mode/targets/limits) are `no_std`, allocation-free and compile-time bounded.
3. **Whole-epoch validation with typed rejections.** Against a declared group profile
   (mandatory feedback mask, mandatory axis mask, group identity), validation accepts
   a complete, current, identity-consistent epoch and rejects — as distinct typed
   reasons, never a boolean — a missing mandatory bit, a non-`Valid` quality on a
   mandatory bit, a repeated epoch, an out-of-order epoch, a wrong group, and a sample
   whose feedback/axis identity disagrees with the profile. A rejected epoch changes
   nothing.
4. **The `MotionGroupTransport` contract, invariants stated as types and tests.**
   `receive_epoch` / `stage` / `commit_at`: `stage` accepts the entire frame or
   changes nothing; a `CommitToken` is single-use and tied to exactly one staged
   frame; `commit_at` cannot alter the frame or its epoch; a late commit fails closed;
   no per-axis "write now" path exists.
5. **The deterministic in-memory double proves the invariants adversarially.** A
   scripted, repeatable transport double delivers configured feedback frames and
   records staged/committed output; tests drive every rejection arm and every
   forbidden transition (partial stage, token reuse, late commit, epoch retag) and
   observe that output did **not** occur, not merely that an error was returned.
6. **Test-first, host tier.** Every criterion has a failing test observed before the
   code that makes it pass; `cargo test -p motion` is the whole evidence tier for this
   Story. Adversarial construction tests (out-of-range identities, mask/sample
   disagreement, wrap-edge epochs) are part of criterion coverage, not extras.

## Delivered, 2026-07-30

All six criteria Green on the host: **51 tests**, written first in full and observed
failing as a 119-error compile-stage Red before any implementation line existed (the
same Red convention `hal-arm64` and `xtask` recorded on `STORY-P1-07-05`), then made
Green without touching a test. `cargo fmt` and `clippy -D warnings` clean, crate on the
no-heap
shipped-crate gate (`no_std`, `#![forbid(unsafe_code)]`, no allocator). Evidence:
[`REPORT-2026-07-30-04`](../reports/REPORT-2026-07-30-04.md).

## Named debt this Story leaves open

- **`LE-62`**: everything between these contracts and moving metal — periodic
  phase-aligned release (`MFS-02` on `FEAT-P1-04`'s machinery), the full 16/32 plant
  simulator, collector/executor/atomic-commit Stories, the deterministic process
  image, EtherCAT MainDevice, NIC/DMA (`LE-26`), CiA-402, HIL and every hardware or
  timing claim. The delivery contract's claim ladder caps this Story at **Code-live**.
- `D21` open-debt row per `LE-35` (subsystem `design`); no guardrail evidence is filed.

## Tests

[`TEST-P1-08-01-A`](../tests/TEST-P1-08-01-A.md) — written before implementation, per
the TDD mandate.
