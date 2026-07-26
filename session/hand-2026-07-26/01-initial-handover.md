# Handover 01 — Initial State Snapshot

Session date: 26 July 2026
Status: **design phase — no implementation code exists yet**

## Start here

- **Founding intent & master specification:** [`TinyOS26thJulySeedMVP.md`](../../TinyOS26thJulySeedMVP.md) — Section 1 is the fixed original ambition; the rest is a comprehensive, actively-maintained specification (goal taxonomy, hardware catalog, MVP narrowing, testing, reliability, security, codebase governance).
- **Current state of the design:** [`README.md`](../../README.md) — the living document. If it disagrees with anything in this handover, the README wins; a handover is a snapshot, not a source of truth.
- **How to write code here:** [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md) — Rust-first policy, `unsafe` boundaries, real-time coding discipline, mandatory TDD, the crate-size ceiling, SOLID enforcement, and the safety > security > correctness > performance priority ordering.

## What exists today

Nothing has been implemented yet — this repository is entirely design and specification, deliberately, so Phase 0 starts from a coherent architecture rather than accumulating one ad hoc.

| Area | Document | Summary |
|---|---|---|
| Vision & pillars | [`README.md`](../../README.md) | RTOS core, UX/control separation, host/bus connectivity, 64-bit-only hardware policy, LLM-as-supervised-operator |
| Coding standard | [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md) | Rust-primary language policy, `no_std`/`std` split, unsafe policy, real-time discipline, mandatory TDD, priority ordering, tooling standard, 20K-LOC crate ceiling, SOLID enforcement |
| Same-machine host comms | [`docs/hbp-spec.md`](../../docs/hbp-spec.md) | Host Bridge Protocol — Windows/Linux ↔ TinyOS on one box (CNC controller reference case) |
| Wireless/remote comms | [`docs/wci-spec.md`](../../docs/wci-spec.md) | Wireless Command Interface — co-bot over WiFi, mutual-TLS auth, single-writer command authority lease |
| Deploy tooling | [`docs/deploy-protocol.md`](../../docs/deploy-protocol.md) | P2P Ethernet / WiFi hot-deploy and reboot-deploy, A/B partition boot with rollback |
| Heterogeneous compute | [`docs/inference-architecture.md`](../../docs/inference-architecture.md) | GPU admission control, Unified Memory Manager, hosting an Ollama-like runtime, daisy-chained distributed inference |
| Shell command surface | [`docs/cli-compatibility-mvp.md`](../../docs/cli-compatibility-mvp.md) | TINYCMD MVP — DOS and POSIX syntax over one canonical verb core |
| Driver architecture | [`docs/universal-driver-model.md`](../../docs/universal-driver-model.md) | Driver isolation, Driver Capability Interface, class drivers, unified ACPI/Device-Tree hardware manifest, honest Apple Silicon scope caveat |
| Physical AI — committed workloads | [`docs/physical-ai-reference-workloads.md`](../../docs/physical-ai-reference-workloads.md) | 5-axis CNC (flagship MVP demonstration, Fanuc-class UX, RTCP/TCPC kinematics), Wire DED robot arm, resin-curing UV array — and the shared RT primitives that let all three run on one kernel |
| Physical AI — vision-tier exploration | [`docs/extended-domain-workloads.md`](../../docs/extended-domain-workloads.md) | Ten further domains (washing machine through rotary detonation engine), tiered honestly by realism (A/B/C) — not a roadmap commitment |
| Reference-only source | [`MsDOS/`](../../MsDOS) (git submodule) | Microsoft's officially released MS-DOS source, kept for historical command-behavior reference only — not built upon |

## Key decisions already made

Don't relitigate these without a specific reason — they've each been argued through once already.

- **64-bit only.** No 32-bit boot path, ever, on either x86_64 or ARM64.
- **Rust-primary.** Assembly limited to boot/context-switch glue; C limited to isolated `-sys` vendor-driver bindings.
- **Remote control is the primary UX**, not a fallback — HBP and WCI are how the device is meant to be operated and developed against from day one.
- **No privileged bypass, ever**, for any caller — human shell, remote host, wireless controller, or LLM agent. Everything routes through the ACI policy engine.
- **Fail-safe over keep-trying**, everywhere: link loss, deploy failures, GPU/inference stalls, watchdog trips — all default to a safe hold state.
- **MVP hardware chosen:** Jetson Orin Nano Super (8GB) for ARM64/GPU/inference validation, plus a budget x86_64 mini-PC (N100/N305 class) for GPU-independent RT validation and host-bridge testing. Not yet purchased.
- **Three deployment modes defined:** Inference-only, Real-Time control, and Inference + Real-Time Execution.
- **Flagship MVP demonstration is the 5-axis CNC controller** — full-depth, no-compromises motion/interpolation/kinematics software; physical accuracy validation deferred to hardware bolt-on.
- **Crate size ceiling: 20,000 lines of code per crate, excluding tests, no exceptions.** SOLID principles, Rust-adapted, are reviewer-enforced and treated as blocking on every PR.

## What's genuinely open

Flagged as open questions in their respective specs — don't assume an answer exists yet.

- Exact wire/frame layout and versioning handshake for HBP, WCI, and the deploy protocol (all currently draft, fields TBD).
- Certificate rotation/revocation mechanics for WCI when a device is offline at rotation time.
- Authority-lease preemption policy defaults (can a `supervisor` session always preempt an `operator` lease, or must it request?).
- Whether a wired daisy-chain (CAN/USB/Ethernet) needs the same mutual-TLS trust model as WCI, or can rely on physical-chain trust.
- Tensor/pipeline partitioning strategy for distributed inference, and its relationship to Fleet mode.
- Session mode selection heuristics for TINYCMD (explicit switch vs. auto-detect from input syntax).
- Whether the Combustion/Ignition Event-Timing primitive (from `docs/extended-domain-workloads.md`) holds at rotary-detonation-engine frequencies, or needs fundamentally different scheduling guarantees — explicitly unresolved.

## Immediate next steps

Roadmap Phase 0 — Kernel skeleton: boot, context switch, preemptive priority scheduler, static memory pools, and a minimal HAL for one x86_64 target, built test-first, against the MVP hardware once purchased. Phase 1.5 (deploy tooling) is intentionally pulled early because remote deploy is meant to be the actual development loop, not a later convenience.

## For whoever (or whatever) picks this up next

Read the seed document's Section 1 first, then `README.md`, then whichever `docs/` spec is relevant to the task at hand. The priority ordering in `agent/CODING_STANDARDS.md` (safety > security > correctness > performance) resolves almost any design disagreement that comes up during implementation.
