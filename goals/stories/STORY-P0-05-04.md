# STORY-P0-05-04 — End-to-End: `blue-sharc.exe` Loads and Runs Under QEMU (Tier 0)

Status: **Planned, not yet started**
Feature: [`FEAT-P0-05`](../features/FEAT-P0-05.md)
Introduced in: [`session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md`](../../session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md)

## Description

The flagship validation case for this whole Feature: an unmodified `blue-sharc.exe` build (`Sharc.Bluekind/target/gate-fast/blue-sharc.exe` in the sibling `Sharc.Blue` project) is fed through `STORY-P0-05-01`'s parser, `STORY-P0-05-02`'s mapper, and `STORY-P0-05-03`'s API shim, and actually runs as a scheduled TinyOS task under QEMU — proving the preceding three Stories compose correctly against a real, non-fixture binary, mirroring the role `TEST-P0-04-01-A` played for `STORY-P0-04-01` (a hand-crafted fixture is a useful additional test, never a substitute for testing against the real thing this Feature exists to support).

## Depends on

`STORY-P0-05-01`, `-02`, `-03` — this Story is pure integration, no new parsing/mapping/shim logic of its own.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. `blue-sharc.exe`, copied into the test fixture set unmodified from a real `Sharc.Blue` build (never patched or recompiled to "make it work" — if it doesn't load as-built, that's a defect in `STORY-P0-05-01`–`-03`, not a fixture problem to paper over), reaches a documented, minimal checkpoint under `xtask`'s QEMU harness: successful process entry and at least one successful heap allocation via the `STORY-P0-05-03` shim, exact checkpoint fixed precisely when this Story is picked up against whatever `blue-sharc.exe`'s actual early startup sequence does.
2. The checkpoint is verified via the same `isa-debug-exit` pass/fail pattern `TEST-P0-01-01-A`/`TEST-P0-04-01-A` established — no serial-output parsing required, a distinguishable success/failure exit code from `xtask`.
3. A deliberately-corrupted copy of `blue-sharc.exe` (mirroring `TEST-P0-01-03-A`'s `fixture-broken-boot` pattern) fails distinguishably rather than being silently accepted, exercising `STORY-P0-05-01`'s fail-closed parsing end-to-end, not just in its own host unit tests.
4. The full built OS image (kernel + hal + hal-x86_64 + exec, excluding drivers) still meets `G-DX-8`'s 8MB ceiling with this Story's fixture and any supporting code included — `check-crate-sizes`-style CI enforcement, applied to the whole image rather than one crate.

## Tests

Not yet written — deferred until this Story is picked up. Requires a Tier 0 (QEMU) integration test at minimum, per `agent/CODING_STANDARDS.md`'s "every driver/kernel path targets at minimum a Tier 0 test" mandate — this Story is, by design, entirely about Tier 0 verification of the preceding three Stories' composition.

## Goals verified

G-PC-1 through G-PC-4 (the full Feature, proven end to end against the real validation case).

**Correction (2026-07-26):** an earlier version of this Story additionally claimed `G-AI-1` (local inference hosting) here, on the premise that `blue-sharc.exe` is an inference runtime. It isn't — `blue-sharc.exe` is `Sharc.Blue`'s MCP sidecar/context engine for LLM tool-calling over stdio/IPC, a real and non-trivial native binary but not a model-inference workload. This Story verifies process/executable compatibility (`G-PC-1`–`G-PC-4`) only; `G-AI-1` remains Phase 6's own, separate goal to prove against TinyOS's own future `inference` crate. See `FEAT-P0-05.md`'s matching correction.
