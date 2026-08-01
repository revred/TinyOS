# Handover 01B — TinyTile Planned: `EPIC-P6B` Written, Two ADRs Accepted

**The session [`01A`](01A-tinytile-planning-session-mandate.md) commissioned, executed 2026-08-01.**
Contracts and decisions only — no crate, no `extern "C"` shim, no device code, per the mandate §9.
The spine is green after every change (28 Features / 70 Stories / 54 Tests / 59 Reports unchanged).

## What was produced

1. **[`ADR 0012`](../../docs/adr/0012-device-kernels-are-admitted-code-compiled-ahead-of-time-off-target.md)**
   — settles the mandate's §5. Device kernels are admitted code and the charter binds them
   *harder* than CPU code; kernels are compiled ahead of time, off target (shape A); no compiler
   ever runs on a TinyOS device; a bounded interpreted fallback is permitted as data; a missing
   variant resolves fail-safe. Shapes B and C-as-hot-path are rejected with reasons.
2. **[`ADR 0013`](../../docs/adr/0013-zero-copy-buffer-sharing-is-conditional-on-dma-containment-qualification.md)**
   — settles §6.1 the `ADR 0005` way. `PD-10` governs; zero-copy is a per-platform qualified
   capability with a positive-control qualification record; the explicit-copy path is the honest
   default; qualified-platform count today: **zero**.
3. **[`goals/epics/EPIC-P6B.md`](../../goals/epics/EPIC-P6B.md)** — the Epic document the backlog
   slot has waited for. Seven Features enumerated (not decomposed, per the just-in-time rule): C
   ABI + CPU reference backend, Tiny Kernel Artifact format + admission, off-target toolchain
   contract, queue/fence/telemetry runtime, **HBP compute broker (Stage 1)**, native Orin backend
   (Stage 2, gated), and a vertical proof — one quantized GEMV/dequant kernel served through the
   inference runtime on the `G-AI-9` axes. [`backlog.md`](../../goals/epics/backlog.md) row updated.

## The two findings that shaped it

- **The driver question (§6.2) has a staged answer, not an unknown.** On the Orin the iGPU is
  driven by the source-available `nvgpu`/`host1x`/`nvmap` stack (MIT core, published in the Jetson
  Linux kernel sources) — a documented submission-interface reference; nouveau/NVK/GSP is the
  discrete-GPU route, not the Orin's. Stage 1 puts the accelerator behind a Linux-hosted C2 broker
  over HBP (the `G-AI-7` transport pattern), proving ABI, artifact, admission and telemetry before
  any native driver exists. TileLang's published validation covers no Tegra device — sm_87
  emission is recorded as a named unknown.
- **The register join already exists — no `APP-20` row was added.** `LZ-02` carries `G-AI-6` and
  `APP-03` declares the C2 GPU-broker seam TinyTile implements. The register enforces an exact
  count (`APPLICATION_PLATFORM_COUNT = 19`, machine-checked, quoted in prose), so a new row is a
  test-first xtask change plus a prose sweep; the Epic records the precise trigger that would make
  `APP-20` mandatory instead of paying that cost speculatively.

## What the next session on this Epic does — and does not — do

- **Does not implement.** Implementation queues behind the hardware-evidence sprint (Pi 5 first
  silicon) and behind `EPIC-P6`. The mandate's remaining deliverable —
  `docs/tinytile-architecture.md` (C ABI surface, artifact format detail, RT non-interference
  argument) — is the natural next planning artifact, downstream of both ADRs.
- **First Feature to decompose when the queue opens:** `FEAT-P6B-02` (artifact + admission) or
  `FEAT-P6B-01` (ABI + CPU backend) — both landable with no device and no driver answer.
- Feature/Story contract rows, crate-map entries, and any `open-debt.tsv` initialisations happen
  at decomposition time, none exist yet, deliberately.
