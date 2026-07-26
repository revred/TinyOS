# TEST-P0-01-02-A — CI Governance-Gate Smoke Test

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-01-02`](../stories/STORY-P0-01-02.md)
Tier: CI (no hardware tier — validates the CI pipeline configuration itself)

## Specification

**Given** a deliberately non-conformant fixture (unformatted code, a `clippy` violation, a crate artificially padded past the 20,000-line ceiling, and a public item missing documentation — four separate fixture cases),
**when** each fixture is run through the CI pipeline,
**then** each fails the specific check it was designed to violate, and passes all others — proving the four governance gates from [`STORY-P0-01-02`](../stories/STORY-P0-01-02.md) actually catch what they claim to catch, not just that they run.

## Test type

CI configuration/meta-test — this test verifies the governance tooling itself, distinct from testing kernel code.

## Implementation location

`os/src/xtask/src/governance.rs` (`governance-fixture-test` command), invoked from `.github/workflows/ci.yml`'s `governance-fixture-smoke-test` job.

## Reports

- [`REPORT-2026-07-26-03`](../reports/REPORT-2026-07-26-03.md) — Pass (local; not yet observed in CI)
