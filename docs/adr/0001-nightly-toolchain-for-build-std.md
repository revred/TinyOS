# ADR 0001 — Nightly Rust Toolchain for `-Z build-std` in Phase 0

Status: **Accepted**
Date: 2026-07-26
Introduced in: [`session/hand-2026-07-26/`](../../session/hand-2026-07-26/) (Phase 0 walking skeleton implementation)

## Context

[`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md#toolchain) sets stable Rust as the default channel, permitting nightly-only features solely when no stable equivalent exists, recorded here per that policy. [`STORY-P0-01-01`](../../goals/stories/STORY-P0-01-01.md) and [`docs/mvp-delivery-strategy.md`](../mvp-delivery-strategy.md#custom-target-specs) both specify `cargo build -p kernel --target os/targets/x86_64-tinyos.json -Z build-std`.

A custom target-spec JSON (required here because the kernel is bare-metal `no_std` with a disabled red zone, a bare code model, and no SIMD in kernel context — none of which the built-in `x86_64-unknown-none` target's prebuilt `core`/`alloc` match) has no prebuilt standard library shipped for it. `-Z build-std` recompiles `core`, `alloc`, and (where used) `compiler_builtins` from source against the custom target. This flag is nightly-only and has been for the life of the feature; there is no stable equivalent as of this writing (Rust 1.99, 2026-07).

## Decision

The `kernel`, `hal`, and `hal-x86_64` crates (and any other crate built against a custom `os/targets/*.json` target spec) are built with the pinned **nightly** toolchain via `-Z build-std=core,alloc,compiler_builtins -Z build-std-features=compiler-builtins-mem`, invoked exclusively through `xtask` so no contributor types the flag by hand. All other workspace crates (`xtask`, `deploy-client`, `bridge-host`, and any other `std` host tooling) continue to build against stable, since they target the host triple with a prebuilt standard library.

`rust-toolchain.toml` pins a specific nightly date (not a floating `nightly` channel), matching the existing "pinned Rust version" policy, so CI and every contributor build against the exact same compiler.

## Consequences

- The workspace has two effective toolchain requirements depending on crate: nightly for bare-metal targets, stable-compatible for host tools. `xtask` and CI both invoke the correct one per crate rather than forcing the whole workspace onto nightly.
- `-Z build-std` is re-evaluated at every `rust-toolchain.toml` bump: if a future stable release stabilizes `build-std` (tracked upstream as an unstabilized Cargo feature with no committed stabilization date at time of writing), this ADR is superseded and the pin reverts to stable.
- No other nightly-only language/library feature is used in kernel code without its own ADR entry — this ADR covers the `build-std` mechanism only, not a blanket nightly allowance.

## Removal / stabilization plan

Revisit this ADR whenever `rust-toolchain.toml`'s pinned version is bumped: check whether `-Z build-std` has stabilized upstream. If so, drop the nightly pin for `kernel`/`hal`/`hal-x86_64` and mark this ADR **Superseded**.
