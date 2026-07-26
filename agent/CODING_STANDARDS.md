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

## Crate size ceiling (hard limit, no exceptions)

**No crate may exceed 20,000 lines of code, excluding test code.** This is a structural constraint, not a style preference, and it is enforced the same way for every crate in the workspace — kernel, HAL, drivers, ACI, shell, agent, bridge, compute.

- **Measurement.** Line count is measured by an automated tool (e.g. `tokei`) configured to exclude `#[cfg(test)]` modules and files under a crate's `tests/` directory. CI computes this on every PR and fails the build if any crate crosses the ceiling.
- **Rationale.** A crate is a unit of comprehension as much as a unit of compilation. Past roughly 20K lines, a single contributor can no longer hold a crate's invariants in their head, review quality degrades, and — specifically for TinyOS — the blast radius of a bug inside one crate grows past what the [Universal Driver Model](docs/universal-driver-model.md)'s isolation guarantees are designed around. The ceiling exists to force decomposition before that happens, not to catch it after.
- **No size-based exception process.** There is no ADR that waives this limit for an "important" or "central" crate — the correct response to a crate approaching 20K lines is always to split it, never to request an exception. If a crate seems impossible to split, that is itself a design smell (see SOLID, below) worth escalating.
- **Splitting strategy.** When a crate approaches ~16,000 lines (80% of ceiling — the trigger point for action, not the limit itself), the next PR that would grow it further must instead extract a cohesive sub-module into its own crate. A natural split follows the crate's own internal seams: a `kernel` crate nearing the ceiling typically separates cleanly into `kernel-sched`, `kernel-ipc`, and `kernel-mem`, for example, each independently testable and each with its own, smaller surface. The split is a normal PR, reviewed like any other — not a special "refactor sprint."
- **Worked example.** Suppose `drivers` (the crate housing early class-driver implementations) is approaching 18,000 lines because storage, network, and HID class drivers all live in it. The fix is not to compress code or delete comments — it's to extract `drivers-storage`, `drivers-net`, and `drivers-hid` as separate crates, each implementing the [Driver Capability Interface](docs/universal-driver-model.md#driver-capability-interface-dci-the-stable-contract) independently, with `drivers` (if it survives at all) reduced to shared enumeration glue.
- **Applies from Phase 0.** The ceiling is not a "we'll worry about it later" concern — CI enforces it starting with the very first crate in the kernel skeleton, so the workspace never accumulates a monolith that's painful to split retroactively.

## SOLID principles — Rust-adapted, never compromised

SOLID was written for object-oriented languages, but every one of its principles has a direct, idiomatic Rust translation, and TinyOS treats all five as non-negotiable — not aspirational guidance, but a review-blocking requirement alongside `clippy` and `rustfmt`. "Never compromised" means: a PR that violates one of these is not merged with a "we'll fix it later" comment. It's fixed before merge, or the PR is redesigned.

### S — Single Responsibility

- Every `struct`, `enum`, and `trait` has exactly one reason to change. If describing what a type does requires the word "and" in a way that implies two unrelated concerns (e.g. "parses DOS command syntax *and* manages the authority lease"), it is two types, not one.
- Applied at the module level too: a module's `mod.rs`/lib root should be describable in one sentence without a conjunction joining unrelated responsibilities.
- Enforcement: code review checklist item; a practical proxy is function and type size — a struct whose `impl` block exceeds roughly 300–400 lines, or a function exceeding roughly 50 lines, is a strong signal of a responsibility split waiting to happen, and is flagged in review even though no automated lint catches this directly.

### O — Open/Closed

- Extend behavior by adding a new trait implementation, not by editing an existing one's match arms. TinyOS's own capability model already demonstrates this: adding a new ACI capability, a new WCI authority scope, or a new UDI device class should be additive — a new implementor of an existing trait — never a modification to a central `match` statement that enumerates every known case.
- Where a genuine central dispatch point is unavoidable (e.g. routing a frame to the correct lane), that dispatch point is kept as thin as possible and is the *only* place allowed to know about every variant — it is explicitly exempted from Single Responsibility's "no growing match statements" guidance because its one responsibility *is* dispatch, and it is reviewed with extra scrutiny precisely because it's the one place required to change when something new is added.
- Enforcement: PRs that add a new variant to an existing enum outside of a designated extension point (and touch unrelated code to handle it) are flagged in review as an Open/Closed violation.

### L — Liskov Substitution

- Any type implementing a trait must be substitutable for any other implementor without the caller needing to know which one it got. A `-sys` vendor binding wrapped behind the DCI must behave identically, from the caller's perspective, to the generic class driver it extends — no implementor may narrow the trait's documented contract (e.g. returning an error where the trait promises success is never valid, even if "this hardware just doesn't support that").
- Trait contracts are documented with their invariants, not just their signatures (per the Formatting & linting section's `missing_docs` policy), specifically so Liskov violations are checkable by a reviewer against the doc comment, not just against the compiler.
- Enforcement: every trait with more than one implementor requires a **shared conformance test suite** that runs against every implementor identically (the same pattern already specified for driver class conformance in the Universal Driver Model) — a Liskov violation shows up as one implementor failing a test the others pass.

### I — Interface Segregation

- Traits are kept small and role-specific. A driver that only needs to read a device's state should depend on a `DeviceStatus` trait, not a monolithic `Device` trait that also exposes write/configure/reset methods it never calls — because depending on the larger trait means it's coupled to (and could plausibly be broken by) changes to methods it never uses.
- This directly serves the [Unsafe code policy](#unsafe-code-policy) and the Universal Driver Model's capability-scoping: a narrow trait is also a narrow attack surface — a caller that only holds a `DeviceStatus` capability literally cannot invoke a write operation, because the type it was handed doesn't have one.
- Enforcement: review flags any trait with more than roughly 5–7 methods as a segregation candidate, and any struct implementing a trait where more than one method is a stub (`unimplemented!()`, or returns a fixed "not supported" error) as a sign the trait was too broad in the first place.

### D — Dependency Inversion

- High-level modules (the ACI policy engine, the scheduler) depend on trait abstractions, never on concrete driver or transport types. The ACI doesn't know whether a command arrived over HBP, WCI, or the local shell — it depends on a `Caller` abstraction, and each transport implements it. This is already the architecture described throughout the README (HBP, WCI, and the local shell are three implementors of one caller abstraction feeding one policy engine) — Dependency Inversion is the formal name for a pattern TinyOS already committed to structurally, and this section makes that commitment explicit at the code level.
- Concretely in Rust: prefer `fn handle(caller: &dyn Caller)` or a generic `fn handle<C: Caller>(caller: &C)` over `fn handle(caller: &HbpSession)` in any code that has no genuine reason to be HBP-specific.
- Enforcement: a PR introducing a concrete-type dependency in a high-level module (kernel, ACI, scheduler) where a trait abstraction already exists for that role is flagged in review as a Dependency Inversion violation; introducing a *new* concrete-type dependency where no abstraction yet exists is the trigger to define one, not a justification to skip it.

### Enforcement summary

| Principle | Primary enforcement mechanism |
|---|---|
| Single Responsibility | Code review checklist + function/type size as a review-time proxy signal |
| Open/Closed | Review flag on unrelated changes to match-statement dispatch points outside designated extension points |
| Liskov Substitution | Shared conformance test suite required for every trait with 2+ implementors |
| Interface Segregation | Review flag on traits >5–7 methods or implementors with stubbed methods |
| Dependency Inversion | Review flag on concrete-type dependencies in kernel/ACI/scheduler code where a trait abstraction exists or should exist |

None of these are automated to the same degree as `clippy`/`rustfmt` today — Rust tooling doesn't yet have a turnkey SOLID linter — which is exactly why they're reviewer-enforced, checklist items, and treated as blocking rather than advisory. As tooling matures (custom `clippy` lints, `cargo-geiger`-style static analysis for trait-object usage), automation should replace manual review checks wherever it reliably can, without lowering the bar in the meantime.

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
