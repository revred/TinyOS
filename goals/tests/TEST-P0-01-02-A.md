# TEST-P0-01-02-A — CI Governance-Gate Smoke Test

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-01-02`](../stories/STORY-P0-01-02.md)
Tier: CI (no hardware tier — validates the CI pipeline configuration itself)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D25`
Security controls: `SEC-14`, `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`, `C2`, `C3`, `C4`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`, `BND-18`
Protection Domain contracts: `PD-02`, `PD-12`, `PD-13`, `PD-14`
Code admission gates: `RCG-05`, `RCG-06`, `RCG-07`, `RCG-12`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

**Given** deliberately non-conformant code-governance fixtures plus malformed performance/security/Story/Test/Report catalogue fixtures,
**when** each fixture is run through the CI governance commands,
**then** each fails the specific check it was designed to violate, while the committed workspace passes — proving the gates from [`STORY-P0-01-02`](../stories/STORY-P0-01-02.md) catch what they claim, not just that they run.

The committed assurance check must report exactly 8 mapped Features, 25 mapped Stories, 23 Test documents, 28 Reports, 5 containment classes, 20 boundary tests, 20 security controls, 14 Protection Domain contracts, 14 code-admission gates, 25 class communication pairs, 19 application/platform targets, nine whole-system landing zones, and a valid 625-cell performance catalogue. It must reject a missing or disconnected Security Charter, missing charter rows or class pairs, invalid `PD-*`/`RCG-*` references, a charter that leaves a control/boundary uncovered, missing classes/tests, unknown class references, unowned boundary tests, unmapped Features, Story classes outside the parent Feature contract, Test metadata that differs from the mapped Story/Feature contract, an unknown application reference, an application selected by no landing zone, or a landing-zone goal/performance/security/class promise disconnected from its selected applications. Functional verification remains distinct from assurance verification.

## Test type

CI configuration/meta-test — this test verifies the governance tooling itself, distinct from testing kernel code.

## Implementation location

`os/src/xtask/src/governance.rs` (`governance-fixture-test`), `performance_catalogue.rs` (`check-performance-catalogue`), and `assurance.rs` (`check-assurance-spine`), invoked from `.github/workflows/ci.yml`.

## Reports

- [`REPORT-2026-07-26-03`](../reports/REPORT-2026-07-26-03.md) — Pass (local; not yet observed in CI)
- [`REPORT-2026-07-26-23`](../reports/REPORT-2026-07-26-23.md) — Pass (assurance-spine extension, local)
- [`REPORT-2026-07-26-25`](../reports/REPORT-2026-07-26-25.md) — Pass (five-class containment-contract extension, local)
- [`REPORT-2026-07-26-26`](../reports/REPORT-2026-07-26-26.md) — Pass (Security Charter and remote-code exclusion contract extension, local)
- [`REPORT-2026-07-26-28`](../reports/REPORT-2026-07-26-28.md) — Pass (whole-system application/landing-zone context extension, local)
