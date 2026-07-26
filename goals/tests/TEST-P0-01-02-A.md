# TEST-P0-01-02-A — CI Governance-Gate Smoke Test

Status: **Planned (not yet implemented)**
Story: [`STORY-P0-01-02`](../stories/STORY-P0-01-02.md)
Tier: CI (no hardware tier — validates the CI pipeline configuration itself)

## Specification

**Given** a deliberately non-conformant fixture (unformatted code, a `clippy` violation, a crate artificially padded past the 20,000-line ceiling, and a public item missing documentation — four separate fixture cases),
**when** each fixture is run through the CI pipeline,
**then** each fails the specific check it was designed to violate, and passes all others — proving the four governance gates from [`STORY-P0-01-02`](../stories/STORY-P0-01-02.md) actually catch what they claim to catch, not just that they run.

## Test type

CI configuration/meta-test — this test verifies the governance tooling itself, distinct from testing kernel code.

## Implementation location (once written)

`os/src/xtask/` (or a dedicated CI fixture directory) — to be finalized when `FEAT-P0-01` work begins.

## Reports

None yet.
