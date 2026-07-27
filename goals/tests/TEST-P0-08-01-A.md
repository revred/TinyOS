# TEST-P0-08-01-A — `pack-txe` Produces a Page-Aligned, `.bss`-Flattened, Content-Faithful Re-Layout

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-08-01`](../stories/STORY-P0-08-01.md)
Tier: Host (`cargo test -p xtask`), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — `xtask::txe::pack` is pure host-side logic with no target-specific dependency.
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D09`, `D25`
Security controls: `SEC-02`, `SEC-06`, `SEC-09`, `SEC-10`, `SEC-17`, `SEC-19`
Containment classes: `C3`, `C4`
Boundary tests: `BND-10`, `BND-11`, `BND-12`, `BND-13`
Protection Domain contracts: `PD-03`, `PD-04`, `PD-11`, `PD-12`, `PD-14`
Code admission gates: `RCG-02`, `RCG-04`, `RCG-05`, `RCG-06`, `RCG-07`, `RCG-08`, `RCG-09`, `RCG-11`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

**Given** a synthetic PE32+ image built with deliberately non-page-aligned section raw-data offsets (mirroring a real linker's default 512-byte `FileAlignment`) and, in one case, a section whose `VirtualSize` exceeds its raw `SizeOfRawData` (the `.bss` shape),
**when** `xtask::txe::pack` re-layouts it,
**then**:
- every section's new `PointerToRawData` is a multiple of 4096,
- every section's new `SizeOfRawData` equals its `VirtualSize`,
- real file-backed bytes are preserved exactly, and the `.bss` tail (plus any page-rounding pad) is physically zero,
- two sections with non-page-aligned sizes never overlap once repacked,
- malformed input (missing DOS signature, truncated) fails closed with a typed `TxePackError`.

## Test type

Unit tests — `xtask::txe`'s own `#[cfg(test)]` module, run via `cargo test -p xtask` (`xtask` has no `[lib]` target, so its tests compile as part of the `xtask` binary's own test harness, alongside `governance`/`performance_catalogue`'s existing tests).

## Implementation location

`os/src/xtask/src/txe.rs`.

## Reports

[`REPORT-2026-07-26-19`](../reports/REPORT-2026-07-26-19.md) — Pass.
