# TinyOS Coding Standards

Status: **draft — governs all code from Phase 0 onward**

## Priority ordering

When any trade-off is required — in design, in code review, in a deadline crunch — TinyOS resolves it in this order, without exception:

1. **Safety.** A decision that could harm a person, a machine, or a controlled process is never made for velocity, elegance, or convenience.
2. **Security.** No privileged bypass, no unauthenticated command path, no shortcut around the ACI/HBP/WCI trust model — see [Non-Negotiables](README.md#non-negotiables).
3. **Correctness.** Proven through TDD (below), not asserted. A feature is not done until its tests are.
4. **Performance.** Once safety, security, and correctness hold, TinyOS competes on squeezing the maximum throughput and lowest latency out of the target hardware — from constrained edge devices to full-capability laptops. This is a first-class design goal, not an afterthought.

Development speed and code convenience are weighed only against priority 4, and never against 1–3.

## Language policy

TinyOS is written primarily in **Rust**. Rust is the default for every new crate — kernel, HAL, drivers, ACI, shell, host bridge services, and agent integration. A component is written in something other than Rust only when there is a specific, documented reason:

- **Boot/entry assembly.** The earliest boot stub, architecture-specific trap/interrupt entry glue, and context-switch register save/restore may be written in a small amount of hand-written assembly (x86_64 / AArch64), wrapped by a Rust `extern "C"` boundary as thin as possible.
- **Vendor SDK bindings.** Where a hardware vendor only ships a C SDK (e.g. GPU/NPU driver blobs, CAN transceiver reference code), the binding lives in an isolated `-sys` crate that wraps the C API and exposes a safe Rust API to the rest of the system. No vendor C header is ever `#include`d or called outside a `-sys` crate.
- Anything else proposed in a language other than Rust requires an ADR (architecture decision record) under `/docs/adr/` explaining why Rust doesn't fit.

Assembly and C footprint should trend toward zero as the HAL matures and as pure-Rust replacements (or safe wrappers) become available.

## `no_std` vs `std`

- **Kernel, HAL, drivers, ACI, shell:** `#![no_std]`. No implicit heap; allocation, if any, goes through the kernel's own static/pool allocators, never the global allocator of a hosted OS.
- **Host bridge services (Windows/Linux side) and dev tooling:** `std` is fine — these run as ordinary host processes, not inside the TinyOS kernel.
- **Agent/inference host integration:** the supervisory wrapper around the local LLM runtime runs as a budgeted TinyOS task; whether it is `no_std` or hosts a constrained `std`-like runtime is decided per Roadmap Phase 6 design work, but it is never granted the RT scheduler's guarantees — see [Non-Negotiables](README.md#non-negotiables) in the README.

## Toolchain

- A pinned Rust version is declared in `rust-toolchain.toml` at the workspace root; CI builds against exactly that version.
- **Stable channel by default.** Nightly-only features are permitted solely when no stable equivalent exists, and only when the specific feature and its removal/stabilization plan is recorded in an ADR under `/docs/adr/`.
- The workspace is a single Cargo workspace; component boundaries in [`README.md`](README.md#repository-layout-planned) (`/kernel`, `/hal`, `/drivers`, `/bridge`, `/shell`, `/aci`, `/agent`) map 1:1 to top-level crates.

## Formatting & linting

- `rustfmt` is authoritative for formatting; a workspace `rustfmt.toml` is committed and CI fails on unformatted code.
- `clippy` runs in CI with warnings promoted to errors (`-D warnings`) on every crate. Justified exceptions are scoped as narrowly as possible with `#[allow(clippy::x)]` plus a comment explaining why, never a blanket crate-level allow without an ADR.
- Public API documentation is enforced via `#![deny(missing_docs)]` on library crates. Documentation explains invariants and *why*, not a restatement of the signature.

## Unsafe code policy

- `unsafe` is a boundary-layer tool, not a convenience. Application-level crates (`aci`, `agent`, `shell`) carry `#![forbid(unsafe_code)]`. `unsafe` is permitted only in `hal`, `drivers`, and `-sys` binding crates.
- Every `unsafe` block is preceded by a `// SAFETY:` comment stating the invariant that makes it sound — not what the code does, but why it's safe to do it here.
- `unsafe` blocks are kept minimal: wrap the smallest possible operation, then return to safe Rust immediately. Do not mark whole functions `unsafe` when only one FFI call inside needs it.
- New `unsafe` in a PR is called out explicitly in the PR description, not left for reviewers to discover in the diff.

## Real-time discipline (kernel and driver code)

This is where TinyOS's coding standard diverges hardest from general Rust practice, because correctness alone isn't the bar — bounded, predictable timing is.

- **No heap allocation in any scheduler, IPC, or interrupt-handling hot path.** Use static allocation, fixed-capacity data structures (`heapless`-style), or pool allocators with a compile-time-bounded pool size. If a path might allocate, treat that as a defect, not a style nit.
- **No unbounded loops or unbounded blocking in RT-path code.** Every loop over external input has an explicit bound; every lock acquired on an RT path has a documented maximum hold time.
- **`panic!` is not error handling.** RT-path code returns `Result` and propagates typed errors. A `panic!` in kernel code is a last-resort kernel-fault path that hands off to the watchdog/failsafe system (see README Non-Negotiable #5), not a substitute for handling an expected error.
- **Every RT task declares its worst-case execution time (WCET) budget** as part of its task definition; code changes that plausibly affect WCET require an update to the timing regression suite (Roadmap Phase 1), not just a passing functional test.
- Non-RT code (shell, host bridge services, agent supervisor) does not inherit these constraints wholesale, but must still never allow its own blocking/allocation behavior to leak into an RT task's execution path — that boundary is the one rule that's never relaxed.

## Concurrency

- Prefer message-passing (typed channels, the internal message bus) over shared mutable state.
- Where shared state is unavoidable in RT paths, use lock-free structures (atomics, SPSC/MPSC ring buffers) over blocking locks.
- Any blocking primitive (mutex, condvar) used outside an RT path must document its expected and worst-case hold time in a doc comment.

## Error handling

- Library crates define their own error enums (`thiserror`-style, or hand-rolled in `no_std` contexts) — no stringly-typed errors, no `Box<dyn Error>` in `no_std` code.
- Errors crossing a subsystem boundary (e.g. driver → kernel, ACI → agent) are part of that subsystem's public API and are documented like any other contract.
- Silent error swallowing (`let _ = fallible_call()`) requires a comment stating why the error is genuinely safe to discard.

## Test-Driven Development (mandatory)

Every feature in TinyOS — kernel, driver, ACI capability, shell command, deploy tooling, agent integration, all of it — is built test-first. This is not a preference; it is how correctness is proven under Priority 3 above.

- **Red, green, refactor.** A failing test is written and present before the implementation that makes it pass. A PR whose tests were written after the implementation (and clearly reverse-engineered from it) is not TDD and is treated as a gap, not a formality satisfied.
- **No exceptions for "trivial" code.** Real-time paths, ACI capability checks, and deploy/hot-swap logic are exactly where an untested edge case turns into a safety or security incident — these get the most rigorous test-first treatment, not the least.
- **Adversarial tests for security/safety-relevant code.** The ACI policy engine, HBP/WCI authentication and authority-lease logic, watchdog/failsafe paths, and deploy tooling all require tests that actively try to violate the invariant — an unauthenticated command, an expired lease, a replayed frame, a command issued mid-reboot — not just happy-path coverage.
- **Timing-sensitive code** additionally requires a benchmark/regression entry under `/tests/` per Roadmap Phase 1 — a functional pass with a timing regression is a CI failure, exactly like a functional test failure.
- **Hardware-in-the-loop (HIL) and QEMU-based integration tests** live under `/tests/`, mirroring the [Target Hardware & Test Matrix](README.md#target-hardware--test-matrix) tiers — every driver targets at minimum a Tier 0 (QEMU/Renode) test before a Tier 1/2 hardware test is required.
- CI enforces this at the process level: a PR that changes implementation without a corresponding test change is flagged for review rather than silently merged.

## Tooling

Tooling is a first-class part of the coding standard, not an afterthought bolted on once the kernel works — a fast, secure, remote-first development loop is what makes squeezing performance out of real hardware (Priority 4) practical to iterate on at all.

- **Reproducible builds.** The full system image builds from a single command against the pinned toolchain (see Toolchain, below); no developer-machine-specific state is allowed to leak into a build.
- **Remote-first deploy.** A dedicated deploy tool connects to a running TinyOS device to reboot it onto a new image or hot-deploy an updated component, over either:
  - a **peer-to-peer Ethernet cable** (link-local addressing, no switch or DHCP server required), or
  - **WiFi**, reusing the [WCI](README.md#remote-control-the-wireless-command-interface-wci) authenticated pairing and session model.
  Both mechanisms go through the same ACI capability gate as everything else — deploy is a scoped `deployer` capability, never a bypass — and are fully audited. See [`docs/deploy-protocol.md`](docs/deploy-protocol.md) for the wire-level spec.
- **Hot deploy vs. reboot deploy.** Non-core tasks and drivers that support safe hot-swap can be updated live; kernel-core updates always go through a full reboot with A/B partition boot and automatic rollback on a failed boot-health check. A deploy is never allowed to leave a device in a state where it can neither run the old image nor the new one.
- **Observability is mandatory, not optional.** Every subsystem ships with a way to inspect its live state remotely, over the same secure channel used for deploy — no subsystem is considered complete if the only way to debug it is a serial cable and a debugger breakpoint.
- Tooling itself is held to the same standard as the OS it operates on: no privileged bypass, full audit trail, fail-safe defaults if the connection drops mid-operation.

## Commit & PR conventions

- Commit messages describe *why*, not a restatement of the diff — consistent with how this repository's history is written.
- A PR introducing new `unsafe`, a new dependency, or a nightly-only feature calls that out explicitly in the description.
- Dependencies are kept minimal, especially in kernel/HAL crates: every new dependency in a `no_std` crate is justified in the PR (audit surface and binary size both matter on edge targets).

## Style notes (beyond what `rustfmt`/`clippy` already enforce)

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) naming conventions (`CamelCase` types, `snake_case` functions, no stuttering module/type names).
- Prefer newtypes over bare primitives for anything that isn't "just a number" — task IDs, capability tokens, session IDs, sequence numbers — so the type system catches misuse the RT scheduler can't afford to catch at runtime.
- Keep module boundaries aligned with the [Repository Layout](README.md#repository-layout-planned); don't let kernel-internal types leak into `aci`'s public API without an explicit, minimal translation layer.

## Status

This document will grow alongside Roadmap Phase 0 as the kernel skeleton lands and real patterns (not just policy) emerge. Treat it as binding for new code today and expect refinement, not replacement, as the codebase grows.
