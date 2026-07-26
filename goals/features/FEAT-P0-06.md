# FEAT-P0-06 — Spoor: Universal 64-Bit Audit Atom

Status: **Verified — 2/2 Stories Verified** (see Exit Criteria for the one still-open follow-up)
Epic: [`EPIC-P0`](../epics/EPIC-P0.md)
Introduced in: (this Feature — 2026-07-26, per a new strategic objective directing that TinyOS's audit primitive be universal, borrowed from `C:\Code\Sharc.Workspace\Sharc.Blue`)

## Description

A **spoor** is a 64-bit, fixed-size, hierarchically bit-packed audit/action record — not a log line, not a string, not an event stream. `Sharc.Blue`'s own doctrine (`Sharc.Bluekind\Blue.Reef\src\shape\spoor.rs`, `docs/ThePlan/Spoor.md`): "What iblock did for code, spoor does for action... stamp immediately at execution point," never buffered or batched, never carrying a `String` field, bit layout frozen once shipped. TinyOS adopts the same shape and the same discipline, for the same reason it exists in `Sharc.Blue`: a bounded-cost, no-heap, no-allocation record of *what happened* at every action boundary, cheap enough to stamp on every scheduler decision, lock contention, IPC call, or exec-shim invocation without threatening real-time guarantees — and precise enough to reconstruct "what the animal was doing" after a crash, an e-stop, or a security incident.

This is directly load-bearing for TinyOS's own goals, not an import for its own sake: **G-PA-6** (physical process auditability — reconstruct what happened after the fact) and **G-AI-3** (identical audit trail for every caller type, human or AI) both need exactly this kind of cheap, structural, always-on audit record, and need it available before the subsystems that will emit it (the scheduler, IPC, the ACI policy engine) are built — the same "introduce foundational infrastructure in Phase 0 so nothing downstream has to retrofit it" reasoning that put `FEAT-P0-03`'s pool allocator this early.

### Bit layout (frozen, mirrors `Sharc.Blue`'s Rust/C# dual implementation exactly)

```text
 63    59    55    51    47          31              0
  [ CAT ][ WHO ][ ACT ][ OUT ][  TARGET  ][   COST     ]
  4 bits 4 bits 4 bits 4 bits  16 bits     32 bits
```

- `CAT` (4 bits) — category ("animal family": e.g. scheduling, lock, IPC, exec, journal, sensor).
- `WHO` (4 bits) — actor.
- `ACT` (4 bits) — verb (what action).
- `OUT` (4 bits) — outcome (ok / empty / chose / capped / failed / skipped / superseded / partial — `Sharc.Blue`'s own outcome vocabulary, reused verbatim so a spoor reader doesn't need a TinyOS-specific outcome table).
- `TARGET` (16 bits) — a hash/id of what was acted on.
- `COST` (32 bits) — elapsed time (µs) or a payload value.

Adopting the identical layout (not just "a similar idea") is deliberate: it means a spoor journal produced by TinyOS is byte-for-byte structurally compatible with `Sharc.Blue`'s own tooling's expectations, and any future cross-project tooling (e.g. a shared spoor viewer) needs no format-translation layer.

## Crate(s) involved

`os/src/kernel/` (new `spoor` module) — `no_std`, `#![forbid(unsafe_code)]` where possible (the packed bit-manipulation itself needs no `unsafe`; only a future journal-writer touching raw memory-mapped storage might).

## Depends on

`FEAT-P0-01` (a booting kernel to run inside) only, for the core type itself — it has no dependency on the scheduler, IPC, or any other Phase 0 subsystem, since a `Spoor` is a value type any of them can construct and pass to a journal writer once one exists.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P0-06-01`](../stories/STORY-P0-06-01.md) | Core packed `Spoor` type: category/actor/action/outcome/target/cost encoding, `stamp`/`complete` constructors | Verified |
| [`STORY-P0-06-02`](../stories/STORY-P0-06-02.md) | Append-only spoor journal writer (fixed-capacity, no heap ring buffer) | Verified |

`STORY-P0-06-01` implemented and Verified in [`session/hand-2026-07-26/20-strategic-objectives-spoor-ipc-blue-sharc-correction.md`](../../session/hand-2026-07-26/20-strategic-objectives-spoor-ipc-blue-sharc-correction.md) — a new `kernel::spoor` module (`REPORT-2026-07-26-15`). `STORY-P0-06-02` implemented and Verified in [`session/hand-2026-07-26/21-story-p0-06-02-spoor-journal-implementation.md`](../../session/hand-2026-07-26/21-story-p0-06-02-spoor-journal-implementation.md) — a new `kernel::spoor_journal::SpoorJournal<N>` ring buffer, storing each entry as its raw `Spoor::to_bits()` value alongside a `JOURNAL_MAGIC` matching `Sharc.Blue`'s own on-disk format exactly (`REPORT-2026-07-26-16`).

Query/replay/summarize tooling (`Sharc.Blue`'s `spoor.query`/`spoor.watch`/`spoor.replay`/`spoor.summarize` atoms) is deliberately out of scope for Phase 0 — TinyOS has no shell/tooling layer yet to host such commands (that's `EPIC-P2`, Phase 2 — Shell & UX). This Feature's job is only to make spoor stamping and journaling *available* to every other Phase 0 subsystem; consuming/querying the journal is later work.

## Exit criteria

- `STORY-P0-06-01` and `-02` both reach **Verified**.
- At least one other Phase 0 subsystem (a natural candidate: `kernel::lock`'s contention/boost events, or `kernel::wcet`'s overrun detection) is updated to actually emit spoors through this Feature's API, proving it's genuinely usable, not just a type that compiles — tracked as a follow-up Story once `-01`/`-02` land, not blocking this Feature's own Verified status (mirroring `STORY-P0-03-02`'s own "don't add a capacity with nothing consuming it" discipline, applied here as "don't claim adoption before a real caller exists").
