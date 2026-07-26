# FEAT-P0-05 — Executable Loading & Process Compatibility (`Sharc.Blue` PE Loader)

Status: **In progress — 3/4 Stories Verified**
Epic: [`EPIC-P0`](../epics/EPIC-P0.md)
Introduced in: [`session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md`](../../session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md)

## Description

TinyOS can load and run a real, externally-built native PE64 executable as a scheduled task — not just its own code — per `SeedMVP.md` §3.7's `G-PC-1` through `G-PC-4`. The flagship validation case, and the reason this Feature exists now rather than later in the roadmap, is `Sharc.Blue`'s `blue-sharc.exe` (`Sharc.Bluekind/target/gate-fast/blue-sharc.exe` in the sibling project) — a Rust MCP sidecar/context engine for LLM tool-calling over stdio/IPC, **not** a model-inference runtime itself (corrected 2026-07-26; an earlier session's "Ollama-like" framing here conflated it with TinyOS's own, separate Phase 6 direction of hosting an Ollama-like runtime using storage/mmap patterns studied from Sharc.Blue's `.rac` substrate — see [`docs/inference-architecture.md`](../../docs/inference-architecture.md) and the `inference-mmap-model-loading` project memory entry, both of which are about TinyOS's future `inference` crate, not about `blue-sharc.exe`'s own function). Proving TinyOS can host `blue-sharc.exe` unmodified is still a concrete, falsifiable milestone — "TinyOS can run a real, non-trivial native Windows binary with a real IPC/stdio surface" — just not evidence toward "TinyOS hosts inference workloads," which remains Phase 6's own, later claim to prove.

This is deliberately **not** a general Windows compatibility layer. Per `G-PC-2`, the API surface a loaded executable can call is a small, explicitly-enumerated allowlist sized to what `blue-sharc.exe` (and similarly `rust-lld`/`cargo-xwin`-built sibling-project binaries, per [ADR 0002](../../docs/adr/0002-no-msvc-dependency-on-windows.md)) actually imports — not a `kernel32`/`ntdll` reimplementation. An import outside that allowlist is a load-time rejection, not a best-effort stub. This scoping is what keeps the Feature compatible with `G-DX-8`'s 8MB total-image ceiling and with the "unhackable by design" security posture directing this work: every emulated API call is mediated by the same ACI capability model as any other caller (`G-PC-3`), and the loader itself — parsing untrusted, externally-supplied PE headers/sections/imports — gets the same adversarial-input discipline `hal-x86_64::acpi` already established for untrusted ACPI tables (`G-PC-4`).

## Crate(s) involved

`os/src/exec/` (new crate, per `docs/mvp-delivery-strategy.md`'s crate map). Depends on `os/src/kernel/` (a task to run the loaded executable as, `FEAT-P0-02`) and `os/src/kernel/` (`mem::Pool`-based address-space/section storage, `FEAT-P0-03`) — see "Depends on" below for exactly how much of each is needed before implementation can start.

## Depends on

- `FEAT-P0-01` (a booting kernel to run inside).
- `FEAT-P0-02`, at minimum through `STORY-P0-02-02` (context switch) — a loaded executable is only meaningfully "running" once the scheduler can actually preempt/resume it, not just create a `Tcb` for it. `STORY-P0-02-01` alone (task creation) is necessary but not sufficient.
- `FEAT-P0-03`'s `STORY-P0-03-01`/`-03` (the `Pool<T, N>` allocator this Feature's Stories reuse for section/segment bookkeeping — already Verified) and likely `STORY-P0-03-02` (compile-time pool-size configuration) once this Feature gives it a first concrete consumer to size against, per that Story's own deferral note.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P0-05-01`](../stories/STORY-P0-05-01.md) | PE64 image parsing into a validated, typed load descriptor | Verified |
| [`STORY-P0-05-02`](../stories/STORY-P0-05-02.md) | Process address-space creation and section mapping | Verified |
| [`STORY-P0-05-03`](../stories/STORY-P0-05-03.md) | Capability-scoped Win32 API compatibility shim | Verified |
| [`STORY-P0-05-04`](../stories/STORY-P0-05-04.md) | End-to-end: `blue-sharc.exe` loads and runs under QEMU (Tier 0) | Planned, not yet started |

`STORY-P0-05-01` is host-testable pure parsing logic with no dependency on `FEAT-P0-02`'s scheduler progress (see `STORY-P0-05-01.md`'s "Depends on"), so it was implemented and Verified in an earlier session ([`REPORT-2026-07-26-07`](../reports/REPORT-2026-07-26-07.md)) independent of that blocker. `STORY-P0-05-02` (process address-space creation and section mapping) implemented and Verified in the following session ([`REPORT-2026-07-26-09`](../reports/REPORT-2026-07-26-09.md)), now that `FEAT-P0-02`'s `STORY-P0-02-02` had landed — `exec::address_space::AddressSpace` (new module) plus a new `hal_x86_64::paging` module for the underlying x86_64 page-table construction. `STORY-P0-05-03` (Win32 API compatibility shim) implemented and Verified this session ([`REPORT-2026-07-26-10`](../reports/REPORT-2026-07-26-10.md)) — a new `exec::win32_shim` module resolving imports against a closed allowlist, mediating every call through a standalone `CapabilityPolicy` trait (the real `aci` crate doesn't exist yet, per that Story's own dependency note), and bounds-checking every buffer argument against the calling process's `AddressSpace` before any access. `STORY-P0-05-04` (end-to-end `blue-sharc.exe`) is the last Story remaining for this Feature.

## Exit criteria

- `STORY-P0-05-01` through `-04` all reach **Verified**.
- `blue-sharc.exe`, unmodified from the sibling `Sharc.Blue` project's own build output, loads and runs to at least its first observable checkpoint (a documented, minimal bar — e.g. successful process entry and a first heap allocation — fixed precisely when `STORY-P0-05-04` is picked up, not before) under `xtask`'s QEMU harness.
- The total built OS image, excluding `os/src/drivers*`, still meets `G-DX-8`'s 8MB ceiling with `exec` included.
