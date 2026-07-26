# STORY-P0-05-03 — Capability-Scoped Win32 API Compatibility Shim

Status: **Verified**
Feature: [`FEAT-P0-05`](../features/FEAT-P0-05.md)
Introduced in: [`session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md`](../../session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md)

## Description

Implement the small, explicitly-enumerated Windows API surface a loaded executable can actually call — sized to exactly what `blue-sharc.exe` imports (process/thread basics, file/mmap access, heap allocation, console I/O), per `G-PC-2`, never a general `kernel32`/`ntdll` reimplementation. Every emulated call is mediated by the same ACI capability model that governs every other caller (`G-PC-3`) — this Story is where "loading a program" and "granting it ambient kernel authority" are kept structurally separate, the central security property this whole Feature exists to prove is achievable.

**Naming direction (2026-07-26):** this shim is TinyOS's own native API, not a Win32 clone with the serial numbers filed off — it should read as a first-class, well-documented, secure-by-design "TinyOS64 API" (working name) in its own right, in the same tradition Win32 itself set for Windows: a stable, well-specified, comprehensively-documented contract application code is written against. The PE-compatibility shim (this Story) satisfies binary compatibility with `blue-sharc.exe`'s *imports*; a native `Tos64` API is a separate, forward-looking design surface for code written for TinyOS directly, not required for this Story's own acceptance criteria but worth keeping in view so the shim's internal structure (allowlist entries, capability-scoped call implementations) doesn't end up shaped only for PE-import compatibility with no native counterpart.

## Depends on

`STORY-P0-05-01` (the parsed import list this Story's allowlist checks against) and the ACI policy engine's capability model as far as it exists at the point this Story is picked up — `aci` (Phase 5 per `docs/mvp-delivery-strategy.md`'s crate map) did **not** exist in this workspace when this Story was picked up, so per this Story's own text it defined the minimal capability-check shape it needs standalone (`exec::win32_shim::CapabilityPolicy`) rather than blocking Phase 0 work on a Phase 5 crate, with the migration path to the real `aci` documented in `win32_shim.rs`'s own doc comment. This dependency-ordering resolution is worth restating for whoever implements `aci`, not assumed obvious from the code alone.

## Acceptance criteria

1. The allowlist is an explicit, closed enumeration (`exec::win32_shim::Api`, resolved by `exec::win32_shim::resolve` — a single designated dispatch point matching a fixed set of `(DLL, symbol)` pairs) — adding a new supported call is an additive change to that enumeration and `resolve`, never a fallback "try to guess what this import wants" path, per the Open/Closed translation in `agent/CODING_STANDARDS.md`.
2. An import `STORY-P0-05-01` parsed that isn't on the allowlist is a load-time rejection (`exec::win32_shim::check_imports` returns `Err(ShimError::NotAllowlisted)` before any code runs), not a runtime stub that fails when called — a caller can never link successfully against a capability it doesn't actually have.
3. Every allowlisted call's implementation is capability-scoped: `write_file`/`read_file` (the two call implementations shipped this Story, chosen to prove the mechanism end to end) both check `CapabilityPolicy::is_granted` before doing anything else, returning `ShimError::PolicyDenied` on denial rather than a direct unmediated effect — no privileged bypass for a loaded executable, restated from `G-PC-3`/Non-Negotiable #2. The remaining seven allowlisted `Api` variants (`GetCurrentProcess`, `ExitProcess`, `HeapAlloc`, `HeapFree`, `GetStdHandle`, `CreateFileA`, `CloseHandle`) resolve and are capability-checkable but have no call implementation yet — adding one is additive (a new function following `write_file`/`read_file`'s pattern), not a redesign, and is out of this Story's own scope (see `FEAT-P0-05.md`/this Story's linked Report for what's deferred).
4. Adversarial tests exist for this Story specifically (`exec/src/win32_shim.rs`'s `#[cfg(test)]` module, `exec/src/fixture_win32_shim_main.rs`'s Tier 0 fixture): a call outside the allowlist is rejected (`check_imports_rejects_a_non_allowlisted_import`), a policy-denied allowlisted call is rejected without a silent no-op (`write_file_is_rejected_when_the_policy_denies_it`, and the Tier 0 fixture's equivalent), and an adversarial buffer argument — one running past its section's single mapped page, and one pointing squarely at the kernel's own reserved region — fails closed with `ShimError::OutOfBounds` *before* any access, verified by reading back `AddressSpace::translate` rather than inducing a real `#PF` (this kernel has no IDT/exception-handling subsystem yet, per `STORY-P0-05-02`'s handover — the same reason that Story's own permission checks are read-back-based, not fault-based).

## Tests

[`TEST-P0-05-03-A`](../tests/TEST-P0-05-03-A.md) — see that Test's revision note and [`REPORT-2026-07-26-10`](../reports/REPORT-2026-07-26-10.md) for the full pass record. Mixed Tier as anticipated: `exec::win32_shim`'s `#[cfg(test)]` module covers the allowlist-membership/policy-check logic on the host; `exec/src/fixture_win32_shim_main.rs` (a new `win32-shim-fixture` `[[bin]]`, `cargo run -p xtask -- qemu-x86_64 --fixture=win32-shim`) covers the capability-gated calls and buffer bounds-checking under real target paging.

## Goals verified

G-PC-2 (minimal, explicitly-enumerated compatibility surface), G-PC-3 (no privileged bypass for a loaded executable), G-AI-3 (no privileged bypass — restated here because an executable-loading path is exactly the kind of "it's not really an AI caller" case that goal exists to prevent from being treated as exempt).
