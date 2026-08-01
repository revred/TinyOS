# Handover 02A — `docs/tinytile-architecture.md` Delivered: the One 01A Deliverable Still Owed

**The session [`01C`](01C-next-steps-after-tinytile-planning.md) Step 2 permitted, executed
2026-08-01.** Design document only — no crate, no `extern "C"` shim, no device code, per the 01A
mandate §9 and 01C trap 2. The spine is green after the change (28 Features / 70 Stories / 54
Tests / 59 Reports, unchanged).

## What was produced

**[`docs/tinytile-architecture.md`](../../docs/tinytile-architecture.md)** — downstream of
`ADR 0012`/`ADR 0013`, upstream of any Feature decomposition. Contents per the mandate §7, all
present:

1. **The tile programming model in `no_std` terms** — the model lives off-target (full TileLang/CUDA
   authoring) and on-target (declared, admission-checked quantities only); POD descriptors with
   compile-time bounds, pool/caller-supplied storage, UMM handles, the clause-4 interpreter as data.
2. **The C ABI surface (v1)** — six boundary rules (caller-owned buffers with capacities, stable
   integer error codes, no panic across the boundary, generation-tagged opaque handles, non-RT
   calling contexts only, additive-only v1 stability statement) and six entry-point families.
   Deliberately absent from v1 is named too: no compilation, no raw pointers, no completion
   callbacks, no graphics.
3. **The Tiny Kernel Artifact, field by field** — 64-byte header, canonical fixed-width manifest
   (mandatory CPU-fallback variant, rollback counter, provenance block, bounded buffer schema),
   variant table, purpose-bound signature block, and the `TOS64-TKA/1` transport envelope which
   carries **zero trust** (every field re-derived at admission). Byte widths are explicitly the v1
   proposal that `FEAT-P6B-02` pins test-first.
4. **The admission path walked gate by gate** — all fourteen `RCG-*` rows given their
   TKA-concrete meaning, including the Stage 1 nuance at `RCG-10` (the broker host's kernel does
   the device-visible mapping, post-admission, and evidence says so) and the clause-5 fail-safe
   ladder as distinct telemetry events and error codes.
5. **The device-backend seam** — one trait boundary, three backends (CPU reference as conformance
   oracle and fail-safe floor; HBP broker labelled `stage=brokered` per 01C trap 3; native Orin
   gated on the driver finding and `ADR 0013` qualification).
6. **The RT non-interference argument stated to be attacked** — seven numbered falsifiable claims,
   each with mechanism and the refutation experiment that becomes a `BND-*`/`TEST-*` obligation at
   `FEAT-P6B-04` decomposition. NI-1..NI-4 closed by construction; NI-5 (memory
   bandwidth/cache), NI-6 (DMA), NI-7 (thermal) honestly per-platform measured, with the
   composite claim never quotable unqualified — until measured, TinyTile alongside hard-RT work is
   **unqualified**, parallel to `ADR 0013`'s zero-copy default.

## Concurrency note (rule 7 of `CONCURRENT_SESSIONS.md`)

This session found the 01B session's artifacts **uncommitted** in the working tree: `EPIC-P6B.md`,
both ADRs, `01B`/`01C`, the `backlog.md` row, and the `index.html` entries for 01B/01C. Per rules
1 and 3 they were left unstaged and uncommitted — they are the other session's (or the owner's
pending-review) work. This session's commit stages only its own files:
`docs/tinytile-architecture.md` and this handover. The `index.html` entry for 02A was added in the
working tree but **not staged**, because staging that file by path would sweep the 01B session's
uncommitted entries into this commit; whoever commits the 01B artifacts commits the index with
them.

## What the next session does — and does not — do

- **Step 1 of 01C is still open:** owner review of the Epic and both ADRs (and now this document,
  which is built on them — if either ADR is amended in review, §4–§7 here re-derive). The owner
  also decides the push; `main` still carries 13+ unpushed commits.
- **Step 3's two probes remain available on owner opt-in:** the `nvgpu` desk study (makes
  `FEAT-P6B-06`'s gate concrete) and the off-target sm_87 emission proof (retires the Epic's named
  unknown). Neither touches TinyOS code.
- **Does not implement.** The queue stays behind the Pi 5 hardware-evidence sprint; when it opens,
  decompose `FEAT-P6B-02` then `-01`, tests first, contracts before code, per 01C Step 4.
