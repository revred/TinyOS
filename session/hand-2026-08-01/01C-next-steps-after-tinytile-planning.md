# Handover 01C — Next Steps After the TinyTile Planning Session

**The start-here document for whatever session follows [`01B`](01B-tinytile-planned-epic-p6b-and-two-adrs.md).**
Written 2026-08-01, immediately after `EPIC-P6B.md`, `ADR 0012` and `ADR 0013` landed. Nothing here
re-opens those rulings; this orders the work that follows from them.

## 0. State you inherit

- **Decided and in-repo, do not re-derive:** [`EPIC-P6B.md`](../../goals/epics/EPIC-P6B.md) (seven
  Features enumerated, none decomposed), [`ADR 0012`](../../docs/adr/0012-device-kernels-are-admitted-code-compiled-ahead-of-time-off-target.md)
  (AOT off-target, charter binds device code harder), [`ADR 0013`](../../docs/adr/0013-zero-copy-buffer-sharing-is-conditional-on-dma-containment-qualification.md)
  (zero-copy per-platform qualified; copy path is the default).
- **Spine green** at 28 Features / 70 Stories / 54 Tests / 59 Reports; no TinyTile contract rows
  exist, deliberately.
- **`main` carries 13+ unpushed commits, most from other sessions.** Whether and when to push is
  the owner's call, not a session's. `git log origin/main..HEAD` before assuming anything, and
  `git config core.hooksPath .githooks` per [`agent/CONCURRENT_SESSIONS.md`](../../agent/CONCURRENT_SESSIONS.md).
- **The standing sprint priority is unchanged:** Pi 5 first silicon
  ([`hand-2026-07-30/08A`](../hand-2026-07-30/08A-hardware-evidence-sprint-mandate.md) binding).
  The 01A mandate was a planning exception, not a re-prioritisation. TinyTile **implementation
  does not start** until that queue opens.

## 1. Next step, in order

### Step 1 — Owner review of 01B's four artifacts *(blocks everything below)*

The Epic and both ADRs are charter-adjacent and were produced in one session. The owner reads
them, amends or accepts, and decides the push. Anything a later session builds on an unreviewed
ADR is built on sand.

### Step 2 — `docs/tinytile-architecture.md` *(planning; permitted now; the one 01A deliverable still owed)*

Downstream of both ADRs, upstream of any Feature decomposition. Contents per the mandate §7:
the tile programming model in Rust `no_std` terms; the C ABI surface (caller-owned buffers,
explicit capacities, integer errors, no panic across the boundary, v1 stability statement); the
Tiny Kernel Artifact format in field-level detail (TOS64-* envelope for transport); the admission
path walked gate by gate; the device-backend seam; and the **RT non-interference argument** stated
so it can later be tested adversarially, not just asserted. Design document, zero code.

### Step 3 — Two cheap de-risking probes *(desk/off-target; owner opts in; either can run alongside Step 2)*

1. **`nvgpu` desk study.** Read the published Jetson Linux `nvgpu`/`host1x`/`nvmap` sources and
   record what a *deliberately narrow submission-queue subset* would minimally require (channel
   setup, gpfifo submission, syncpoints, memory import). Output is a finding document that makes
   `FEAT-P6B-06`'s gate concrete — or honestly reports intractability, which the Epic already
   treats as a legitimate close.
2. **sm_87 emission proof.** Off target, off repo: compile one TileLang GEMV for `sm_87` and run
   it on a stock Jetson Linux Orin. This retires the Epic's named unknown ("TileLang's published
   validation covers no Tegra device") for the cost of an afternoon. Evidence lands as a dated
   note the Epic can cite; no TinyOS code is touched.

### Step 4 — When the implementation queue opens *(after the Pi 5 headline; not before)*

Decompose in this order, tests first, contracts before code:

- **`FEAT-P6B-02`** (Tiny Kernel Artifact + admission) — no device, no driver answer needed, and
  its hostile-artifact parser is the highest-value security surface to pin early.
- **`FEAT-P6B-01`** (C ABI + CPU reference backend) — makes every TKA executable somewhere and
  gives `FEAT-P6B-07`'s vertical proof its fallback leg.
- Each decomposition brings: Feature doc, `feature-contracts.tsv` row, Story docs +
  `story-contracts.tsv` rows, crate-map entries in `docs/mvp-delivery-strategy.md`, and `LE-35`
  open-debt initialisation for any selected domain whose subsystem doesn't exist.
- **Sequencing risk, named now:** `FEAT-P6B-05` (HBP compute broker) leans on host-bridge maturity
  and `EPIC-P4` is not decomposed. Check that dependency honestly at decomposition time rather
  than discovering it mid-Feature.

### Step 5 — Parked, deliberately

- **`APP-20` register row** — trigger recorded in the Epic's Registers section; do not add
  speculatively (it costs a test-first xtask constant bump plus a "19 targets" prose sweep).
- **The "TinyOS" name collision** (UC Berkeley's sensor-network OS) — real, pre-launch concern,
  owner's decision, not a session's.
- **`docs/inference-architecture.md` vocabulary migration** ("GPU submission" → "compute-device
  submission" etc.) — opportunistic, per the Epic's vocabulary ruling; no sweep.

## 2. Traps

1. **Do not re-open shapes B/C.** `ADR 0012` rejected them with reasons; the only door is a
   superseding ADR, and nothing has changed since yesterday to justify one.
2. **Step 2's architecture doc will want to become code.** The C ABI is the most enjoyable part to
   write and `extern "C"` shims are one keystroke from "just checking it compiles". The mandate's
   §9 still applies: contracts before code, and the sprint queue is not open.
3. **The broker stage's evidence will read better than it is.** Stage 1 numbers include a Linux
   kernel, its scheduler and its driver stack in the loop; they prove the ABI and artifact path,
   not TinyOS-native performance. Label every figure with its stage or `ADR 0013`/`ADR 0005`
   discipline is violated by drift.
4. **A `nvgpu` desk study is not a driver.** Step 3.1's output is a feasibility finding. Treating
   it as "the driver is basically understood" is exactly the imaginary-submission-path kill
   criterion the Epic carries.
