# TEST-P0-05-03-A — Win32 API Shim Rejects Non-Allowlisted and Adversarial Calls

Status: **Verified**
Story: [`STORY-P0-05-03`](../stories/STORY-P0-05-03.md)
Tier: Mixed — host unit test for allowlist-membership logic, Tier 0 (QEMU x86_64) for capability-gated call implementations, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix). This Test is explicitly adversarial, per `agent/CODING_STANDARDS.md`'s requirement for security-relevant subsystems ("actively try to violate the invariant," not just happy-path coverage).

## Specification

**Given** a loaded process making calls through the Win32 API compatibility shim,
**when** it calls:
- an import not on the explicit allowlist — **then** the process fails to load in the first place (link-time rejection, `STORY-P0-05-01`'s import list checked against the allowlist before any code runs), never a runtime stub that fails when invoked,
- an allowlisted call the ACI-equivalent capability model denies for this process (e.g. file access outside a granted scope, once that policy surface exists) — **then** the call returns the documented Windows-API-shaped error path (not a kernel panic, not a silent no-op that pretends to succeed), and the denial is audited the same way any other ACI-mediated denial is,
- an allowlisted call with an adversarial argument (e.g. a buffer pointer/length pair that would read or write outside the calling process's own mapped memory) — **then** the call fails closed without touching memory outside that process's own address space — verified by attempting to read/write a page belonging to a different process or to the kernel itself and confirming the shim rejects it before any actual access,
- a well-formed, in-allowlist, in-policy call with valid arguments — **then** it succeeds and behaves per the documented Windows API semantics for that call (baseline: whatever subset `blue-sharc.exe` actually exercises, since this Feature is not a general Windows compatibility layer).

## Test type

Adversarial security test, mixed Tier — the allowlist-membership check (is this `(DLL, symbol)` pair in the enumeration) is pure and host-testable; the capability-gated call implementations and out-of-bounds-memory rejection require real process/memory context, hence Tier 0. Per `agent/CODING_STANDARDS.md`, this Story is exactly the kind of security-relevant subsystem that gets adversarial tests as a requirement, not a nice-to-have.

## Implementation location

`os/src/exec/src/win32_shim.rs` (the shim module itself, with its host `#[cfg(test)]` tests) and `os/src/exec/src/fixture_win32_shim_main.rs` (the Tier 0 fixture, a new `win32-shim-fixture` `[[bin]]` in `exec/Cargo.toml`, wired into `xtask` as `--fixture=win32-shim`).

## Revision note (2026-07-26)

`aci` (Phase 5) did not exist in this workspace when `STORY-P0-05-03` was picked up, so this Test's "the ACI-equivalent capability model" wording is satisfied by `win32_shim::CapabilityPolicy`, a standalone trait this Story defined per its own dependency note — not a wrapper around a real `aci` crate. The "audited the same way any other ACI-mediated denial is" clause is **not** satisfied: there is no audit-log subsystem in this workspace yet (independent of `aci`), so a policy denial returns `ShimError::PolicyDenied` with no audit trail — a documented gap, see `STORY-P0-05-03.md` and `REPORT-2026-07-26-10`.

The out-of-bounds-buffer clause is verified by reading back `AddressSpace::translate` before any access is attempted, not by inducing and catching a real `#PF` — this kernel has no IDT/exception-handling subsystem yet (`STORY-P0-05-02`'s own handover names this same gap), so "fails closed... before any actual access" is enforced by a proactive bounds check, never a fault caught after the fact. Only two calls (`write_file`, `read_file`) got a real implementation, chosen as the representative pair needed to prove allowlist dispatch, policy-gating, and buffer bounds-checking end to end; the remaining seven allowlisted `Api` variants resolve and are capability-checkable but have no call body yet.

## Reports

[`REPORT-2026-07-26-10`](../reports/REPORT-2026-07-26-10.md) — Pass, both the host and Tier 0 halves.
