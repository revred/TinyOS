# TEST-P1-08-02-A — A Probe Is Not an Axis: Ownership Typed, Aliases Refused

Status: **Verified (Host), 2026-07-30** — every clause Green (57 host tests after the migration, rewritten/written first and observed failing as a compile-stage Red). Specification unchanged since it was written before implementation.
Story: [`STORY-P1-08-02`](../stories/STORY-P1-08-02.md)
Tier: Host unit tests (`cargo test -p motion`) — no Tier 0 fixture, no board, no bus
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D21`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`, `C3`
Boundary tests: `BND-03`, `BND-14`, `BND-15`, `BND-17`
Protection Domain contracts: `PD-05`, `PD-07`, `PD-08`, `PD-12`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

Applicable guardrails: none closeable — `D21` readiness is `design`; the selection is
stated open debt per `LE-35`. This Test raises the crate's claim no higher than
**Code-live**.

## What this test is for

Risk `R4` of the de-risking contract: an axis-only feedback schema forces probe and
process signals into an "auxiliary axis" alias the controller does not couple on, so
the Hexapod build would look complete while silently excluding the signal that closes
the surface-following loop. The kill rule is *no casting probe data into an unrelated
axis* — and a kill rule that exists only in prose is not a detector. This Test makes
it a positive control: the cast is attempted, and the whole epoch must be rejected.

## Specification

### 1. Effector identity is typed and bounded

**Given** the public motion types,
**then** `EffectorId` is distinct from every other identity, refuses indices at or
above its compile-time bound, and no raw integer crosses the boundary unchecked.

### 2. Ownership is a closed sum with per-owner role vocabularies

**Given** a feedback sample,
**then** its identity is exactly one of axis-owned (motor position / load position /
velocity), end-effector-owned (probe position / probe deflection / contact force), or
group/process-owned (metrology / environment / process) — and no axis-only
`Auxiliary` role survives anywhere in the public contract.

### 3. The profile binds an owner per channel and validation enforces it (`BND-03`, `PD-12`)

**Given** a profile declaring each mandatory channel's owner,
**then** a complete epoch whose every mandatory sample carries the declared owner is
accepted, and a sample whose owner disagrees in *any* respect — wrong axis, wrong
effector, wrong role, or wrong ownership kind entirely — is a whole-epoch
`IdentityMismatch` rejection naming the channel.

### 4. The `R4` kill rule is a positive control

**Given** a profile whose probe channel is declared end-effector-owned,
**when** a frame presents that channel as axis-owned feedback (the cast the risk
register forbids),
**then** the whole epoch is rejected; and the equivalent cast of a group/process
channel into an axis is rejected the same way.

### 5. The Hexapod sensor set shares one epoch, and the probe is load-bearing

**Given** the worked case's shape — three drive axes with motor- and load-side
channels each, one end-effector probe-deflection channel, one group metrology
channel, all mandatory,
**then** a complete epoch is accepted whole; and a frame missing the probe bit — or
carrying it with non-valid quality — is rejected whole, exactly as a missing axis
channel would be. The probe participates as coupled feedback, never as an optional
extra whose absence still validates.

### 6. No weakening, no allocation (`SEC-19`, `SEC-20`, `BND-15`)

**Given** the migration,
**then** every `-01` behaviour that still applies remains covered (epoch discipline,
mandatory masks, staging atomicity, commit semantics, the double's determinism), the
rewritten suite was observed failing before the implementation changed, and the crate
remains `no_std`, `#![forbid(unsafe_code)]` and allocation-free on the no-heap gate.

### 7. What this test explicitly does **not** establish

- **No calibrated quantities or frame transforms** — WP1's second half, deliberately
  deferred with `R8`'s calibration model (`LE-63`).
- **No controller, no kinematics, no timing figure, no transport** — `LE-62`,
  `LE-63`, and `ADR 0011`'s claim discipline all unchanged.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/motion/src/`), rewritten/written Red first.

## Implementation location

- `os/src/motion/` — `ident` (effector identity), `feedback` (ownership sum),
  `profile` (owner bindings), `validate` (owner enforcement), `double` (migrated).

## Reports

[`REPORT-2026-07-30-05`](../reports/REPORT-2026-07-30-05.md).
