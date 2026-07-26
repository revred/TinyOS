# Heterogeneous Compute & Distributed Inference — Draft Spec

Status: **draft / spans Roadmap Phase 6 (LLM integration) and Phase 8 (Fleet mode)**

## Purpose

TinyOS is designed to host a local LLM runtime (Ollama or an Ollama-compatible runtime) as a supervised operator, per the README's Design Pillar 5 and the [Agent Command Interface](README.md#5-llm-as-a-supervised-operator-not-a-root-user). This document specifies the hardware and software architecture that makes that practical on real target devices: GPU/VRAM-equipped hardware, shared CPU/GPU memory, and inference workloads split across multiple daisy-chained TinyOS nodes.

The governing constraint carried in from the rest of the system: **inference and GPU work never jeopardizes the CPU-side real-time guarantees.** Everything below is designed around that boundary, not in spite of it.

## Hardware assumptions

Per the [Target Hardware & Test Matrix](README.md#target-hardware--test-matrix), TinyOS targets are 64-bit x86_64 or ARM64 systems that may include:

- A discrete or integrated GPU with dedicated VRAM (laptop/NUC dGPU, or Jetson-class integrated GPU).
- Unified or shared memory architectures where CPU and GPU address the same physical memory pool (e.g. Jetson's unified memory, or laptop platforms with resizable BAR / shared system RAM exposed to the GPU).
- Optional additional accelerators (NPU) alongside the GPU.
- Multiple such devices connected in a chain over CAN, USB, or Ethernet (see [Inter-OS Communication](README.md#inter-os-communication-the-host-bridge-protocol-hbp) and [Remote Control](README.md#remote-control-the-wireless-command-interface-wci) for the transport patterns this reuses).

## Compute admission model

GPU/accelerator work is fundamentally different from CPU RT-task work: it is throughput-oriented, has coarser and less predictable completion latency, and is usually scheduled by vendor firmware/driver logic outside TinyOS's own scheduler. TinyOS treats it accordingly:

- GPU submission queues are managed by **admission control**, not the hard real-time scheduler. A GPU/inference task requests a resource budget (VRAM footprint, expected submission rate) and is admitted, throttled, or rejected by policy — it is never given scheduler-level priority that could compete with an RT task's deadline.
- The RT kernel's own execution is never blocked waiting on a GPU submission, fence, or driver call. Any code path that waits on GPU completion runs in a non-RT task context.
- This mirrors the same isolation pattern used for the shell (Design Pillar 2) and for the LLM agent itself (Design Pillar 5) — a consistent rule applied to a new kind of caller.

## Unified Memory Manager

Where the underlying hardware supports it, TinyOS exposes a **Unified Memory Manager (UMM)** so CPU-side and GPU-side code can share buffers without redundant copies:

- Buffers are represented as typed, ownership-tracked handles, not raw pointers passed across the CPU/GPU boundary.
- A buffer has a single writer at a time; the UMM enforces explicit hand-off (fencing) between CPU and GPU access rather than allowing silent concurrent access — this is the same discipline as the RT kernel's own memory model (deterministic, no surprise aliasing), applied to heterogeneous memory instead of just CPU memory.
- On hardware without true unified memory, the UMM falls back to an explicit copy path (host RAM ↔ VRAM) behind the same handle API, so higher-level code (the agent runtime, inference driver) doesn't need to know which memory model the target device has.
- All UMM code that touches vendor driver APIs lives in `-sys` binding crates per [`CODING_STANDARDS.md`](../CODING_STANDARDS.md#language-policy); the UMM's own ownership/fencing logic is safe Rust built on top of that boundary.

## Hosting an Ollama-like runtime

- The inference runtime is hosted as its own isolated task/partition (Design Pillar 5), resource-budgeted like any other TinyOS citizen: CPU time budget, VRAM budget, and admission-controlled GPU submission rate.
- Model loading, prompt/response handling, and tool-call dispatch all cross into TinyOS exclusively through the ACI — a locally hosted model has no more privilege than a remote agent connecting over [WCI](README.md#remote-control-the-wireless-command-interface-wci); the transport differs, the policy gate does not.
- The runtime's tool-calling surface maps 1:1 onto ACI capabilities (Roadmap Phase 6): every action the model can request corresponds to a pre-registered, typed, rate-limited capability, with the same provenance logging as any other caller.

## Distributed inference across daisy-chained nodes

For workloads too large for a single device's VRAM, or for deliberately partitioned deployments (e.g. a coordinator node plus several edge worker nodes), TinyOS nodes can chain together to split inference work:

- **Transport reuse, not a new stack.** Node-to-node inference traffic reuses the same transport and lane discipline as HBP/WCI: a fixed-size, versioned binary framing over CAN/USB/Ethernet (wired chain) or WCI-style authenticated TLS (wireless chain) — no bespoke unauthenticated protocol for compute traffic.
- **Roles.** One node acts as **coordinator**, holding the full model manifest and orchestrating shard dispatch and result aggregation. Other nodes act as **workers**, holding only the shard(s) they were assigned (tensor-parallel or pipeline-parallel split, depending on model and link bandwidth).
- **Compute lane.** A third lane, alongside the existing command and telemetry lanes, carries shard dispatch requests and partial results between coordinator and workers. Like the telemetry lane, it is decoupled from any RT command traffic sharing the same physical link.
- **Admission and budgets travel with the shard.** A worker admits a shard under the same admission-control model described above — it can refuse or downscale a shard request if its local VRAM/compute budget doesn't have room, rather than accepting and then failing mid-inference.
- **Failure semantics.** If a worker drops mid-inference, the coordinator either retries with a smaller/degraded configuration (fewer shards, smaller model, CPU-only fallback) or fails the inference request cleanly and reports it through the ACI — it never blocks or retries indefinitely against a CPU RT deadline on any node in the chain. Consistent with the rest of the system: **inference degrades or fails; it does not hang, and it never touches another node's real-time guarantees to try to recover.**

## Open questions

- Exact tensor/pipeline partitioning strategy and how it's negotiated (static config vs. coordinator-computed based on advertised worker capability).
- Whether the compute lane needs its own flow-control/backpressure scheme distinct from the telemetry lane's fire-and-forget model, given partial-result payloads are likely much larger than a telemetry frame.
- Security model for the wired-chain (CAN/USB/Ethernet) case — WCI's mutual-TLS/authority-lease model is specified for wireless; the wired equivalent (trusted physical chain vs. requiring the same authentication) needs its own decision.
- Relationship to Roadmap Phase 8 (Fleet mode): is a distributed-inference chain a specialization of fleet mode, or a distinct mechanism that fleet mode later subsumes?

## Status

This document accompanies the [Repository Layout](README.md#repository-layout-planned)'s `/agent/` component and extends Roadmap Phases 6 and 8. It will be refined as GPU/UMM driver work and the ACI capability registry mature enough to validate these assumptions against real hardware.
