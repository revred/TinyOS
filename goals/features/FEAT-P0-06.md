# FEAT-P0-06 — Spoor: Universal 64-Bit Audit Atom

Status: **Verified — 4/4 Stories Verified** (exit criteria fully met; `STORY-P0-06-04` adopts spoor into a second subsystem beyond what the exit criteria required)
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
| [`STORY-P0-06-03`](../stories/STORY-P0-06-03.md) | Wire `kernel::lock` to emit spoors on boost/restore (this Feature's own exit-criteria follow-up) | Verified |
| [`STORY-P0-06-04`](../stories/STORY-P0-06-04.md) | Wire `kernel::wcet` to emit spoors on overrun/reset (second subsystem, beyond what the exit criteria required) | Verified |

`STORY-P0-06-01` implemented and Verified in [`session/hand-2026-07-26/20-strategic-objectives-spoor-ipc-blue-sharc-correction.md`](../../session/hand-2026-07-26/20-strategic-objectives-spoor-ipc-blue-sharc-correction.md) — a new `kernel::spoor` module (`REPORT-2026-07-26-15`). `STORY-P0-06-02` implemented and Verified in [`session/hand-2026-07-26/21-story-p0-06-02-spoor-journal-implementation.md`](../../session/hand-2026-07-26/21-story-p0-06-02-spoor-journal-implementation.md) — a new `kernel::spoor_journal::SpoorJournal<N>` ring buffer, storing each entry as its raw `Spoor::to_bits()` value alongside a `JOURNAL_MAGIC` matching `Sharc.Blue`'s own on-disk format exactly (`REPORT-2026-07-26-16`). **`STORY-P0-06-03`** (added mid-Feature, per this Feature's own named exit-criteria follow-up) implemented and Verified in [`session/hand-2026-07-26/23-story-p0-06-03-spoor-adoption-kernel-lock.md`](../../session/hand-2026-07-26/23-story-p0-06-03-spoor-adoption-kernel-lock.md) — `kernel::lock::PriorityInheritingLock::try_lock`/`unlock` (`STORY-P0-02-03`, `FEAT-P0-02`) each now stamp a `Spoor` into a caller-supplied `SpoorJournal<J>` on an actual priority boost/restore, proving the API is genuinely usable, not just a type that compiles (`REPORT-2026-07-26-17`). **`STORY-P0-06-04`** (added by explicit user request once this Feature's own exit criteria were already fully met) implemented and Verified in [`session/hand-2026-07-26/24-story-p0-06-04-spoor-adoption-kernel-wcet.md`](../../session/hand-2026-07-26/24-story-p0-06-04-spoor-adoption-kernel-wcet.md) — `kernel::wcet::record_tick`/`reset_budget_window` (`STORY-P0-02-04`, `FEAT-P0-02`) each now stamp a `Spoor` into a caller-supplied `SpoorJournal<J>` on an actual budget overrun/reset, adopting spoor into a second subsystem (`REPORT-2026-07-26-18`).

Query/replay/summarize tooling (`Sharc.Blue`'s `spoor.query`/`spoor.watch`/`spoor.replay`/`spoor.summarize` atoms) is deliberately out of scope for Phase 0 — TinyOS has no shell/tooling layer yet to host such commands (that's `EPIC-P2`, Phase 2 — Shell & UX). This Feature's job is only to make spoor stamping and journaling *available* to every other Phase 0 subsystem; consuming/querying the journal is later work.

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C1** · subjects **C0–C4** · boundary test **BND-17**.

That row also selects this Feature’s [`PD-*`](../security/protection-domain-contracts.tsv) and [`RCG-*`](../security/code-admission-gates.tsv) Security Charter obligations. Every Test repeats the exact selections and CI rejects drift.

A spoor observes a decision but never authorizes one. Every boundary record must identify source and target class, actor, action, object, decision, result, sequence, and relevant generation while remaining fixed-size and bounded. Required evidence covers allow, deny, fault, revoke, restart, promotion, concurrent ordering, wrap pressure, tamper detection, and reserved critical-event capacity.

## Exit criteria

- `STORY-P0-06-01` and `-02` both reach **Verified**. **Met.**
- At least one other Phase 0 subsystem is updated to actually emit spoors through this Feature's API, proving it's genuinely usable, not just a type that compiles. **Met** by `STORY-P0-06-03`: `kernel::lock::PriorityInheritingLock` now stamps `Category::Lock`/`Action::Boost`/`Action::Restore` spoors on real priority-mutating events.

**All four Stories now Verified — this Feature's exit criteria are fully met, with no open follow-up.** `STORY-P0-06-04` adopted spoor into `kernel::wcet` as well, beyond what the exit criteria required, per explicit user request. A production caller wiring a real `SpoorJournal<N>` into `main.rs` (and the capacity/footprint accounting that would then justify, per `kernel::capacities`' own "don't add a capacity with nothing consuming it" discipline) remains open — tracked as a consequence of `FEAT-P0-02`'s own still-open dispatcher/timer gap, not this Feature's.
