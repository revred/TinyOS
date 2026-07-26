# TinyOS — Handover Document

Status: **design phase complete for the initial seed scope; no implementation code exists yet**

## Start here

- **Original intent:** [`TinyOS26thJulySeedMVP.md`](TinyOS26thJulySeedMVP.md) — the founding statement of what TinyOS is for. Read this first; it's short and fixed, and everything else in this repository is elaboration on it.
- **Current state of the design:** [`README.md`](README.md) — the living document. If it disagrees with anything below, the README wins; this handover is a snapshot, not a source of truth.
- **How to write code here:** [`CODING_STANDARDS.md`](CODING_STANDARDS.md) — Rust-first policy, `unsafe` boundaries, real-time coding discipline, mandatory TDD, and the safety > security > correctness > performance priority ordering that governs every trade-off.

## What exists today

Nothing has been implemented yet — this repository is entirely design and specification, deliberately, so that Phase 0 starts from a coherent architecture rather than accumulating one ad hoc. What's in place:

| Area | Document | Summary |
|---|---|---|
| Vision & pillars | [`README.md`](README.md) | RTOS core, UX/control separation, host/bus connectivity, 64-bit-only hardware policy, LLM-as-supervised-operator |
| Coding standard | [`CODING_STANDARDS.md`](CODING_STANDARDS.md) | Rust-primary language policy, `no_std`/`std` split, unsafe policy, real-time discipline, mandatory TDD, priority ordering, tooling standard |
| Same-machine host comms | [`docs/hbp-spec.md`](docs/hbp-spec.md) | Host Bridge Protocol — Windows/Linux ↔ TinyOS on one box (CNC controller reference case) |
| Wireless/remote comms | [`docs/wci-spec.md`](docs/wci-spec.md) | Wireless Command Interface — co-bot over WiFi, mutual-TLS auth, single-writer command authority lease |
| Deploy tooling | [`docs/deploy-protocol.md`](docs/deploy-protocol.md) | P2P Ethernet / WiFi hot-deploy and reboot-deploy, A/B partition boot with rollback |
| Heterogeneous compute | [`docs/inference-architecture.md`](docs/inference-architecture.md) | GPU admission control, Unified Memory Manager, hosting an Ollama-like runtime, daisy-chained distributed inference |
| Shell command surface | [`docs/cli-compatibility-mvp.md`](docs/cli-compatibility-mvp.md) | TINYCMD MVP — DOS and POSIX syntax over one canonical verb core |
| Reference-only source | [`MsDOS/`](MsDOS) (git submodule) | Microsoft's officially released MS-DOS source, kept for historical command-behavior reference only — not built upon |

## Key decisions already made (don't relitigate without reason)

- **64-bit only.** No 32-bit boot path, ever, on either x86_64 or ARM64. See README §4.
- **Rust-primary.** Assembly limited to boot/context-switch glue; C limited to isolated `-sys` vendor-driver bindings. See `CODING_STANDARDS.md`.
- **Remote control is the primary UX**, not a fallback — HBP and WCI aren't optional add-ons, they're how the device is meant to be operated and developed against from day one (including deploy tooling).
- **No privileged bypass, ever**, for any caller — human shell, remote host, wireless controller, or LLM agent. Everything routes through the ACI policy engine.
- **Fail-safe over keep-trying**, everywhere: HBP/WCI link loss, deploy failures, GPU/inference stalls, watchdog trips — all default to a safe hold state, never a retry loop against a real-time deadline.
- **MVP hardware chosen:** Jetson Orin Nano Super (8GB) for ARM64/GPU/inference validation, plus a budget x86_64 mini-PC (N100/N305 class) for GPU-independent RT validation and host-bridge testing. Not yet purchased as of this handover.
- **Three deployment modes defined:** Inference-only, Real-Time control, and Inference + Real-Time Execution — more may be added later, each defined by which ACI capability classes it exposes.

## What's genuinely open / not yet decided

These are flagged as open questions in their respective specs — don't assume an answer exists:

- Exact wire/frame layout and versioning handshake for HBP, WCI, and the deploy protocol (all currently "draft, fields TBD").
- Certificate rotation/revocation mechanics for WCI when a device is offline at rotation time.
- Authority-lease preemption policy defaults (can a `supervisor` session always preempt an `operator` lease, or must it request?).
- Whether a wired daisy-chain (CAN/USB/Ethernet) needs the same mutual-TLS trust model as WCI, or can rely on physical-chain trust.
- Tensor/pipeline partitioning strategy for distributed inference, and its relationship to Roadmap Phase 8 (Fleet mode).
- Session mode selection heuristics for TINYCMD (explicit switch vs. auto-detect from input syntax).

## Immediate next steps (Roadmap Phase 0)

Per the README roadmap, the next concrete work is **Phase 0 — Kernel skeleton**: boot, context switch, preemptive priority scheduler, static memory pools, and a minimal HAL for one x86_64 target — built test-first per the TDD mandate, against the MVP hardware once purchased. Phase 1.5 (deploy tooling) is intentionally pulled early in the roadmap because remote deploy is meant to be the actual development loop, not a later convenience — worth standing up as soon as there's something to deploy.

## For whoever (or whatever) picks this up next

Read the seed document first, then the README, then whichever `docs/` spec is relevant to the task at hand. The priority ordering in `CODING_STANDARDS.md` (safety > security > correctness > performance) should resolve almost any design disagreement that comes up during implementation — when in doubt, that's the tiebreaker.
