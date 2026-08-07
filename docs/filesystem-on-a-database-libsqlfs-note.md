# A Filesystem on a Database — notes on `libsqlfs`, and what transfers

Status: **Reference note, 2026-08-07. No code, no Feature, no Story, no decision.**
Written because the owner has been circling the same idea and an outside existence proof
turned up. It deliberately stops short of decomposition — `agent.md`'s just-in-time rule,
and the 2026-07-30 hardware-evidence sprint rule, both cut against opening design surface
here.

Companion to [`tinydb-rt-scope.md`](tinydb-rt-scope.md), which settles the **engine**
question (SharCrust, the SQLite-*format* engine, as the RT proving ground). This note is
about the **layer above it**: what happens if a filesystem is a set of tables rather than a
block device with a directory tree bolted on.

Source: <https://www.nongnu.org/libsqlfs/> — read 2026-08-07.

---

## 1. What `libsqlfs` actually is

A C library implementing *"a POSIX style file system on top of an SQLite database"*, giving an
application *"a full read/write file system in a single file."*

- **POSIX semantics**, not a toy subset: read, write, `mkdir`, `symlink`, `chmod`, and a custom
  `type` field for metadata beyond the standard attributes.
- **Two structures carry everything**: `struct key_attr` (metadata) and `struct key_value`
  (contents). **Files are keys.** The directory tree is a naming convention over a key space,
  not a structure in its own right.
- **FUSE is optional.** It exists to expose the database as an OS-level mount; the library
  works standalone for embedded use. That library mode is the only shape that could ever mean
  anything here — TinyOS has no FUSE, no VFS, and no mount concept.
- Built by PalmSource for their ALP platform. **LGPL v2 or later.**
- Extended attributes unsupported at the version documented; tested on Linux i386/StrongARM.

## 2. The one fact worth carrying: it was tested by building GCC and the Linux kernel through it

No performance figures are published, so there is nothing to quote and nothing to compare
against. What *is* stated is far more useful:

> testing included "the complete build process of gcc and the Linux kernel" and the "Apple
> file system test tool."

A kernel build is millions of file operations with deep trees, hardlinks, rename-over,
concurrent readers and a brutal metadata-to-data ratio. **That is an existence proof that
POSIX semantics survive being mapped onto a key/value-over-SQL store** — not a benchmark, and
much better than one for our purposes. The question *"does this even work?"* is answered by
someone else, for free, and we can stop asking it.

## 3. Why the code cannot come here, briefly, so nobody re-derives it

None of this is a criticism of `libsqlfs`; it is a statement about which machine it was written
for.

| | |
|---|---|
| **Licence** | LGPL v2+ against this repository's **MIT** (`LICENSE`). Vendoring or deriving from the source changes this project's licensing position. **The published description is fair game; the source is not to be copied.** If anything here is ever built, it is clean-room from the concepts in §4. |
| **Language and runtime** | C on SQLite proper, which wants `malloc`, a VFS, and a libc. The assurance spine forbids `#[global_allocator]`, `extern crate alloc` and even `use alloc::` anywhere in the image, and withdraws `G11` loudly if any appears. |
| **Real time** | SQLite has no WCET story, and neither does a transaction. `EPIC-P1` is *Determinism Proof*. |
| **Size** | SQLite is comfortably past the 20,000-line crate ceiling on its own, and the ceiling has no exceptions. |
| **Hostile input** | Reading a database file that TinyOS did not itself create is **a complex hostile-format parser**, which `BND-03` forbids in `C1` and `PD-12` makes an explicit non-goal. `tinydb-rt-scope.md` §4 already rules this out in its own words, and it is the same argument that stopped the device-tree parser in `LE-98` on 2026-08-07 — a file-format parser is the most CVE-prone shape in any of these designs. |

**So the transferable thing is the schema and the semantics argument, not a dependency.**

## 4. What genuinely transfers

Four things, in rough order of how much they are worth.

**1. The metadata/data split is a design worth copying.** `key_attr` and `key_value` as
separate structures keyed by path means a `stat` never touches file contents, and a directory
listing is a query over the metadata table alone. On a machine with a fixed page pool that
distinction is the difference between an `O(depth)` bounded lookup and dragging file bodies
through the cache to read a name. `RamVolume` (`os/src/shell/src/volume.rs`) already keeps
labels beside content for `G-SEC-5`; the same split, one layer down.

**2. "Files are keys" removes a whole class of structure.** There is no inode table, no
directory as an object, no free-block bitmap — so there is no `fsck`, because there is no
second structure that can disagree with the first. For an OS whose pitch is *fail-safe over
keep-trying*, **crash consistency by construction rather than by repair** is the strongest
argument in the whole idea, and it is worth stating in exactly those terms whenever this comes
up again.

**3. It confirms the layering `tinydb-rt-scope.md` already chose.** That note rules the engine
*an application, not a kernel subsystem*. `libsqlfs` is the same shape from the other end: a
**library** that an application uses, with FUSE — the OS integration — strictly optional and
bolted on last. Two independent designs landing on the same boundary is mild evidence the
boundary is right.

**4. The `type` field.** `libsqlfs` carries a custom type beyond POSIX attributes. TinyOS
already has a strictly stronger version of this idea in `G-SEC-5` labels, which *travel*
through copy and rename and can gain derivation history but never shed it. Worth noting that
the outside world reached for the same affordance and stopped well short of it.

## 5. The uncomfortable half, stated plainly

**The best part of this idea is the part TinyOS defers longest.**

`libsqlfs`'s central value proposition is transactional: atomic multi-file operations,
concurrency control, crash consistency — all of it resting on SQLite's **write** path.
`tinydb-rt-scope.md` §4 excludes the write path from every staged increment, and correctly:
*"a page split is unbounded work wearing an amortised disguise."* It also excludes persistence
outright, because TinyOS cannot read the SD card or NVMe on this board at all, and it excludes
parsing any database file that arrived from off-board.

So a hypothetical TinyOS filesystem-on-a-database would, at the stages currently thinkable, be
**a read-only view of an in-RAM byte range that dies with the boot** — which is not a
filesystem in any sense a user would recognise, and delivers none of the crash-consistency
argument that makes the idea attractive.

That is not an objection to the idea. It is the honest sequencing: **the idea's payoff is
gated on a bounded write path and on persistent storage existing at all**, and neither is
close. Anyone returning to this should check those two things first, because if they have not
moved, nothing else here has either.

## 5a. One database per drive — the owner's extension, 2026-08-07

**The proposal:** each database *is* a drive in the traditional `A:`/`C:` sense, and drives
become dynamically sized rather than fixed.

This is a better fit than it first looks, and it has one sharp catch.

### It lands on a surface that already exists and is currently a stub

`os/src/shell/src/volume.rs` already parses drive prefixes — and today **`A:` is the only
drive that exists**. `B:` is not unimplemented; it is *explicitly refused*, with a test
pinning it:

```rust
assert_eq!(vol.create(0, "B:\\X.TXT", b"x", Labels::seeded()), Err(VolumeError::BadPath));
```

So the syntax, the path walker and the refusal are in place, and what is missing is exactly
one thing: a second store to point a letter at. "One database per drive" is not a new concept
bolted on — it is the concept the shell already half-implements, given something to name.

### The reframe that dissolves "dynamic vs bounded"

The obvious objection is that *dynamically sized* contradicts everything this project
enforces: fixed capacities, no allocator, bounded work. That objection is answered by a
distinction worth keeping:

- **A declared ceiling** is what bounds B-tree depth, and therefore bounds lookup latency.
  `tinydb-rt-scope.md` §3 already relies on it — *"depth bounded by a compile-time
  database-size ceiling, refused past it."*
- **Occupancy** is a different quantity, and nothing requires it to equal the ceiling.

**A ceiling is a bound, not an allocation.** A drive can be sparse and growing up to a stated
maximum while every lookup inside it stays `O(≤ MAX_DEPTH)`. That delivers what dynamic sizing
is actually wanted for — no pre-partitioning, no reserved-and-unused space, no resize tool —
without touching the property the real-time path needs. The bound is on *depth*, and depth is
a function of the ceiling, not of how full the drive is.

So the honest form of the idea is **"declared ceiling, dynamic occupancy"**, and stating it
that way is what keeps it compatible with the assurance spine.

### The catch: dynamic drives re-create the exhaustion problem, one layer down

If several drives grow into a shared page pool, **one drive can starve another** — a drive
that fills takes pages a second drive will later need, and the refusal surfaces in the
innocent one. That is not a new problem. It is exactly `BND-15` and `RCG-08`: *one flooding
domain cannot consume another class's budget or any reserve.*

The mechanism that would make multi-drive dynamic sizing safe is **`FEAT-P1-12` — the RT
reserve and the per-class budget — which does not exist** (`Tcb` carries no containment class,
the pool is one flat capacity with no reservation floor). So:

> **Dynamically-sized drives inherit a scheduling problem, and their prerequisite is a
> Feature currently on the do-not-start list.**

That is a real dependency and it points the right way round: per-drive floors are the same
mechanism as per-class floors, so `FEAT-P1-12` would serve both. Anyone tempted to build
dynamic drives first would be re-implementing that reserve badly, in storage code, without the
contract.

### Two things the split buys that a single filesystem does not

**Containment is structural rather than enforced.** Separate databases share no B-tree, no
freelist and no page map, so a malformed or hostile store on one drive cannot corrupt another
— there is no shared structure through which it could. That is a materially better isolation
story than one filesystem with one metadata tree, and it composes with the class model: a
*drive* is a plausible unit to carry a containment class, which a directory never was. It also
narrows the hostile-parse surface `tinydb-rt-scope.md` §4 rules out — a store admitted from
off-board would be one drive's problem, quarantined by construction rather than by review.

**The atomicity limit is free, because it is already the familiar behaviour.** `libsqlfs` gets
atomic multi-file operations from one database's transaction. Across two databases that would
need two-phase commit — unbounded, and requiring a recovery path. So the rule has to be
*atomic within a drive, never across drives*. That is **exactly DOS semantics**: a cross-drive
move has always been copy-then-delete and has never been atomic. The constraint the
architecture forces is the behaviour users already expect, which is a rare thing and worth
noticing.

### What it does not fix

Nothing here moves §5. With no persistent device readable on this board, "drives" are RAM byte
ranges that die with the boot, however many of them there are and however elastically they are
sized.

## 5b. The hierarchy is a view — the owner's second extension, 2026-08-07

**The proposal:** how files are organised is *a view of the data in the database*, so one
store can present Windows-style, macOS-style or Linux-style organisation, and file layout
becomes flexible rather than baked in.

**This is the strongest form of the whole idea, and it is already latent in §1.** `libsqlfs`
stores files as *keys*; its directory tree is a naming convention over a key space, not a
structure in its own right. Nothing in that design makes the convention singular. The owner is
naming what the schema already implies: **if there is no directory object, there is no
canonical hierarchy either.**

It is also a good fit for where this project is going. `EPIC-P2` is DOS parity, a Win32 shim
already exists, and `docs/whole-system-context.md` carries compatibility layers as a
destination. Today the DOS-ness lives *in the storage layer*. Making it a view moves it to
where it belongs and lets a second personality exist without a second store.

### The sharp edge is identity and existence, not rendering

Rendering a hierarchy from a key space is the easy half. The hard half, and the one that
usually gets discovered late, is that **naming policies disagree about what exists.**

**Case sensitivity is a collision rule, not a display preference.** A Linux view *requires*
that `README.TXT` and `readme.txt` be two files. A Windows view *requires* that they cannot
both exist. Those are not two renderings of one key space — they are contradictory
constraints on it. One policy has to be canonical and the others are lossy projections of it.

**This is not hypothetical here; the decision has already been taken, in storage.**
`os/src/shell/src/volume.rs:36` matches name components with `eq_ignore_ascii_case`, described
in its own comment as *"DOS ergonomics"*. So the current store **cannot hold both spellings at
once** — a Linux view over it is not a missing feature, it is a decision to revisit. That is
exactly the sort of thing worth knowing before rather than after.

Three more asymmetries in the same family:

- **Reserved names.** `CON`, `NUL`, `AUX`, `PRN` are legal filenames on Linux and unnameable
  on Windows. A file created through one view can be **unreachable** through another. Views
  are not symmetric, and the projection can lose entries rather than merely rename them.
- **8.3 short names are stateful.** `PROGRA~1` depends on which siblings already exist, so a
  DOS view is not a pure function of a key — it needs a persisted mapping that survives
  restarts, or the same file gets a different short name after a reboot.
- **Separators and legal character sets** differ, so a name legal in one view may need
  escaping in another, and escaping is a place where two files can collide into one.

None of these kills the idea. They mean the design decision is **"which naming policy is
canonical, and what are the others allowed to lose"**, and that it must be *declared* rather
than discovered when the second view is written.

### What the split gives you for free

**`G-SEC-5` labels survive it without any work.** Labels are already a property of the file,
and if files are keys then labels attach to keys rather than to paths. A file reachable as
`C:\DOCS\README.TXT` and `/docs/readme.txt` carries the same label through both, and
`RamVolume::copy`/`rename` already propagate labels bit-for-bit with derivation history that
can grow but never shed. **A view cannot launder a label**, because the view does not own it.
That property is worth stating explicitly, because "the same file under two names" is exactly
the shape a labelling scheme usually gets defeated by.

### The real-time reading

A view is a canonicalisation step on **every** path resolution, so each one must be a total,
bounded, allocation-free function from a name to a key. `volume.rs`'s path walker already is
one — it refuses `..` escapes and unknown drive prefixes rather than allocating or looping —
so this is a known shape rather than new risk. Each additional view is another such function,
and a path string is a vastly simpler input than a file format: **this is not the `BND-03`
hostile-parser problem**, and it should not be argued as if it were.

### Why this belongs in a note now rather than in code later

The canonical key form is the one decision here that is **expensive to change afterwards**.
Retrofitting canonicalisation over stored data is a migration, and TinyOS has no migration
story and should not grow one to fix a naming choice. Everything else in §5b can be deferred
indefinitely at no cost.

So the single sentence worth carrying forward: **decide the canonical key form and the
case-collision rule before anything writes a byte; the views themselves can wait forever.**

## 5c. "A drive is too big for one database" — an apps engineer's pushback, 2026-08-07

**The proposal:** one database per drive is the wrong granularity. Introduce a **cluster** — a
defragmented region on disc holding a single database file with all its contents — and make a
drive *a collection of clusters*.

**The shape is right and the stated reason is not the reason.** Working through it changes what
the split should be *for*, which changes where the boundary goes.

### What the objection gets right

**Blast radius and independent lifetime are real, and they are the same argument §5a already
makes one level up.** Separate stores share no B-tree, no freelist and no page map, so damage,
quarantine, sizing and discard are all per-store. If a drive is one store, the drive is the
unit of every one of those, and that is too coarse — a scratch area and an audit journal should
not be able to lose each other. The engineer is applying §5a's own reasoning consistently, and
the conclusion "a drive is a collection of stores" follows.

### What "too big" does not mean here

Three of the usual reasons a single large store is painful do not apply on this machine, and
saying so keeps the discussion on the reason that does.

- **Depth is `O(log n)`.** A B-tree lookup cost grows with the *logarithm* of size; a store two
  orders of magnitude larger is a page read or two deeper. `tinydb-rt-scope.md` §3 bounds
  lookups by `MAX_DEPTH`, and the bound comes from a **declared ceiling**, not from occupancy
  (§5a). Size is close to the weakest argument available against one store.
- **The page pool is sized by depth, not by store size.** A depth-bounded lookup needs
  `MAX_DEPTH` buffers whether the store is 1 MB or 1 TB. Splitting one store into ten does not
  shrink the working set; it multiplies the number of open stores whose roots must be resident.
- **Lock contention and write amplification are moot.** Single core, and
  `tinydb-rt-scope.md` §4 excludes the write path from every staged increment. There is no
  concurrent writer to serialise.

### The load-bearing objection: a "region on disc" rebuilds the thing we removed

This is the part worth stopping on.

§4's strongest argument for the whole design is that there is **no inode table, no directory
object and no free-block bitmap — so there is no `fsck`, because there is no second structure
that can disagree with the first.**

A *defragmented region on disc* is an **extent**. If TinyOS decides where clusters begin, how
big they are, how they grow and how their space is reclaimed, then TinyOS is running a
**free-space manager over a raw device** — which is a second structure, which can disagree with
the first, which is exactly `fsck`. The proposal would hand back the single best property of
the design in exchange for a granularity that can be obtained another way.

It also inherits *defragmentation itself*: keeping a region contiguous means relocation, and
relocation is unbounded work with a recovery story, on a project whose storage rule is
`O(depth)` or refuse.

**The escape is available and cheap:** let each store be **a whole object to the layer
beneath**, never an extent this project places. A file on a host filesystem, a whole partition,
a named region a bootloader hands over — anything where somebody else owns the free space. Then
"a drive is many stores" is bought without owning an allocator, and the no-`fsck` property
survives. The distinction is precise and it is the whole difference between the two designs:
**many stores, yes; extents we manage, no.**

### The split criterion should be containment, not size

If the reason to split is blast radius, lifetime and quarantine, then those should *be* the
boundary. This project already has the vocabulary: a store per **containment class**, or per
independently-discardable lifetime, is a boundary that means something and that `BND-*` can be
written against. A store per *n* megabytes is arbitrary — it will cut across a class one day
and nothing will notice.

Stated as a rule: **split where failures, authority or lifetimes must be independent; never
where a number got large.**

### The cost the proposal introduces, which should not be paid silently

§5a gets one property free: **atomic within a drive, never across drives** — and that is
already DOS behaviour, so it costs nothing. Making a drive many stores **gives that back**. A
rename between two directories that happen to live in different stores is now a cross-store
operation, which needs two-phase commit — unbounded, with a recovery path — or must be
non-atomic in a place where users expect atomicity, *within a single drive*.

That is a genuine regression, not a detail. It can be designed around (make the store boundary
one users cannot cross implicitly), but it must be **decided**, because the failure mode is a
half-completed rename inside one drive letter.

### The word is already taken, and taken in exactly this project's face

**In FAT and NTFS a *cluster* is the minimum allocation unit — typically 4 KB.** This project
ships DOS parity, prints `Insufficient disk space` from `verbs.rs`, and is read by people for
whom "cluster" means that and nothing else. Using it for a multi-megabyte store guarantees a
conversation at cross purposes, five orders of magnitude apart, with the one audience most
likely to read this design. Pick another word before the name sets — *store*, *segment* or
*volume-part* all survive contact with a DOS reader.

### Where this lands

The engineer's shape survives: **a drive is a collection of stores.** Two amendments, and both
are about keeping properties this design already has:

1. **Not extents.** Each store is a whole object owned by the layer below, or the free-space
   manager and `fsck` come back.
2. **Split by containment and lifetime, not by size** — and state the cross-store atomicity
   rule at the same time, because that is what the split costs.

Nothing to build. On this board there is still no readable persistent device at all (§5), so
every word above is about a shape to hold, not work to schedule.

## 5d. A cluster is an indexing scope, not a size — the refinement, 2026-08-07

**The clarification:** a cluster has *`file_ptr`-like speed for out-of-core access*, and
indexes and speeds up file access **within that region or scope**.

**This is a much stronger argument than size, and it revises §5c.** §5c judged "too big" a weak
justification because depth is `O(log n)`. That judgement stands against *size* and does not
touch *scope*. A cluster justified as an **indexing and resolution boundary** is a different
proposal, and a better one.

### What it actually buys, in this project's terms

**Path resolution stops being paid per access.** Today `volume.rs` walks name components on
every lookup. A cluster handle is a *resolved* handle: pay the walk once, then operate inside a
bounded scope. That is precisely the shape a real-time path wants — one bounded resolution,
then repeated bounded scoped operations — rather than a global lookup on the hot path.

**The depth bound gets tighter, and tightness is what WCET cares about.** §5c noted that a
larger store is only a page read or two deeper *on average*. But `MAX_DEPTH` is a **worst-case
constant**, and a per-cluster ceiling makes it small and local instead of large and global. A
drive-wide ceiling forces every lookup in every scope to be budgeted against the deepest tree
the drive could ever hold. Per-cluster ceilings let a small scope declare a small bound and
*mean it*. **That is a real gain and §5c undersold it** — the argument is about the bound, not
about the average.

**The resident set follows locality.** §5c said the page pool is sized by depth, not by store
size — true, but with one index per scope, a workload confined to one cluster needs that
cluster's root and depth resident and nothing else. Scoped indexes make the *fixed* pool go
further, which on a machine with `N` compile-time page buffers is the constraint that actually
binds.

### The sharp edge: a fast handle is cached authority

This is the part to get right, and the project already knows how.

A handle that reaches data **without re-walking the name** has also skipped whatever the walk
*checked* — the `G-SEC-5` label, the containment class, the capability. A raw pointer-like
handle is therefore not a performance detail; **it is a capability**, and a stale one that
outlives its region reads whatever now occupies that space. That is the use-after-free shape in
a new costume.

TinyOS has solved this twice already and both are the pattern to copy, not to re-invent:

- `kernel::mem`'s `PoolHandle` — *"opaque by design and generational: both the slot index and
  the allocation generation"*, with slots **permanently retired** rather than silently reused.
- `exec::shared_memory`'s grants — generation-stamped, so *"the token's generation no longer
  matches"* is a typed refusal rather than a stale read.

**A cluster handle must be generation-stamped, revocable and validated on use**, at which point
it is a grant rather than a pointer — and it keeps `file_ptr`-like cost, because a generation
compare is an integer compare, not a walk. The speed survives the safety. What must not happen
is a handle that is fast *because* it skipped the check.

### Two costs that land where this project has already deferred them

**An index that speeds reads is maintained on writes.** Scoped indexes are free to read and
unbounded to update — page splits, rebalances, and the index-versus-data ordering problem on
crash. `tinydb-rt-scope.md` §4 excludes the write path from every staged increment, so the
benefit is available in the stages that exist and the cost arrives in the stages that do not.
That is the same asymmetry §5 records for the whole idea, and it is not an objection so much as
a reason not to be surprised later.

**Two indexes can disagree**, which is §5c's `fsck` argument in a new place. A per-cluster index
plus any drive-level directory is a second structure — *unless the drive-level view is
**derived** from the cluster indexes rather than maintained beside them.* Which is exactly
§5b: **the hierarchy is a view.** The three extensions hold together only if that one is taken
seriously; a materialised drive-level directory would quietly undo it.

### And it still presumes a device this board does not have

*Out of core* means there is a core to be out of. TinyOS cannot read the SD card or NVMe on
this board at all (§5, `tinydb-rt-scope.md` §4), so today every cluster handle is a handle into
RAM that dies with the boot. The design is sound; the hardware premise is unmet.

## 5e. What a cluster actually is — the unifying statement, 2026-08-07

Three further refinements from the owner, which turn out to be one idea:

> *A cluster makes DB access and resource access deterministic.*
> *Each cluster can have its own entitlement for security and data integrity.*
> *Clusters can be transient or persistent, for apps and sandboxes.*

**Taken together these stop describing a storage subdivision and start
describing a containment unit.** That is a much better thing for it to be, and
it is the form worth carrying forward.

### One correction of vocabulary, because this project is careful about it

*Deterministic* is not quite the claim, and `ADR 0015` (accepted 2026-08-06)
draws the distinction the whole tree now turns on: **TinyOS declares and
enforces a budget; it does not claim a measured worst case.** A cluster does not
make access take the same time every time — it makes access **bounded, with the
bound declarable per scope.**

That is the stronger statement, not a weaker one. A drive-wide ceiling forces
every scope to be budgeted against the deepest tree the drive could ever hold. A
cluster lets a small scope **declare a small bound and mean it** (§5d). So the
precise form is: *a cluster is the unit at which an access bound can be stated
truthfully.*

### The four properties are the same property

Each refinement names a different thing a cluster bounds, and they are all the
same boundary:

| what it bounds | mechanism it needs | where it already exists here |
|---|---|---|
| **Access latency** | scoped index, per-cluster `MAX_DEPTH` | `tinydb-rt-scope.md` §3 |
| **Resource draw** | a declared page/space reservation | `FEAT-P1-12`'s floor — **unbuilt** (§5a) |
| **Authority** | entitlement carried by the cluster | `G-SEC-5` labels, `exec::shared_memory` grants |
| **Lifetime** | transient or persistent, declared at creation | nothing today |

**A cluster is therefore the unit of budget, authority and lifetime at once** —
and a boundary that carries all four is exactly what this project already calls
a **containment class**. That is the sentence to keep: *a cluster is a
containment boundary that happens to be made of storage.*

It also retires §5c's remaining doubt. §5c argued the split criterion should be
containment rather than size; §5e says the owner's cluster *is* that, provided
the entitlement is declared at the boundary rather than inherited from whatever
opened it.

### Entitlement per cluster is the strongest of the three

A store that carries its own entitlement means the **authority to read or write
it is a property of the store, not of the caller's path to it.** Two
consequences worth stating:

- **A hostile store is quarantined by construction.** `tinydb-rt-scope.md` §4
  refuses any database arriving from off-board because a file-format parser is
  the most CVE-prone shape in the design. A cluster with its own entitlement
  makes that a *narrower* refusal rather than a blanket one: an untrusted store
  is a cluster whose entitlement grants nothing but read-into-a-`C4`-domain, and
  it cannot reach the audit journal in the next cluster because it has no
  entitlement to it and shares no structure with it (§5a).
- **Integrity becomes per-scope and declarable.** A journal cluster can demand
  checksums-on-read; a scratch cluster can decline them and say so. Today that
  choice would be global, which means it is set by whichever workload is most
  paranoid and paid for by all of them.

### Transient versus persistent is what makes sandboxes cheap

Declaring a cluster **transient at creation** means it has no persistence
obligation at all: no flush, no crash-recovery path, no `EnsurePowerOn`-style
guarantee, and teardown is dropping the region rather than reclaiming space
inside a shared one. For an app sandbox that is the whole lifecycle, and it
composes with `STORY-P1-03-02`'s generation-safe teardown, which already
requires that a torn-down space cannot be reached through a stale handle before
its frames are reused.

**And it is honest about this board.** §5 records that TinyOS cannot read a
persistent device at all, so *every* cluster today is transient whether or not
it says so. Declaring the axis now costs nothing and means the persistent case
is a value rather than a retrofit — which matters, because §6 item 1b already
warns that the one expensive thing here is a decision that has to be reversed
after bytes exist.

### What this does not resolve

The dependency in §5a is unchanged and gets sharper: **per-cluster resource
entitlement is `FEAT-P1-12`'s reservation floor**, which does not exist, and
which the do-not-start rule covers. A cluster that declares a budget nothing
enforces is a comment. So the honest ordering is: the *shape* is now settled
enough to stop re-deriving, and the first mechanism it needs is one the project
has already scoped and deliberately not started.

## 5f. Related files, and integrity as an invariant rather than a dial — 2026-08-07

Two more from the owner, in one sentence:

> *A cluster represents a collection of **related** files stored in a DB instead
> of a file system. **The integrity of data is always ensured within a cluster.***

The first sharpens §5c's split criterion. The second **contradicts §5e**, and the
contradiction is the useful part.

### "Related" must be co-extensive with containment and lifetime, or the boundary lies

§5c concluded that stores split by *containment*, never by size; §5e added
lifetime and entitlement. "A collection of related files" could quietly reopen
the size argument under a friendlier word — *related* is exactly the kind of
criterion that starts as containment and decays into convenience, because
everything is related to something.

So the form to carry is the constraining one: **files belong in one cluster when
they share a fate.** Same authority, same lifetime, same budget, same integrity
obligation. If two files are "related" but one is transient scratch and the other
is the audit journal, they are *not* one cluster no matter how naturally a person
would group them — because §5e's boundary carries authority and lifetime, and a
boundary that carries two different lifetimes carries neither.

That gives a testable rule rather than a feeling: **relatedness is not the
criterion; shared fate is, and relatedness is how a person recognises it.**

### The contradiction, stated plainly

§5e says integrity is per-scope and declarable — *"a journal cluster can demand
checksums-on-read; a scratch cluster can decline them and say so."* The owner
says integrity is **always** ensured within a cluster. Both cannot stand, and the
owner's is the better contract for three reasons this project already argues
elsewhere:

- **A dial gets set wrong once and is silent about it.** §5e's own case for the
  dial was that a global choice is "set by whichever workload is most paranoid
  and paid for by all of them" — but the failure mode of a per-scope dial is
  worse than paying for paranoia: it is a scratch cluster that was cheap on
  Tuesday and load-bearing on Friday, with nothing marking the transition.
- **An invariant can be tested; a dial can only be inspected.** The whole
  assurance spine is built on absence-tested properties, and "no cluster
  anywhere is readable without its integrity check" is exactly that shape.
- **It costs what it costs, in the open.** Uniform integrity means the cost is in
  every declared budget rather than hidden in the ones that opted in — which is
  `ADR 0015`'s posture applied to storage: declare and enforce a budget.

**So §5e's integrity dial is withdrawn.** Entitlement still varies per cluster
(who may read and write it); integrity does not (what a read is allowed to
return). §5e's other three properties — access bound, resource draw, lifetime —
are unaffected.

### The question this raises, which is not yet answered

**"Always ensured" has two readings, and they cost very differently:**

1. **Always *detected*.** Every read is checksum-verified; a mismatch is a named,
   fail-closed refusal and never a returned byte. Bounded, cheap, and exactly
   this project's posture everywhere else — a refusal is spoken, never a plausible
   value. It needs no redundancy.
2. **Always *repaired*.** Corruption is corrected transparently. This cannot be
   done without redundancy *inside the cluster* — a second copy, ZFS's ditto
   blocks or a mirror — so it changes the space contract, and a cluster's declared
   reservation must then cover its own replicas.

Reading 1 is achievable now and consistent with `SECURITY_CHARTER.md`'s
fail-closed default. Reading 2 is a storage-redundancy Feature with its own
budget arithmetic. **Until this is decided, the note should not be read as
promising either** — and the decision belongs in the ADR §6 already requires,
not in a Story.

### What "within a cluster" excludes, and why that is a feature

The guarantee is scoped to a cluster, so **there are no cross-cluster
transactions** and none should be added. That falls out of §5a's no-shared-
structure rule and is worth stating as a positive: an operation spanning two
clusters has no atomicity guarantee, which is precisely what keeps the write
bound *local* and the blast radius one boundary wide. A design that later wants
multi-cluster atomicity is asking for a global commit, and a global commit is a
global bound — the thing §5d removed.

### Where 5f lands

The shape is now settled to the point of being repetitive, which is the signal to
stop refining it. §6's conditions are unchanged and still unmet; the first
mechanism is still `FEAT-P1-12`'s reservation floor, which still does not exist.
This section adds no surface, exactly as §7 says the note must not.

## 5g. Transactional mutation is the mechanism — and §5f asked the wrong question, 2026-08-07

The owner, closing §5f's open question:

> *Integrity always ensured ⇒ **files are mutated as a transaction**. This is
> clearly the foundation on which ZFS is based.*
> *Even if you pull the plug, the files are not corrupted.*

**§5f's question was mis-framed and is withdrawn.** It offered *always detected*
versus *always repaired* — both of which are about **corruption of stored bytes**.
The guarantee being made is about **atomicity of mutation**: there is never a
torn intermediate state to detect in the first place. That is a different axis,
and it is the foundational one. Getting the axis wrong would have sent the ADR
looking for redundancy when what it actually needs is a commit protocol.

### Three separable properties, and which one this is

ZFS holds all three; they are independent, cost differently, and only the last is
still open:

| property | mechanism | needs redundancy? | status here |
|---|---|---|---|
| **Atomicity** — no torn state, ever | copy-on-write, then one atomic root swap | no | **this is the owner's guarantee** |
| **Detection** — a bad block is never returned as good | checksum in the parent block pointer, verified on read | no | cheap, ZFS-native, assume yes |
| **Repair** — a bad block is transparently corrected | a second copy: mirror, ditto blocks, RAIDZ | **yes** | still open; changes §5a's reservation arithmetic |

Atomicity and detection together are what "even if you pull the plug, the files
are not corrupted" actually requires. Repair is a separate Feature with a space
cost, and the ADR should say so rather than letting it ride in on the same
sentence.

### This ratifies §4 item 2 rather than extending it

§4 item 2 already identified *"crash consistency by construction rather than by
repair"* as the strongest argument in the whole idea, and asked that it be stated
in exactly those terms whenever this resurfaces. It has resurfaced, and it is
stated in exactly those terms. **Two independent arrivals at the same conclusion
is the signal to stop re-litigating the mechanism and start costing it.**

### What copy-on-write costs, and why the cluster boundary is what makes it affordable

This is the part the note has not yet drawn, and it is where the real-time goal
and the storage design meet.

- **A mutation ripples to the root.** Under COW nothing is overwritten in place,
  so changing one leaf rewrites every block on the path above it, up to and
  including the root that gets swapped. A single small write therefore costs
  `O(depth)` block writes. **§5d's per-cluster `MAX_DEPTH` is exactly that bound**
  — the cluster's indexing depth and its write-amplification bound are *the same
  number*. That convergence is the strongest argument for the cluster boundary
  yet made in this note: a drive-wide tree would force every write to be budgeted
  against the deepest path the drive could ever hold, and a cluster lets a small
  scope declare a small write cost and mean it (`ADR 0015`: declare and enforce).
- **Transaction groups cannot be inherited as ZFS batches them.** ZFS accumulates
  dirty state and commits periodically, and the flush's size is a function of how
  much accumulated — unbounded work wearing an amortised disguise, which is
  `tinydb-rt-scope.md` §4's exact objection to page splits. A bounded form exists
  and is the same mechanism as everything else here: **a per-cluster dirty
  ceiling, declared at creation and flushed when reached**, which is `FEAT-P1-12`'s
  reservation floor once more.
- **COW makes a free-space structure mandatory, which §5c warned against — and
  ZFS answers it.** §5c said an extent allocator is a free-space manager and a
  free-space manager is `fsck`; §4 item 2 celebrated having *no* free-block
  bitmap. Neither survives COW unamended, because always-allocate-elsewhere
  requires knowing what is free. ZFS's resolution is the one to copy: **the
  allocator's own state is committed inside the same transaction as the data**, so
  there is still no second structure that can disagree with the first, and still
  no `fsck`. §4 item 2's conclusion holds; §5c's simplification does not, and the
  cost it warned about is real but is paid *per cluster, over a fixed
  reservation*, rather than drive-wide.

### The payoff is a boot-time bound, not only a safety property

Worth stating in the project's own currency: the reason no-`fsck` matters here is
not merely that data survives. **A repair-on-mount pass is unbounded startup
work.** An OS that intends to boot to a deadline cannot have one. Crash
consistency by construction is therefore a *real-time* property as much as a
durability one, and it belongs in the RT fitness argument, not only in the
storage chapter.

### The claim is now falsifiable on this bench, which it was not yesterday

*"Even if you pull the plug, the files are not corrupted"* is a claim of exactly
the kind `CODING_STANDARDS.md` refuses to accept on assertion — "iron clad" is
never evidence by itself. It has always been untestable here because pulling the
plug required a hand.

**As of 2026-08-07 it does not** (`LE-95`, closed in `hand-2026-08-07/17A`):
`tos64-power cycle` switches real mains under software control, and the board
comes back and reports on the wire at exit 0. The experiment that would earn this
claim is therefore now writable as a fixture rather than wished for — **N cycles
cut at randomised points inside a write workload, then every cluster mounted and
verified, with the cut points recorded** — and the failure it hunts is the one
that only ever appears at the moment of the cut.

That experiment does not exist and nothing above should be read as though it did.
But the instrument does, and it arrived the same day as the claim.

### Decision: repair is deferred, opt-in later — and what that actually defers

> *We can opt in later to transparent repair — we don't lose much if every
> mutation is a transaction.*

**Accepted, and the reasoning is right, but the loss should be named precisely
rather than as "not much", because the two things being traded are different
kinds.**

With atomicity and detection but no repair, the failure modes split cleanly:

- **A power cut, a crash, a reset mid-write** — fully covered by the transaction.
  Repair is irrelevant to these, which is the owner's point and it is correct.
  This is also the *only* failure class this bench can currently produce.
- **Bit rot, a misdirected write, a device that acknowledges a write it never
  performed** — not covered, and a transaction cannot cover them, because the
  commit was honest and the medium or the device lied afterwards.

For that second class the outcome without repair is **detected and refused**: a
named, fail-closed read error rather than a plausible wrong byte. So the honest
statement of the trade is:

> **Integrity is kept. Availability is what is deferred.** Without repair the
> store never returns bad data; it sometimes cannot return data at all.

That is the same trade this project takes everywhere else — refuse rather than
return something plausible — so deferring repair is consistent rather than a
concession. Two further reasons it costs nothing *today*: repair is meaningless
for a **transient** cluster (§5e), which has no persistence obligation at all;
and persistent clusters do not exist on this board, which cannot read a
persistent device (§5).

**The one condition that makes "opt in later" genuinely free.** §6 item 1b warns
that the expensive mistake here is a decision that must be reversed after bytes
exist, because reversing it is a data migration and this project has no
migration story. Transparent repair is precisely such a decision **if the block
pointer has no room for a second copy.** ZFS's ditto blocks work because its
block pointer carries up to three device addresses from the start, most of them
usually unused.

So the deferral is free on two conditions, both of which cost nothing now:

1. **The block pointer reserves room for replicas from the first written byte**,
   even while every cluster writes exactly one copy. Reserved and unused is a
   field; added later is a format change.
2. **Detection ships from day one** — the checksum lives in the parent pointer,
   as §5g's table already assumes. Without it, repair is not merely absent later,
   it is *impossible* later: you cannot correct what you cannot identify, and a
   store that cannot tell a good block from a bad one has no safe way to choose
   between two copies.

With those two, repair arrives later as a per-cluster entitlement (§5e) over an
unchanged format, which is exactly the shape "opt in later" needs.

## 6. If this is ever picked up

Not a plan — the conditions under which a plan would be worth writing.

1. **A bounded write path exists**, with the page-split bound *stated* rather than measured
   once. Until then the idea's payoff is unreachable.
1a. **If drives are dynamically sized, `FEAT-P1-12`'s reservation floor exists first**
   (§5a). Without it, one drive starves another and the refusal lands on the innocent one.
1b. **The canonical key form and the case-collision rule are decided before the first
   write** (§5b) — the only choice here that is expensive to reverse, because reversing it
   is a data migration and this project has no migration story.
1c. **Stores are split by containment and lifetime, never by size, and never as extents this
   project places** (§5c) — an extent allocator is a free-space manager, and a free-space
   manager is `fsck`.
1d. **Any scope handle is generation-stamped and validated on use** (§5d), following
   `kernel::mem::PoolHandle` and `exec::shared_memory`'s grants. A handle that is fast
   because it skipped the check is a capability leak, not an optimisation.
1e. **A cluster declares its entitlement and its lifetime at creation** (§5e) — authority
   is a property of the store, not of the caller's path to it, and transient-versus-persistent
   is a value rather than a retrofit. Say *bounded*, not *deterministic* (`ADR 0015`).
1f. **Integrity is an invariant of every cluster, not a per-cluster dial** (§5f), and it is
   delivered by **transactional mutation** — copy-on-write with one atomic root swap, so
   there is never a torn state to detect (§5g). Atomicity and checksum *detection* need no
   redundancy and are assumed in; **only transparent *repair* is still open**, and it needs
   a second copy inside the cluster and therefore changes 1a's reservation arithmetic. The
   write bound is `O(depth)` and **§5d's per-cluster `MAX_DEPTH` is that bound** — say
   *bounded*, not *deterministic*. A per-cluster dirty ceiling replaces ZFS's transaction
   groups, whose flush size is unbounded. The allocator COW forces on us is committed
   **inside** the transaction, which is how §4 item 2's no-`fsck` conclusion survives §5c's
   warning.
1g. **Transparent repair is deferred, opt-in later** (owner, 2026-08-07) — the deferral
   trades *availability*, never integrity: without it the store refuses a bad block rather
   than returning it. **It is free only if the format keeps the option**, and both conditions
   cost nothing today: the block pointer **reserves room for replicas from the first written
   byte** even while every cluster writes one copy, and **detection ships from day one**,
   because repair is impossible later without it — you cannot correct what you cannot
   identify, nor choose between two copies. Reserved-and-unused is a field; added later is a
   format change, which is §6 item 1b's one expensive mistake. Files share a cluster when
   they share a **fate**
   (authority, lifetime, budget, integrity obligation); "related" is how a person
   recognises that, never the criterion itself. **No cross-cluster transactions**, ever:
   a guarantee spanning two clusters is a global commit, and a global commit is the global
   bound §5d removed.
2. **TinyOS can read a persistent device.** Today it cannot, on this board, at all.
3. **It is a user-layer application**, per `tinydb-rt-scope.md` §5 — never kernel code, and
   never in `kernel` or `hal-arm64`.
4. **It only ever opens stores it created**, until a `C4`-contained parsing Feature exists with
   its own adversarial tests. A `.arc` off the wire is external bytes.
5. **Clean-room from §4's concepts.** MIT against LGPL; do not read the source into this tree.

## 7. What this note is not

It is not a proposal, not a Feature, and adds no design surface. It records an outside data
point and what it does and does not imply, so that the next time the idea surfaces the argument
starts from here instead of from the beginning. `tinydb-rt-scope.md` remains the document that
settles scope; where the two disagree, that one wins.
