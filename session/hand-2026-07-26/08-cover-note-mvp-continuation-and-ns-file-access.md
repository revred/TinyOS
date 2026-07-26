# Cover Note — MVP Continuation and Nanosecond-Class File Access

This note sits at the end of today's handovers (numbered `08` so it sorts last, after [Handover 07](07-phase-0-walking-skeleton-implementation-handover.md)) because it states a mandate for the *next* session rather than a record of what this one did — same role [Cover Note 00](00-cover-note.md) played for the day, restated for where things stand now that `FEAT-P0-01` is Verified.

## Where the MVP stands

`FEAT-P0-01` (workspace bootstrap, kernel boot-to-halt under QEMU, `xtask`, CI governance gates) is Verified locally — see Handover 07 and `goals/reports/REPORT-2026-07-26-01` through `-03`. That is the walking skeleton only: no scheduler, no memory pools, no HAL/ACPI backend yet. Per [`EPIC-P0`](../../goals/epics/EPIC-P0.md), the remaining work to close this Epic is:

- **`FEAT-P0-02`** — preemptive priority scheduler core (G-RT-1). Not yet decomposed into Stories.
- **`FEAT-P0-03`** — static/pool memory allocator (G-RT-2). Not yet decomposed into Stories. **Load-bearing for the second half of this note** — see below.
- **`FEAT-P0-04`** — x86_64 HAL backend & ACPI manifest normalization (G-HW-4). Not yet decomposed into Stories.

Per the just-in-time decomposition principle, decompose each into Stories/Tests when work on it actually begins, not before. Unlike `FEAT-P0-01`'s three Stories (which Handover 07 found tightly coupled — kernel boot, `xtask`, and CI all blocked on verifying each other, making single-threaded implementation safer than a blind parallel fan-out), `FEAT-P0-02` through `FEAT-P0-04` look more genuinely independent of each other once decomposed: the scheduler, the memory allocator, and the ACPI/HAL backend don't need each other's internals to build or test in isolation, only a common booting kernel (already Verified) to run inside. **This is a better candidate for the cover note 00's original parallel-subagent mandate** than `FEAT-P0-01` turned out to be — worth deliberately trying once these three Features are decomposed into Stories, rather than defaulting back to single-threaded work out of habit.

## Nanosecond-class file access for the inference runtime

Directed this session: TinyOS's Ollama-like runtime (`os/src/inference/`, Roadmap Phase 6) should access model weights and other large on-SSD artifacts via **pointer-based access against a memory-mapped file**, not `read()`-into-a-fresh-buffer — modeled on the file-pointer/mmap pattern in `C:\Code\Sharc.Workspace\Sharc.Blue`'s `.rac` BlockStore substrate (`Sharc.Bluekind/Blue.Sharc/`, `Blue.Guard/`), where `rac-ptr`-style lookups dereference directly into a warm-mmap'd region instead of parsing/copying on every access.

This is written up in full in [`docs/inference-architecture.md`](../../docs/inference-architecture.md#model-storage--fast-load-path-mmappointer-based-not-read-and-copy)'s new "Model storage & fast-load path" section — read that before designing against it. The two things worth restating here because they're easy to lose in translation between projects:

1. **The "nanosecond" figure describes warm, page-cache-resident access, not a cold SSD read.** `mmap` pays real SSD/NVMe latency once per page on first touch (page fault), and is a bare pointer dereference — no syscall, no copy, no parse — on every access after that. The correct claim for TinyOS's model-load path is "pay SSD latency once per page, then RAM-parity dereference forever after," not "SSD access at nanosecond speed" as a literal physical claim. Get this framing right in any Phase 6 design doc that cites this note or Sharc.Blue's numbers.
2. **This depends on `FEAT-P0-03` existing first.** TinyOS's `inference` crate runs `no_std` under TinyOS's own kernel, not a hosted OS — so the memory-mapping primitive has to be TinyOS's own kernel virtual-memory manager (demand-paging a file-backed region), not `std::fs`/OS-level mmap. **`FEAT-P0-03` (static/pool memory allocator) is the direct prerequisite for this work to become buildable at all**, which is why it's called out again in the MVP-continuation section above rather than left as a purely Phase-6-scoped concern — whoever picks up `FEAT-P0-03` should know a Phase 6 consumer is waiting on its virtual-memory story, not just the pool-allocator story `FEAT-P0-03`'s own title suggests.

Pointer-based access to an untrusted model file is also a security-boundary question, not just a performance one (validate bounds/format before trusting any pointer into the mapped region, matching Sharc.Blue's own header-validation-before-pointer-trust discipline) — the ACI-mediated boundary in `README.md` Design Pillar 5 must not be quietly bypassed because the mmap path is fast. This is also recorded in project memory (`inference-mmap-model-loading` memory entry) so it survives even without this file being read first.

## What this note does not do

It does not decompose `FEAT-P0-02`/`03`/`04` into Stories, and it does not start Phase 6 design work — both are for whoever picks up the next session to trigger deliberately, the same way Cover Note 00 deferred launching Phase 0 itself. This note exists so the next session's mandate — continue the MVP toward a full `EPIC-P0`, and carry the ns-file-access direction into `FEAT-P0-03`'s scope rather than losing it as a Phase-6-only aside — is written down instead of only remembered.
