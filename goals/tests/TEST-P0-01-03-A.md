# TEST-P0-01-03-A — `xtask qemu-x86_64` Command Smoke Test

Status: **Planned (not yet implemented)**
Story: [`STORY-P0-01-03`](../stories/STORY-P0-01-03.md)
Tier: CI / Tier 0

## Specification

**Given** a clean checkout with only the pinned toolchain and QEMU installed,
**when** `cargo run -p xtask -- qemu-x86_64` is invoked,
**then** it builds `kernel` against the custom target, launches QEMU, and returns the correct exit code for both a passing boot (delegates to `TEST-P0-01-01-A`'s pass condition) and a deliberately broken boot fixture (returns a distinguishable failure code).

## Test type

Integration/smoke test for the `xtask` tool itself.

## Implementation location (once written)

`os/src/xtask/tests/` — to be finalized when `FEAT-P0-01` work begins.

## Reports

None yet.
