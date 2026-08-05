# SharCrust on the Board — the Real-Time Proving Ground, and What It May Never Do

Status: **Design note, 2026-08-05, revised the same day after reading `internal/Sharc.Probe`.
No code, no Feature, no Story.** Written so the scope is settled before anything is built, and
deliberately stopping short of decomposition — `agent.md`'s just-in-time rule cuts against
specifying an application tier while the board has not yet dispatched a task.

Prerequisite: **`FEAT-P1-11` must be board-proven first.** One dispatch round, from the park
loop, with interrupts live. This document describes the first thing to run *after* that.

**Revision note.** The first version of this document proposed a new fixed-slot hash table
built from scratch. That was written without having read `internal/Sharc.Probe`. The owner's
direction is to make **SharCrust — the homegrown Rust SQLite-format engine, stripped of
external dependencies — the proving ground for the RT layer**, and that is a better target:
feasibility proven against a stand-in proves the stand-in, while feasibility proven against
the engine that will actually run proves the thing.

---

## 1. Why a real storage engine is the right feasibility probe

Every timing number this project holds comes from `fixture_measure`: phases run by a harness,
single core, interrupts masked inside the measured region, nothing contending. Those measure
**the mechanism**, and they cannot fail in an interesting way.

A storage engine with bounded operations, running under a live scheduler, measures **the
system** — and it can fail. If p99.9 on a page read blows out once dispatch is real, that is a
finding about TinyOS. `EPIC-P1` is *Determinism Proof*, and a proof needs something that could
have come out the other way.

**And the SQLite file format is a surprisingly good real-time target**, which is not obvious
until stated: a B-tree lookup is `O(depth)` with a known page size, so bounding the database
size bounds the depth, which bounds the read. Reads are structurally boundable. Writes with
page splits are not, which is why §4 excludes them from the first stage rather than hoping.

## 2. What can come to the board, in what order

`internal/Sharc.Probe/sharcrust/` is layered, and the layers have very different distances to
travel. Measured, not guessed — imports per module today:

```
format.rs      external=0  std=0     ← already clean
records.rs     external=0  std=0     ← already clean
schema.rs      external=0  std=1
error.rs       external=0  std=1
primitives.rs  external=2  std=0     ← uuid + rust_decimal, for GUID/Decimal codecs
```

**Two external imports and two `std` imports** stand between the format layer and something
that compiles `no_std` with no allocator. That is the whole distance for stage 1.

| Stage | What moves | Distance | Bounded? |
|---|---|---|---|
| **1. Format core** | varints, serial types, record codec, header parse | Replace `uuid`/`rust_decimal` with internal codecs; drop 2 `std` imports | Yes — pure functions over `&[u8]`, no state |
| **2. Fixed-pool pager + read cursor** | `pager`, `btree` read path | Replace a growable page cache with `N` compile-time page buffers | Yes — `O(depth)`, depth bounded by database size |
| **3. Measurement under the scheduler** | the RT feasibility answer | `measure_phases` entries, board capture | This is the deliverable |
| **Not staged** | `btree_writer`, `overflow`, `freelist`, `crypto`, `intelligence`, `scanner` | — | No — see §4 |

**Stage 1 is testable entirely on the host** and is where the "no external dependencies" work
actually lands. Stage 2 is the first thing that needs the board. Stage 3 is the point of the
exercise.

## 3. What it may do on the board

Each operation bounded by a compile-time constant, each stamping a spoor.

| Operation | Bound | Notes |
|---|---|---|
| Decode a record | Bytes in the cell | Pure function over a slice; no state, no allocation. |
| Read a page | One fixed buffer | From a pool of `N`; a pool miss with all buffers pinned **refuses**. |
| B-tree point lookup | `≤ MAX_DEPTH` page reads | Depth bounded by a compile-time database-size ceiling, refused past it. |
| Cursor step with budget | Exactly `budget` cells | Caller states the budget and gets a cursor back. **No unbounded scan.** |

**Every refusal is a spoor, not a silence.** A depth overrun, an exhausted page pool, a
truncated cell — each is a recorded outcome (`Capped`, `Failed`) with its reason on the wire.
A store that quietly degrades lies about the run that mattered, which is the rule the spoor
ring already holds itself to.

## 4. What it may never do

Exclusions, not deferrals. Each one, if admitted, breaks a guarantee the project currently
machine-enforces.

- **No allocation, of any kind.** The assurance spine forbids `#[global_allocator]`,
  `extern crate alloc` **and `use alloc::`** anywhere in the image, and withdraws `G11` loudly
  if any appears. So no `Vec`, no `Box`, no `String` — which is precisely why stage 1 is a
  dependency-strip and not a straight vendoring.
- **No write path in the first stage.** `btree_writer` does INSERT with page split, and a page
  split is unbounded work wearing an amortised disguise. `sharcrust/specs/write-ops-roadmap.md`
  is honest that overflow-write, freelist management, UPDATE, DELETE and rebalance are
  specified and not built; none of them may reach the board before its bound is stated.
- **No unbounded operation whatsoever** — no full scans, no joins, no sorts, no aggregates, no
  cursor that outlives its call. Every entry point carries a budget or a constant.
- **No blocking, no I/O, no waiting on a device.** Fail-safe over keep-trying.
- **No query planner.** A planner is a variable-latency component by construction. Callers name
  pages and keys, not intentions.
- **No persistence, and not because it is hard.** TinyOS cannot read the SD card or NVMe on
  this board at all. The database is a byte range in RAM, and its contents die with the boot.
  **That is a scope statement, not an apology**, and no durability claim may be made.
- **No parsing of a database file from off-board.** A `.arc` arriving over the wire is external
  bytes and makes this a hostile-input surface under `PD-12`/`BND-03` — with a *file-format
  parser* as the attack surface, which is the single most CVE-prone shape in this whole design.
  In-image byte ranges only, until that Feature exists with its own adversarial tests.
- **No crypto in the first stages.** `aes-gcm`, `pbkdf2` and `ed25519-dalek` are external
  dependencies and, more importantly, unbounded-ish work with no stated WCET here.

## 5. Where it runs, stated honestly

**At EL1, in the kernel's own protection domain.** There is no `EL0` on this path — the
exception-level module treats it as *"the impossible entry"* — and there are no per-task
address spaces. So this is **not** a contained `C3` application and **no containment evidence
may be claimed for it.** It is a workload that proves the scheduler, not an isolation
demonstration. A legitimate first step and an illegitimate final one.

## 6. The licence question, which blocks stage 1

`internal/` is git-ignored, correctly and for the same reason `external/npcap188/` is: a
proprietary tree inside an MIT repository survives untracked exactly until someone types
`git add`. Vendoring SharCrust's format core into `os/src/` **moves code across that line into
a public MIT image.**

And the two statements about that code do not currently agree:

```
sharcrust/Cargo.toml   license = "MIT"
Sharc.Probe/README.md  "Proprietary and confidential. Not licensed for redistribution."
```

Both cannot hold for code vendored into TinyOS. The owner holds both copyrights so this is
settleable by decision rather than negotiation — but it must be settled **explicitly and in
writing before a line moves**, and the resolution belongs in `external/README.md`'s sibling
for `internal/`. This is the same discipline that caught the Npcap boundary before a
`git add external/` could have staged a source-available tree into an MIT repository.

## 7. The measurement contract

Not finished when it works — finished when its cost is **stated**, in the same envelope and by
the same discipline as everything else:

- `sharc_record_decode`, `sharc_page_read`, `sharc_btree_lookup_at_depth_bound`,
  `sharc_cursor_step_of_N` — each `n=1000 warmup=100` through `measure_phases`, landing beside
  the spoor costs in the `TOS64-MEAS/2` envelope.
- **Batched where the operation is small**, per `LE-24`: the batched twin is the quotable
  figure and the unbatched one is residue-contaminated. Applied *before* the first number is
  quoted rather than after, which is the lesson that row cost this project a Report.
- **The depth-bound lookup is the WCET case** and is the number that matters. An average
  lookup says nothing about whether a deadline holds.
- Until those exist off the wire, no determinism claim may be made for this layer.

## 8. What this document does not decide

- **Any Feature or Story decomposition.** This is a scope note so the boundary is settled
  before code exists — not a plan, and explicitly not a commitment to build.
- **The host-side archive's contents.** Its *boundary* is decided: **a file, not a Cargo
  dependency.** TinyOS emits, Sharc.Probe ingests. A clean clone has no `internal/`, so any
  `xtask → sharcrust` path dependency fails on the runner — and `CONTEXT.md` flags the sharper
  version itself, warning to cut or vendor SharCrust's `kit-step` dev-dependency *"before
  building SharCrust inside any other repository's CI."*
- **The archive is still gated behind `LE-76`**: ingesting today's text transcript would bake
  in records with no sequence, no epoch and invisible loss. Once spoor records carry the
  envelope, a `.arc` archive is readable by `probe_cli`, PySharc and **`sharc_mcp`** — whose
  `hot_epoch_diff` and `hot_time_travel` already do, for datasets, exactly what the board's
  boot epoch invites for runs.
