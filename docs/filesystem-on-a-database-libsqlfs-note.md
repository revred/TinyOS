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

## 6. If this is ever picked up

Not a plan — the conditions under which a plan would be worth writing.

1. **A bounded write path exists**, with the page-split bound *stated* rather than measured
   once. Until then the idea's payoff is unreachable.
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
