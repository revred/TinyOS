# Handover 07 — Phase 0 Walking Skeleton Implemented and Verified Locally

Status: **`FEAT-P0-01` Verified locally (CI run pending)**
Follows: [Cover Note 00](00-cover-note.md)'s Phase 0 delivery mandate

## What this session did

Acted on the cover note's mandate to begin Phase 0 implementation rather than leave it as a standing instruction. Confirmed toolchain/hardware readiness (QEMU-only Tier 0 bring-up, per the cover note's step 1), installed the missing pieces (QEMU via winget, a pinned nightly Rust toolchain with `rust-src`), then implemented and verified all three `FEAT-P0-01` Stories:

- **`STORY-P0-01-01`** — `os/src/kernel/` boots to a clean halt under QEMU x86_64. Implemented via the **Xen PVH** direct-boot protocol (not Multiboot — QEMU's `-kernel` multiboot loader only accepts 32-bit ELF images, incompatible with our ELF64 `no_std` kernel), a hand-written 32-bit-to-long-mode transition in `os/src/kernel/src/boot.rs`, and a QEMU `isa-debug-exit`-based pass/fail signal (`os/src/kernel/src/qemu_exit.rs`) instead of serial-console parsing.
- **`STORY-P0-01-02`** — CI governance gates (`rustfmt`, `clippy -D warnings`, crate-size ceiling, `missing_docs`) wired into `.github/workflows/ci.yml`, with a 4-fixture smoke test (`os/src/xtask/src/governance.rs`) proving each gate fails exactly the violation it targets.
- **`STORY-P0-01-03`** — `cargo run -p xtask -- qemu-x86_64` builds and boots the kernel, with a bounded (15s) startup timeout and a distinguishable exit-code contract (0 = boot succeeded, 1 = kernel panicked, 2 = harness/QEMU problem), verified against both a normal boot and a deliberate `--fixture=broken-boot` panic path.

All three Tests (`TEST-P0-01-01-A/02-A/03-A`) pass locally against the pinned `nightly-2026-07-26` toolchain — see `goals/reports/REPORT-2026-07-26-01` through `-03`. **None have yet been observed passing in the actual GitHub Actions CI run** — the workflow is authored but not yet triggered against a pushed commit.

## Decisions recorded as ADRs

- [`docs/adr/0001-nightly-toolchain-for-build-std.md`](../../docs/adr/0001-nightly-toolchain-for-build-std.md) — nightly is required solely for `-Z build-std` (no stable equivalent exists); scoped to `kernel`/`hal`/`hal-x86_64` only.
- [`docs/adr/0002-no-msvc-dependency-on-windows.md`](../../docs/adr/0002-no-msvc-dependency-on-windows.md) — `xtask`'s host build does not depend on Visual Studio/MSVC (an actual VS Build Tools install attempt failed on this machine, installer exit code 1602); uses `rust-lld` + a `cargo-xwin` import-library cache instead, mirroring the pattern already established in the sibling `Sharc.Blue` project. **Known gap, carried over from that project's own unresolved issue**: the `LIB` path in `os/.cargo/config.toml` is this machine's absolute, username-bearing path — not reproducible for a second Windows contributor as committed. Flagged, not silently accepted.

## What's still open

- **CI has not actually run.** Everything above is verified locally only. The first real CI run against `.github/workflows/ci.yml` should happen before `FEAT-P0-01` is considered fully closed out in practice, even though its Stories/Tests are marked Verified based on local passes (per the Report format's explicit tier field, this is stated, not hidden).
- **`FEAT-P0-02` through `FEAT-P0-04`** (scheduler, memory pools, ACPI/HAL backend) remain un-decomposed and not started, per the just-in-time decomposition principle — this session deliberately did not get ahead of them.
- **The Windows `LIB` path gap** in ADR 0002 should be resolved (dynamic discovery, not a hardcoded path) before a second Windows contributor tries to build this workspace.
- Per the cover note's own delivery mandate, this work was done single-threaded rather than fanned out across parallel subagents — the walking skeleton's pieces (kernel boot, `xtask`, CI) turned out to be tightly coupled (each blocks verifying the others), which made single-threaded, verify-as-you-go implementation safer than parallel Stories that would have needed to integrate blind. The CI governance-gate fixtures (Story 02) were the one piece genuinely independent of kernel/xtask internals; a future session with more Features in flight (once `FEAT-P0-02`–`04` are decomposed) is a better fit for the parallel-subagent pattern the cover note describes.

## Updated in this session

`goals/index.html`, `goals/traceability-matrix.md`, `goals/reports/README.md` and three new Report files, `goals/epics/EPIC-P0.md`, `goals/features/FEAT-P0-01.md`, all three `goals/stories/STORY-P0-01-*.md` and `goals/tests/TEST-P0-01-*.md` files — per the cover note's requirement that "substantial" be verifiable from the dashboard, not just claimed here.
