# Handover 39A — Adopting Sharc.Blue's Allocation Profiler: Scoping, and Why the Port Is a Pattern and Not a File

**A scoping document, not a Story and not an implementation.** No code was written. Nothing was
contracted. §7 lists five decisions that must be settled before anything is built, in
[Handover 31](31-qemu-virt-fixture-scoping.md)'s pattern, because 31's shape worked.

The instruction was to adopt Sharc.Blue's allocation profiler as self-analysis tooling on the real-time
hot paths. **The instruction is right and the target is better than it looks** — but the mechanism cannot
be copied, for a reason that is a gate in this repository rather than a matter of taste.

## 1. What Sharc.Blue actually has, read rather than summarised

Source: `C:\Code\Sharc.Workspace\Sharc.Blue\Sharc.Bluekind\Blue.Sharc\`.

- **`src/alloc_probe.rs`** — two `AtomicU64`s, `ALLOC_COUNT` and `ALLOC_BYTES`. Nineteen lines. **They
  are compiled into the production ship build and never written there**, because the allocator that
  feeds them is only installed by dev profiling binaries. The production drills that read them therefore
  read `0` and are free no-ops.
- **`CountingAlloc`** (in `src/region_path_bench.rs:42`) — a `GlobalAlloc` wrapper. `alloc` and `realloc`
  `fetch_add(1, Relaxed)` on the count and the size on the bytes, then delegate to `System`. **`dealloc`
  is deliberately not counted**: the metric is *allocation traffic*, not residency.
- **`src/crack_profile.rs`** — snapshots the two counters around named phases (`SCAN`, `PERSIST`),
  divides by the work unit (nodes), and emits **allocs/node and bytes/node per phase** as one line of
  JSON. It carries `CEIL_ALLOCS_PER_NODE`, and a phase over the ceiling sets `rc = 1`.
- Results are banked as versioned artifacts — `docs/coverage_matrix/ALLOC_BASELINE.md`,
  `crack_rocksdb_profile.{json,md}` — with the ceiling documented as *"aspirational target, rc=1 by
  design"*.

**What makes it strong is not the counter.** A counting `GlobalAlloc` is thirty lines that anyone can
write. Three things around it are the actual asset:

1. **Normalisation by work unit.** `allocs/node`, not `allocs`. That is what turned a number into a
   diagnosis: allocs/node **flat** at 130–156 across a 60× corpus range proved the persist path linear
   and healthy, while allocs/node climbing 115 → 992 over the same range exposed an **O(N²)
   whole-graph-rescan signature** that no total would have shown.
2. **A ceiling that fails loudly**, in the same run, with a non-zero exit code.
3. **Two disciplines learned the expensive way**, and both are stated in
   `docs/ThePlan/Agenda_CrackSpeed_10x_RocksDB_AllocProfiling_2026-07-16.md`:
   - **Allocation-count reduction is not wall reduction.** Measured: killing the O(N²) rescan cut allocs
     **−31% with zero wall change.** So *gate on wall; profile both.*
   - **A counting allocator's own milliseconds are an allocation proxy, never wall**, because the
     instrument taxes what it measures. Sharc records that trusting them "mis-steered the persist-
     parallelization dead end".

## 2. Why the file cannot be ported: a gate already forbids it

**`os/src/xtask/src/assurance.rs:1498`, `validate_no_heap`.** For each of the six `SHIPPED_CRATES` —
`hal`, `hal-arm64`, `hal-x86_64`, `exec`, `kernel`, `os` — it rejects any source line containing
`#[global_allocator]`, `extern crate alloc`, or `use alloc::` outside a `#[cfg(test)]` module. It is
asserted by the host test `the_shipped_crates_contain_no_heap`.

So `CountingAlloc` cannot be installed in a shipped TinyOS crate. **And there would be nothing for it to
count if it could**: the shipped crates have no heap by construction, which is the property the gate
exists to protect.

**This is not an obstacle to the instruction — it is the reason the instruction lands somewhere more
useful.** The thing worth measuring on a TinyOS real-time hot path was never heap traffic. It is
**bounded-resource claims**, and TinyOS has them:

- **`kernel::mem::Pool<T, N>::alloc` / `::free`** (`os/src/kernel/src/mem.rs:78` and `:128`) — the
  fixed-capacity pool. `D07` is *"Static pool allocation"*, a performance domain in its own right.
- Candidates beyond the pool, to be settled in §7: `exec::shared_memory::grant`, address-space mapping,
  IPC queue claims, and stack high-water.

**What transfers unchanged is the cleverest part**: two atomics compiled in and never written unless a
profiling build installs the producer, so the shipped hot path pays nothing. That design is about
zero-cost instrumentation, not about heaps, and it is exactly what a real-time kernel needs. `xtask` is
**not** in `SHIPPED_CRATES`, so it is also the one place a literal `GlobalAlloc` port is legal — see §7
decision 4.

## 3. The catalogue already ordered this instrument, twice, for all 25 domains

**This is the finding that changes the priority.** It is not a new capability needing a decision about
whether it belongs. It is the **missing instrument for 50 release-gated rows that already exist**:

| | Guardrail | Across |
| --- | --- | --- |
| **`G11`** | *"steady-state allocation count"* | all 25 domains |
| **`G12`** | *"allocation and reclamation latency"* | all 25 domains |

And `G11`'s target column already anticipates the TinyOS shape, verbatim:

> `metric: allocations_per_operation`
> `target: heap allocations per steady-state work unit = 0; pool claims are separately counted`

**`allocations_per_operation` is Sharc's `allocs/node`.** The catalogue independently specified the same
normalisation, and it already separates *heap must be zero* from *pool claims are counted*. All 50 rows
are `status: specified` and none has an instrument — as are all 625, so this is unbuilt work rather than
a defect, and **no loose end is registered for it.** What it means is that the tool has 50 waiting
customers rather than a hypothetical one.

## 4. The first real customer, and it is already open

**`LE-42`.** The `D09` accept path measures **17.6–39.1× over every latency and cycle budget its own
catalogue rows state**, and `PE64` parse cost had never been measured before `STORY-P0-01-06`. Its row
says to re-measure on a stable environment before concluding anything about the parser.

A per-work-unit allocation and pool-claim census on the `PE64` path is precisely the instrument that
distinguishes *the parser is alloc-bound* from *the parser is compute-bound* from *Tier 0 QEMU/TCG is
lying about both* — which is the question `LE-42` cannot currently answer. **`D09-G11` exists and is
specified.** This is a concrete first target, not a demonstration.

Sequencing note: `LE-42` waits on `LE-23` (re-record from a CI run), so the instrument can be built
before the environment is stable and used the moment it is.

## 5. Where this collides with `ADR 0005`, and it does

**A counting instrument perturbs the thing it measures.** Sharc learned this and wrote it down; this
repository has now derived the same rule from a fourth direction. Two consequences are binding rather
than advisory:

**No number this profiler produces may be promoted to a bound.** Under `ADR 0005` a worst-case bound is
quotable only from a qualified platform, and zero platforms are qualified. Independently, a profiling
build's timing is not the shipped build's timing, so **a profiled run must never be baselined** — it
would feed `check-timing-regression` a number describing the instrument. That is `LE-33`'s second
condition arriving from a new direction, and it is an argument for closing that row *before* this tool
produces numbers people want to quote.

**`G11` asks for a zero, and that is the trap this project has now derived three times.** A steady-state
allocation count of zero is *the cheapest result any counter can produce*, and it is indistinguishable
from a counter that is not wired up. `ADR 0005` §"The trap this ADR sets, named up front" — and its
2026-07-28 provenance addition, which records the `.org` guard padded past 128 bytes and the SIMD
detector self-tested on `v1.16b` and `fadd s0` — makes the rule explicit for `Q3`. **It applies here
without modification:**

> A `G11` zero is inadmissible unless the same counter has been shown, in the same Report, to count a
> deliberately planted allocation or pool claim.

That requirement is a **Red clause on the Test document**, and it is the single most important sentence
in this scoping. It is a fourth arrival of the positive-control rule. Per Handover 37's judgment the
citation habit is not worth a loose end and neither is this — it is worth being *in the Test*.

## 6. What this is not

Stated in Handover 31's style, because scope creep on an instrument is how instruments become subsystems:

- **Not a timing bound, and it closes no release gate by itself.** It produces *mechanism evidence*.
- **It does not touch `LE-09`.** No hardware tier follows from an allocation census.
- **Not a heap.** Nothing here weakens `validate_no_heap`; the no-heap gate stays exactly as strict.
- **Not a general profiler.** No call-graph, no sampling, no symbolisation. Counters and phases.
- **Not `dealloc`/residency accounting** in its first form, matching Sharc's deliberate omission —
  though `G12` (*"allocation and reclamation latency"*) will eventually want the reclaim side, which is a
  reason to keep the counter set extensible rather than a reason to build it now.

## 7. Decisions this scoping does not take

1. **Placement.** A new `FEAT-P0-*` Feature for self-analysis instrumentation, versus a Story under an
   existing Feature. Handover 26 trap 6 applies: a seventh Story on a Feature means re-decomposing it,
   not extending it. The counters must live in a shipped crate (they are read from hot paths); the
   *harness* belongs in `xtask`. **Which shipped crate owns the counters is the load-bearing half** —
   `kernel` if the subject is the pool, `hal` if it is meant to be architecture-neutral.
2. **What counts as an "allocation" in a no-heap kernel.** Pool claims only, or also address-space
   mappings, IPC queue claims, `exec::shared_memory::grant`, and stack high-water? `G11`'s target names
   pool claims explicitly and nothing else, which argues for starting there — but `D09`'s accept path is
   where `LE-42` lives, and it may claim more than pool slots.
3. **The work unit per domain.** `allocs/node` has no analogue until each domain names its unit: per
   dispatch (`D05`), per interrupt entry (`D02`), per context switch (`D04`), per tick (`D03`), per
   accepted image (`D09`). **These must be the units the existing catalogue rows already imply**, not new
   ones, or the instrument will not close the rows it exists to close.
4. **Whether host-side `xtask` profiling is in scope at all.** `xtask` is not a shipped crate, has a
   heap, and carries 137 tests — so a literal `CountingAlloc` port works there unchanged. It would profile
   *the tooling*, not the OS. Cheap and genuinely useful for the build path; **out of scope for
   "real-time hot paths"**, and worth naming so it is not smuggled in as if it were the same job.
5. **Whether the ceiling gate rides the existing harness or gets its own.** TinyOS already has
   `TINYOS-MEAS/1` envelopes, `xtask measure`, `check-timing-regression` and committed baselines. Sharc's
   profiler carries its own ceiling and its own JSON. **Reusing the envelope is strongly preferable** —
   it inherits the ratio gate, the tier field, and `LE-33`'s future platform-identity field — but the
   envelope currently carries cycles, and an allocation count is a different quantity that must not be
   mistaken for one. That is a schema decision, and `LE-35` will bite on the contract selection exactly
   as it does for `-M virt`.

## 8. Risk, stated plainly

**The largest risk is not implementation — it is a believed zero.** `G11`'s target is zero, the
instrument's most likely first output is zero, and a zero is what everyone wants. §5's positive-control
clause is the whole defence, and it is cheap: plant an allocation, see it counted, record both in the
Report.

**The second risk is the alloc-count-is-not-wall trap**, which Sharc paid for in a dead end and wrote
down. A −31% allocation reduction with zero wall change is a real, measured outcome. If this instrument
is used to justify an optimisation, the wall claim needs the wall, from the production path — and on
this project, under `ADR 0005`, from a qualified platform before it is a bound at all.

## 9. Recommendation

**Adopt the pattern; do not vendor the file.** Specifically:

- Take the **two-atomics-never-written-in-ship** split verbatim as a design. It is nineteen lines and it
  is the right answer for a real-time kernel.
- Take **normalisation by work unit** and **a ceiling that exits non-zero**, because those are what made
  Sharc's numbers diagnostic rather than decorative.
- Replace the **subject**: pool claims and bounded-resource acquisitions, not `GlobalAlloc`.
- Write the **positive control into the Test document as a Red clause** before the counter is believed.
- Point it at **`D09`/`LE-42`** first.

**Where it sits in the work order** is the owner's call, and [38A](38A-outstanding-actions.md) now
carries it as **W5, unranked**. The argument for ranking it above `W3`/`W4`: it is the missing instrument
for **50 already-specified release gates**, and it is the only proposed work that would explain
`LE-42`'s 17.6–39.1× overshoot rather than re-measuring it. The argument against: `LE-33`'s second
condition should arguably land first, so that the numbers this tool produces cannot be promoted into
bounds by a gate that does not yet exist.

## Concurrency, per rule 7 — and it bears directly on §9's ranking

**A concurrent session was live throughout, and it appears to be building the very gates this document
argues should come first.** At commit time its uncommitted working tree carried new
`goals/assurance/open-debt.tsv` and `qualified-platforms.tsv`, a new `STORY-P0-01-07` and
`TEST-P0-01-07-A`, and new `os/src/xtask/src/bound_provenance.rs` and `spine_files.rs`. Running
`check-assurance-spine` against that tree reports capabilities that did not exist an hour ago:
**`0 bound claims checked`**, **`5 platforms (0 qualified)`**, **`24 open-debt selections`**, and
**`59 Feature/Story status rows agree`** — which are, respectively, `LE-33`'s second condition,
`ADR 0005`'s qualification register, `LE-35`'s unwritten rule, and `LE-44`'s cross-check.

**Nothing of theirs was read, staged, or relied on**, and `LE-33`, `LE-35` and `LE-44` were all still
`open` in the register when this was written — so every claim above is accurate as of this commit. But
**§5 and §9 are the parts to re-check first**: if `LE-33`'s second condition has landed, the argument
against ranking W5 early weakens considerably, because the gate that would refuse to promote this
profiler's numbers into a bound would then exist. **Re-read the register before acting on §9's ranking
rather than inheriting it.** That is this project's own rule, applied to a document that is one hour old.

A second document also claimed slot `39A` (`39A-the-four-machines.md`, created 22 minutes before this
one). On the owner's instruction the slot was **not** resolved by creation order, and the two may be
merged later; theirs was accidentally swept into this session's index by a directory-wide `git add` and
**immediately unstaged** — rule 1's exact failure, caught by reading `git diff --cached` before
committing rather than after.

## State

No change from this document. No code, no contracts, no register change.

```text
main                    93a32cb + this document's commit, UNPUSHED, three sessions
assurance spine         44 loose ends (30 open), 84 status headers — unchanged
host tests              549, unchanged
catalogue               625 specified; G11+G12 = 50 rows awaiting this instrument
```
