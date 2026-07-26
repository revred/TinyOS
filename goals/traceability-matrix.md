# Traceability Matrix

Status: **Living document — update in the same PR that adds or changes any Story or Test**

The single-page cross-reference between Goals, the V&V hierarchy, and execution state. This table summarizes what the individual files under [`goals/`](README.md) already say in full — if this table and an individual file disagree, the individual file is authoritative and this table is stale; fix the table.

## EPIC-P0 (Kernel Skeleton) — fully decomposed

| Goal | Epic | Feature | Story | Test | Report | Status |
|---|---|---|---|---|---|---|
| G-RT-7, G-HW-4 (partial) | `EPIC-P0` | `FEAT-P0-01` | `STORY-P0-01-01` | `TEST-P0-01-01-A` | `REPORT-2026-07-26-01` | Verified (local; CI pending) |
| G-DX-5, G-DX-6 | `EPIC-P0` | `FEAT-P0-01` | `STORY-P0-01-02` | `TEST-P0-01-02-A` | `REPORT-2026-07-26-03` | Verified (local; CI pending) |
| G-DX-3 | `EPIC-P0` | `FEAT-P0-01` | `STORY-P0-01-03` | `TEST-P0-01-03-A` | `REPORT-2026-07-26-02` | Verified (local; CI pending) |
| G-RT-1 | `EPIC-P0` | `FEAT-P0-02` | `STORY-P0-02-01` | (5 `#[test]` fns in `sched.rs`) | `REPORT-2026-07-26-05` | Verified (local) |
| G-RT-1 | `EPIC-P0` | `FEAT-P0-02` | `STORY-P0-02-02` | `TEST-P0-02-02-A` | `REPORT-2026-07-26-08` | Verified (local) |
| G-RT-1 | `EPIC-P0` | `FEAT-P0-02` | `STORY-P0-02-03` | `TEST-P0-02-03-A` | `REPORT-2026-07-26-11` | Verified (local) |
| G-RT-1 | `EPIC-P0` | `FEAT-P0-02` | `STORY-P0-02-04` | `TEST-P0-02-04-A` | `REPORT-2026-07-26-12` | Verified (local; structural gaps noted) |
| G-RT-1 | `EPIC-P0` | `FEAT-P0-02` | `STORY-P0-02-05` | `TEST-P0-02-05-A` | `REPORT-2026-07-26-13` | Verified (local) |
| G-RT-2 | `EPIC-P0` | `FEAT-P0-03` | `STORY-P0-03-01` | `TEST-P0-03-01-A` | `REPORT-2026-07-26-04` | Verified (local) |
| G-RT-2 | `EPIC-P0` | `FEAT-P0-03` | `STORY-P0-03-02` | `TEST-P0-03-02-A` | `REPORT-2026-07-26-14` | Verified (local) |
| G-RT-2 | `EPIC-P0` | `FEAT-P0-03` | `STORY-P0-03-03` | `TEST-P0-03-03-A` | `REPORT-2026-07-26-04` | Verified (local) |
| G-HW-4 | `EPIC-P0` | `FEAT-P0-04` | `STORY-P0-04-01` | `TEST-P0-04-01-A` | `REPORT-2026-07-26-06` | Verified (local; CI pending) |
| G-HW-4 | `EPIC-P0` | `FEAT-P0-04` | `STORY-P0-04-02` | — | — | Planned, not yet started |
| G-HW-4 | `EPIC-P0` | `FEAT-P0-04` | `STORY-P0-04-03` | — | — | Planned, not yet started |
| G-PC-1, G-PC-4 | `EPIC-P0` | `FEAT-P0-05` | `STORY-P0-05-01` | `TEST-P0-05-01-A` | `REPORT-2026-07-26-07` | Verified (local) |
| G-PC-1, G-RT-2 | `EPIC-P0` | `FEAT-P0-05` | `STORY-P0-05-02` | `TEST-P0-05-02-A` | `REPORT-2026-07-26-09` | Verified (local) |
| G-PC-2, G-PC-3, G-AI-3 | `EPIC-P0` | `FEAT-P0-05` | `STORY-P0-05-03` | `TEST-P0-05-03-A` | `REPORT-2026-07-26-10` | Verified (local) |
| G-PC-1..4 | `EPIC-P0` | `FEAT-P0-05` | `STORY-P0-05-04` | `TEST-P0-05-04-A` | — | Planned, not yet started (Test specified) |
| G-PA-6, G-AI-3 | `EPIC-P0` | `FEAT-P0-06` | `STORY-P0-06-01` | `TEST-P0-06-01-A` | `REPORT-2026-07-26-15` | Verified (local) |
| G-PA-6, G-AI-3 | `EPIC-P0` | `FEAT-P0-06` | `STORY-P0-06-02` | `TEST-P0-06-02-A` | `REPORT-2026-07-26-16` | Verified (local) |
| G-PC-3, G-AI-3 | `EPIC-P0` | `FEAT-P0-07` | `STORY-P0-07-01` | — | — | Planned, not yet started |
| G-PC-3, G-RT-2 | `EPIC-P0` | `FEAT-P0-07` | `STORY-P0-07-02` | — | — | Planned, not yet started |

## EPIC-P1 through EPIC-P8 — backlog, not yet decomposed

See [`goals/epics/backlog.md`](epics/backlog.md) for the full Goal-to-Epic mapping for the remaining Roadmap phases. No Features, Stories, or Tests exist for these yet.

## Cross-cutting performance catalogue

[`performance/catalogue.tsv`](performance/catalogue.tsv) specifies 625 performance guardrails as the complete cross-product of 25 OS domains and 25 measurement/safety guardrails. They serve G-DX-7 and the latency, determinism, frugality, isolation, driver, inference, security, and compactness goals across every Roadmap phase.

These contracts are not counted as Story-level `TEST-*` artifacts and do not change any verification status in the matrix above. Every row begins `specified`; a row becomes reportable only when its owning subsystem has an implementing Story and evidence from the tier named in the catalogue. See [`performance/README.md`](performance/README.md) for the execution and comparison rules and [`performance/current-state-review.md`](performance/current-state-review.md) for the current audit.

## Session cross-reference

| Session handover | What it changed in `goals/` |
|---|---|
| [`session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md`](../session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md) | Created the entire V&V model: this folder, `EPIC-P0` fully decomposed, `EPIC-P1`–`EPIC-P8` stubbed in `epics/backlog.md`. |
| [`session/hand-2026-07-26/07-phase-0-walking-skeleton-implementation-handover.md`](../session/hand-2026-07-26/07-phase-0-walking-skeleton-implementation-handover.md) | `FEAT-P0-01` implemented and Verified locally (all three Stories/Tests): Cargo workspace bootstrap, `kernel` boots to halt under QEMU x86_64 via Xen PVH, `xtask qemu-x86_64` command, CI governance-gate workflow + fixture smoke test. Filed `REPORT-2026-07-26-01` through `-03`. |
| [`session/hand-2026-07-26/09-feat-p0-03-decomposition-and-pool-allocator.md`](../session/hand-2026-07-26/09-feat-p0-03-decomposition-and-pool-allocator.md) | Decomposed `FEAT-P0-02`/`-03`/`-04` into Stories (4/3/3). Implemented and Verified `STORY-P0-03-01`/`-03` (`Pool<T, N>` bounded-capacity allocator, `os/src/kernel/src/mem.rs`), filed `REPORT-2026-07-26-04`. `FEAT-P0-02`/`-04` Stories left unimplemented — they need QEMU-based Tier 0 verification, unavailable in this environment. |
| [`session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md`](../session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md) | QEMU became available in the working environment. Implemented and Verified `STORY-P0-02-01` (task creation/priority, `os/src/kernel/src/sched.rs`, host-testable, `REPORT-2026-07-26-05`) and `STORY-P0-04-01` (ACPI table parsing → `hal::topology::Topology`, `os/src/hal/`, `os/src/hal-x86_64/`, `os/src/kernel/src/{boot,main}.rs`, Tier 0 QEMU-verified, `REPORT-2026-07-26-06`), in parallel via one background agent (`STORY-P0-02-01`) and direct work (`STORY-P0-04-01`) to avoid file conflicts in a non-git-tracked working directory. |
| [`session/hand-2026-07-26/11-cover-note-feat-p0-05-sharc-blue-pe-loader.md`](../session/hand-2026-07-26/11-cover-note-feat-p0-05-sharc-blue-pe-loader.md) | Specified (not implemented) `FEAT-P0-05` (`G-DX-8`, `G-PC-1`..`G-PC-4`): decomposed into 4 Stories, all 4 Tests written test-first, traceability matrix and `index.html` updated. |
| [`session/hand-2026-07-26/12-story-p0-05-01-pe64-parser-implementation.md`](../session/hand-2026-07-26/12-story-p0-05-01-pe64-parser-implementation.md) | Implemented and Verified `STORY-P0-05-01` (PE64 image parsing → `pe::LoadDescriptor`, new `os/src/exec/` crate, host-testable, `REPORT-2026-07-26-07`). |
| [`session/hand-2026-07-26/13-story-p0-02-02-context-switch-implementation.md`](../session/hand-2026-07-26/13-story-p0-02-02-context-switch-implementation.md) | Implemented and Verified `STORY-P0-02-02` (x86_64 context switch, `os/src/kernel/src/context.rs` + `context_switch_fixture.rs`, Tier 0 QEMU-verified plus host unit tests, `REPORT-2026-07-26-08`) — unblocks `STORY-P0-05-02`. |
| [`session/hand-2026-07-26/14-story-p0-05-02-address-space-implementation.md`](../session/hand-2026-07-26/14-story-p0-05-02-address-space-implementation.md) | Implemented and Verified `STORY-P0-05-02` (process address-space creation and section mapping, new `hal_x86_64::paging` + `exec::address_space` modules, Tier 0 QEMU-verified via `exec`'s own `exec-fixture` binary plus host unit tests, `REPORT-2026-07-26-09`) — unblocks `STORY-P0-05-03`. |
| [`session/hand-2026-07-26/15-story-p0-05-03-win32-shim-implementation.md`](../session/hand-2026-07-26/15-story-p0-05-03-win32-shim-implementation.md) | Implemented and Verified `STORY-P0-05-03` (capability-scoped Win32 API compatibility shim, new `exec::win32_shim` module — closed allowlist, standalone `CapabilityPolicy` trait standing in for the not-yet-existing `aci` crate, proactive buffer bounds-checking against `AddressSpace`, Tier 0 QEMU-verified via `exec`'s new `win32-shim-fixture` binary plus host unit tests, `REPORT-2026-07-26-10`) — unblocks `STORY-P0-05-04`. |
| [`session/hand-2026-07-26/16-story-p0-02-03-priority-inheritance-implementation.md`](../session/hand-2026-07-26/16-story-p0-02-03-priority-inheritance-implementation.md) | Implemented and Verified `STORY-P0-02-03` (priority inheritance on lock contention, new `kernel::lock::PriorityInheritingLock` — boost-on-contention/restore-on-release bookkeeping, host-only per its own scope-resolution note since no ready-queue dispatcher exists yet to verify the behavioral half, `REPORT-2026-07-26-11`) — unblocks `STORY-P0-02-04`. |
| [`session/hand-2026-07-26/17-story-p0-02-04-wcet-enforcement-implementation.md`](../session/hand-2026-07-26/17-story-p0-02-04-wcet-enforcement-implementation.md) | Implemented and Verified `STORY-P0-02-04` (WCET budget enforcement, new `kernel::wcet::record_tick` — synchronous overrun detection via a standalone `OverrunHandler` trait standing in for the not-yet-built watchdog, host-only for the same missing-dispatcher/missing-watchdog reasons `STORY-P0-02-03` already surfaced, `REPORT-2026-07-26-12`) — named the missing ready-queue dispatcher as a twice-surfaced prerequisite. |
| [`session/hand-2026-07-26/18-story-p0-02-05-dispatch-loop-implementation.md`](../session/hand-2026-07-26/18-story-p0-02-05-dispatch-loop-implementation.md) | Added and implemented **`STORY-P0-02-05`** (new, not part of `FEAT-P0-02`'s original decomposition — priority-ordered cooperative dispatch loop, new `kernel::dispatch::run_once` + `Scheduler::highest_priority_ready`/`iter_tasks`/`TaskState`, `Pool::iter_occupied`/`PoolHandle::index`), closing `STORY-P0-02-03`'s own behavioral gap with a real `context::switch`-based dispatch round (`REPORT-2026-07-26-13`) — `FEAT-P0-02` now 5/5 Stories Verified, one structural gap remaining (`STORY-P0-02-04`'s: no timer, no watchdog). |
| [`session/hand-2026-07-26/19-story-p0-03-02-capacity-configuration-implementation.md`](../session/hand-2026-07-26/19-story-p0-03-02-capacity-configuration-implementation.md) | Implemented and Verified `STORY-P0-03-02` (compile-time pool-size configuration, new `kernel::capacities` module consolidating `MAX_CPUS`/`EXEC_FRAME_POOL_CAPACITY` and a `const`-evaluated `STATIC_MEMORY_BUDGET_BYTES` check, proven to fail a build via a new `fixture-capacity-budget` governance fixture, `REPORT-2026-07-26-14`) — completes `FEAT-P0-03` (3/3 Stories Verified). |
| [`session/hand-2026-07-26/20-strategic-objectives-spoor-ipc-blue-sharc-correction.md`](../session/hand-2026-07-26/20-strategic-objectives-spoor-ipc-blue-sharc-correction.md) | New strategic direction from the user: spoor (universal audit atom, borrowed from `Sharc.Blue`), local IPC, and `blue.atom`/`blue-sharc.exe`-driven deploy tooling for Raspberry Pi 5. Corrected `FEAT-P0-05.md`/`STORY-P0-05-04.md`'s inaccurate "`blue-sharc.exe` is an Ollama-like inference runtime" framing (it's an MCP sidecar/context engine; removed the erroneous `G-AI-1` claim). Added and implemented **`FEAT-P0-06`** (spoor) — `STORY-P0-06-01` (new `kernel::spoor` module: packed `Spoor` type, `Category`/`Actor`/`Action`/`Outcome` enums, `REPORT-2026-07-26-15`) Verified; `STORY-P0-06-02` (journal writer) specified, not yet implemented. Added (specified, not implemented) **`FEAT-P0-07`** (local IPC: message channels + shared-memory grants, explicitly no loopback TCP, mirroring `Sharc.Blue`'s own retired-loopback-TCP doctrine) with two Stories. Recorded a real network-facing TCP/IP stack as a separate, later `EPIC-P3` concern in `goals/epics/backlog.md`, not part of `EPIC-P0`. |
| [`session/hand-2026-07-26/21-story-p0-06-02-spoor-journal-implementation.md`](../session/hand-2026-07-26/21-story-p0-06-02-spoor-journal-implementation.md) | Implemented and Verified `STORY-P0-06-02` (append-only spoor journal, new `kernel::spoor_journal::SpoorJournal<N>` fixed-capacity ring buffer storing each entry as its raw `Spoor::to_bits()` value, plus `JOURNAL_MAGIC` matching `Sharc.Blue`'s own on-disk format exactly, `REPORT-2026-07-26-16`) — completes `FEAT-P0-06` (2/2 Stories Verified; one follow-up, wiring a real subsystem to emit spoors, tracked separately per that Feature's own exit criteria). |
| [`session/hand-2026-07-26/22-performance-catalogue-625-guardrails.md`](../session/hand-2026-07-26/22-performance-catalogue-625-guardrails.md) | Added the cross-cutting 625-test performance catalogue, current-state audit, measurement/claim rules, `xtask` integrity validator with five tests, CI gate, and dashboard links. All catalogue rows remain Specified until their owning subsystem and tier evidence exist. |

## How to keep this in sync

1. Adding a new Story or Test: add a row here (or extend an existing Feature's rows) in the same PR.
2. A Test's Status changes (e.g. first pass, or a regression): update the Status column and add the Report row in the "Session cross-reference" table if a handover discusses it, in the same PR that adds the Report file under `goals/reports/`.
3. Decomposing a backlog Epic into Features/Stories: move it out of the "not yet decomposed" section into its own fully-populated block, matching the `EPIC-P0` pattern above.
