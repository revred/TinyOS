# TinyDB — an Ultra-Fast Store Beside the Real-Time Kernel, and What It May Never Do

Status: **Design note, 2026-08-05. No code, no Feature, no Story.** Written at the owner's
request so the scope is settled before anything is built, and deliberately stopping short of
decomposition — `agent.md`'s just-in-time rule cuts against specifying an application tier
while the board has not yet dispatched a task.

Prerequisite: **the kernel must run on silicon first.** See `session/hand-2026-08-05/04A` §3.
This document describes the first thing to run *after* that, and is written now only because
the design constraints bear on how the dispatch work is shaped.

---

## 1. Why a database is the right first application

Every timing number this project holds comes from `fixture_measure`: phases run by a harness,
single core, interrupts masked inside the measured region, nothing contending for anything.
Those measure **the mechanism**. They cannot fail in an interesting way.

A store with deterministic operations, running under a live scheduler, measures **the system**
— and it can fail. If p99.9 on a point read blows out once dispatch is real, that is a finding
about TinyOS rather than about a benchmark. `EPIC-P1` is *Determinism Proof*, and a proof needs
something that could have come out the other way.

It is also the right shape for a first application because it exercises exactly the subsystems
that have never executed on hardware: tasks, dispatch, priority-inheriting locks, WCET budgets,
and fixed-pool memory. **"Run the kernel on the board" and "run TinyDB as the first service" are
the same work approached from opposite ends** — this end supplies a reason and a falsifiable
target instead of a hello-world.

## 2. The shape: a table, not an engine

**Open-addressed, fixed-capacity, allocation-free.** Compile-time `CAPACITY`, compile-time
`KEY_BYTES` and `VALUE_BYTES`, one contiguous array of slots. No tree, no page cache, no
planner, no journal.

This is the same shape the project has already converged on twice — `SpoorJournal`'s
fixed-slot append-only ring, and the `.rac`-style mmap substrate earmarked for Phase 6 model
loading. Convergence from three directions is usually a sign the shape is real rather than
convenient.

**A probe budget is the central design decision.** Linear probing with a hard
`MAX_PROBE` constant: a lookup that has not resolved within `MAX_PROBE` slots **refuses**. It
does not probe further, does not rehash, does not grow. That single rule is what converts a
hash table from "amortised O(1)" — a statement about averages that says nothing about the run
that mattered — into a bounded operation with a stated worst case.

## 3. What TinyDB may do

Each operation below is O(1) with a worst case bounded by a compile-time constant, and each
stamps a spoor.

| Operation | Bound | Notes |
|---|---|---|
| `get(key)` | ≤ `MAX_PROBE` slot reads | Returns a value copy; no reference escapes the table. |
| `put(key, value)` | ≤ `MAX_PROBE` slot reads + 1 write | Refuses past the load-factor bound. |
| `delete(key)` | ≤ `MAX_PROBE` + 1 write | Tombstone; slot reused by the next `put`. |
| `scan(cursor, budget)` | exactly `budget` slots | Caller states the budget; the call returns a cursor to resume. **There is no unbounded scan.** |
| `len()`, `capacity()`, `load()` | O(1) | Counters, not traversals. |

**Every refusal is a spoor, not a silence.** A `put` past the load bound, a `get` that exhausts
its probe budget, a `scan` that hits its budget mid-table — each is a recorded outcome
(`Capped`, `Failed`) with the reason on the wire. A store that quietly degrades is a store that
lies about the run that mattered, which is the same rule the spoor ring already holds itself to.

## 4. What TinyDB may never do

These are exclusions, not deferrals. Each one, if admitted, would break a guarantee the project
currently machine-enforces.

- **No allocation, of any kind.** Not merely no heap: the assurance spine forbids
  `#[global_allocator]`, `extern crate alloc` **and `use alloc::`** anywhere in the image, and
  withdraws the `G11` evidence loudly if it appears. So no `Vec`, no `Box`, no `BTreeMap`, no
  `String`. If the existing implementation is `std`- or `alloc`-based, **that port is the real
  cost of this work and should be scoped before anything is committed to.**
- **No growth and no rehash.** Capacity is a compile-time constant. A full table refuses.
  Rehashing is an unbounded operation wearing an amortised disguise, and there is no point in a
  real-time system at which "usually fast" is the property being sold.
- **No unbounded operation whatsoever** — no full scans, no joins, no sorts, no aggregates, no
  iterators that outlive a call. Every entry point takes its own budget or has a constant one.
- **No blocking.** No waits, no retries against a deadline, no I/O. Fail-safe over keep-trying.
- **No query language and no planner.** A planner is a variable-latency component by
  construction. Callers name slots, not intentions.
- **No persistence, and not because it is hard.** TinyOS cannot read the SD card or NVMe on
  this board at all; `RamVolume` lives in `shell` and is Tier 0, never compiled for the target.
  TinyDB is RAM-backed and its contents die with the boot. **Saying so is a scope statement, not
  an apology** — and it means no durability claim may be made for it in any Report.
- **No parsing of external bytes.** Keys and values arrive from in-image callers (`C1`). The
  moment a key arrives from off-board, this becomes a hostile-input surface under `PD-12` and
  `BND-03` and needs its own contracts. It does not have them.
- **No randomised hashing.** A per-boot hash seed would make the probe count — and therefore
  the WCET — vary between boots, which defeats the whole point. The consequence is that
  **TinyDB is vulnerable to hash flooding by construction**, and that is acceptable *only*
  while every key originates in-image. This is the exclusion most likely to be forgotten when
  someone later wants to key on a network-supplied identifier, so it is stated loudest.

## 5. Where it runs, stated honestly

**At EL1, in the kernel's own protection domain, initially.** There is no `EL0` on this path —
the exception-level module treats it as "the impossible entry" — and there are no per-task
address spaces. So the first TinyDB is *not* a contained `C3` application, and **no containment
evidence may be claimed for it.** It is a workload that proves the scheduler, not an isolation
demonstration.

That is a legitimate first step and an illegitimate final one. The staging:

1. **EL1, kernel domain** — the RT feasibility probe. Measurable, falsifiable, uncontained.
2. **EL0 with per-task address spaces** — when `EPIC-P1`'s isolation work lands, TinyDB becomes
   the first genuine `C3` subject, and the containment claims become testable rather than
   assumed.
3. **Persistence** — only after a block-device service exists with its own `BND-07` evidence.

## 6. The measurement contract

TinyDB is not finished when it works. It is finished when its cost is **stated**, in the same
envelope and by the same discipline as everything else:

- `tinydb_get_hit`, `tinydb_get_miss_at_probe_bound`, `tinydb_put_new`, `tinydb_put_replace`,
  `tinydb_delete`, `tinydb_scan_of_N` — each `n=1000 warmup=100`, through `measure_phases`,
  landing beside the spoor costs in the `TOS64-MEAS/2` envelope.
- **Batched where the operation is small**, per `LE-24`: the batched twin is the quotable
  figure and the unbatched one is residue-contaminated. That lesson was learned after the fact
  once; here it is applied before the first number is quoted.
- **The miss-at-probe-bound case is the WCET case** and is the number that matters. An average
  `get` tells you nothing about whether a deadline holds.
- Until those numbers exist off the wire, no claim about TinyDB's determinism may be made, and
  the Story says so.

## 7. What this document does not decide

- Whether the owner's existing Rust database can be ported under §4's constraints, or whether
  what lands is a new fixed-slot table sharing its semantics. **That is a scoping question to
  answer by reading the implementation, not by assertion here.**
- The host-side archive (a real database over spoors and evidence, living in `xtask`, derived
  from raw captures and rebuildable at any time). Different layer, different constraints,
  gated behind `LE-76`.
- Any Feature or Story decomposition. This is a scope note so that the boundary is settled
  before code exists — not a plan, and explicitly not a commitment to build.
