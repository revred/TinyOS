# TEST-P0-05-04-A — `blue-sharc.exe` Boots to First Checkpoint Under QEMU

Status: **Specified — not yet implemented** (written ahead of `STORY-P0-05-04`, per the TDD mandate)
Story: [`STORY-P0-05-04`](../stories/STORY-P0-05-04.md)
Tier: Tier 0 (QEMU x86_64), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — the flagship end-to-end validation for `FEAT-P0-05`, mirroring the role `TEST-P0-04-01-A` played for `STORY-P0-04-01` (real target, not a fixture stand-in, is the point of this specific Test).

## Specification

**Given** the `kernel` crate built against `os/targets/x86_64-tinyos.json`, linking `exec`'s PE loader/mapper/API shim, with an unmodified `blue-sharc.exe` build (from the sibling `Sharc.Blue` project, `Sharc.Bluekind/target/gate-fast/blue-sharc.exe`) embedded as a boot-time fixture,
**when** it is booted under QEMU x86_64 (`q35` machine type) via `xtask qemu-x86_64` (or a Story-specific `xtask` subcommand, decided when `STORY-P0-05-04` is picked up),
**then**:
- `blue-sharc.exe`'s PE image is parsed, mapped, and its entry point is reached as a scheduled TinyOS task,
- it reaches a documented, minimal checkpoint (successful entry plus at least one successful heap allocation via the API shim — the precise checkpoint fixed against `blue-sharc.exe`'s actual early startup sequence when this Test is implemented, not guessed at now),
- the `isa-debug-exit` success code is reached — a parsing, mapping, or API-shim failure at any step instead reaches the failure code, so this Test cannot pass by accident,
- a deliberately-corrupted copy of the same `blue-sharc.exe` fixture (mirroring `TEST-P0-01-03-A`'s `fixture-broken-boot` pattern) fails distinguishably instead.

## Test type

Integration test (Tier 0, QEMU-based), per `agent/CODING_STANDARDS.md`'s "every driver/kernel path targets at minimum a Tier 0 test" requirement. This Test is pure composition — it exercises `STORY-P0-05-01`–`-03` together against the real validation case; it does not introduce new parsing/mapping/shim logic of its own (that logic's own dedicated tests are `TEST-P0-05-01-A` through `-03-A`).

## Implementation location

Expected in `os/src/kernel/src/main.rs` (or a Story-specific entry point) and `os/src/xtask/src/main.rs`, once `STORY-P0-05-01`–`-03` land — not yet created. Requires `blue-sharc.exe` to be available as a build/test fixture; exactly how it's sourced (vendored copy vs. built from the sibling `Sharc.Blue` project as part of CI) is a decision for whoever picks up `STORY-P0-05-04`, not fixed here.

## Reports

None yet — this Test has no implementation to report against.
