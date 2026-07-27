# TEST-P0-01-03-A — `xtask qemu-x86_64` Command Smoke Test

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-01-03`](../stories/STORY-P0-01-03.md)
Tier: CI / Tier 0
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`, `D24`
Security controls: `SEC-01`, `SEC-17`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`, `BND-18`
Protection Domain contracts: `PD-02`, `PD-12`, `PD-13`, `PD-14`
Code admission gates: `RCG-05`, `RCG-06`, `RCG-07`, `RCG-12`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

**Given** a clean checkout with only the pinned toolchain and QEMU installed,
**when** `cargo run -p xtask -- qemu-x86_64` is invoked,
**then** it builds `kernel` against the custom target, launches QEMU, and returns the correct exit code for both a passing boot (delegates to `TEST-P0-01-01-A`'s pass condition) and a deliberately broken boot fixture (returns a distinguishable failure code).

## Test type

Integration/smoke test for the `xtask` tool itself.

## Implementation location

`os/src/xtask/src/main.rs` (the `qemu-x86_64` command and its exit-code mapping), fixture path via `kernel`'s `fixture-broken-boot` Cargo feature (`os/src/kernel/src/main.rs`).

## Reports

- [`REPORT-2026-07-26-02`](../reports/REPORT-2026-07-26-02.md) — Pass (local; not yet observed in CI)
