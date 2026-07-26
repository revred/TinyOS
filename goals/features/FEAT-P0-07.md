# FEAT-P0-07 — Local Inter-Process Communication (Shared Memory & Message Channels)

Status: **Not yet started — specified this session, per a new strategic objective**
Epic: [`EPIC-P0`](../epics/EPIC-P0.md)
Introduced in: (this Feature — 2026-07-26, per a new strategic objective naming "In-Process Communication, sharing memory between processes, socket communication, TCP/IP stack" as a priority)

## Description

Same-machine, same-kernel-instance IPC between two TinyOS tasks: a shared-memory region primitive and a message-passing channel ("socket communication" in the local sense — a bounded, capability-scoped channel between two tasks, not a network socket). `docs/mvp-delivery-strategy.md`'s own crate map already names IPC as in-scope for the `kernel` crate in Phase 0 ("Scheduler, IPC, memory pools, deadline monitor") — this Feature is that crate-map line item, decomposed.

**Scope boundary, decided deliberately (2026-07-26):** this Feature is local IPC only — shared memory and a bounded message channel between tasks on the same running TinyOS instance. A real network-facing TCP/IP stack (for UAV telemetry, edge-device connectivity, data-center networking) is a **separate, later** Feature, not part of this one — see "Relationship to a real TCP/IP stack" below for why, and `goals/epics/backlog.md` for where that later work is tracked.

### Doctrine: no loopback TCP for local IPC

Researched from the sibling `Sharc.Blue` project (`Sharc.Bluekind/Blue.Sharc/Cargo.toml`'s own dependency-justification comment): that project explicitly **retired loopback TCP as an IPC transport** ("S74 W7 forbade loopback TCP attack-surface") in favor of named pipes (Windows) / Unix domain sockets (Linux/macOS) — a narrow, same-user-SID-scoped, no-ambient-network-surface transport — keeping exactly one documented exception (a localhost-TCP bridge to a C# worker subprocess, scoped specifically to that one dispatch path, not general-purpose). TinyOS adopts the identical doctrine for the same reason: a loopback TCP listener is discoverable/connectable surface a local, no-network IPC path has no business exposing, and this project's own security posture ("unhackable by design," every caller capability-scoped, `G-PC-3`/`G-AI-3`) is stricter than `Sharc.Blue`'s already-strict baseline, not looser. Concretely: TinyOS's local IPC primitive is a bounded, capability-scoped channel/shared-memory handle exchanged between tasks the scheduler already knows about — never a socket bound to any address, loopback included.

### Relationship to a real TCP/IP stack

A TCP/IP stack is a real, separate, larger piece of infrastructure: a NIC class driver (`G-HW-2`, Roadmap Phase 3 — Connectivity, `EPIC-P3`), a protocol implementation above it, and its own capability/audit story for network-facing traffic (unlike local IPC, network traffic crosses a trust boundary the ACI model has to mediate explicitly). It belongs with `EPIC-P3`'s eventual decomposition, once that Epic is picked up — not bolted onto Phase 0's kernel skeleton. See `goals/epics/backlog.md`'s `EPIC-P3` row for the tracking placeholder.

## Crate(s) involved

`os/src/kernel/` (new `ipc` module).

## Depends on

`FEAT-P0-02` (a task to be an IPC endpoint), `FEAT-P0-03` (the `Pool<T, N>` allocator this Feature's channel/shared-memory-handle bookkeeping will reuse).

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P0-07-01`](../stories/STORY-P0-07-01.md) | Bounded, capability-scoped message channel between two tasks | Not yet started |
| [`STORY-P0-07-02`](../stories/STORY-P0-07-02.md) | Shared-memory region handle exchange between two tasks | Not yet started |

## Exit criteria

- `STORY-P0-07-01` and `-02` both reach **Verified**.
- Neither Story introduces a network-addressable socket of any kind (loopback included) — a review checklist item for both Stories' PRs, not just an aspiration in this doc.
