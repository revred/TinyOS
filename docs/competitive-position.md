# Competitive Positioning & Domain Comparative Analysis

> [!IMPORTANT]
> **Claim Gate Notice (G24 / G25 Compliance)**:
> In accordance with Non-Negotiable #18 and `goals/context/landing-zones.tsv`, all comparative statements regarding performance, worst-case execution bounds, isolation overhead, or latency relative to third-party operating systems (such as FreeRTOS, Zephyr, VxWorks, QNX, seL4, Qubes OS, or Linux) and inference runtimes (such as Ollama or vLLM) are **provisional architectural hypotheses** and **claim-gated**. They remain unearned until empirical, same-hardware benchmarks with published raw timing, memory, and cycle evidence are recorded in the project's Report catalogue.

---

## Executive Overview

TinyOS operates at the convergence of three domains historically engineered by distinct teams using incompatible safety and security paradigms:

1. **Deterministic Real-Time Control** (traditional RTOS discipline).
2. **Supervised Edge & Cloud Machine Intelligence** (agentic local LLM hosting and control).
3. **Capability-Gated Secure Remote Operation** (isolated host-bridge and wireless control).

Unlike general-purpose operating systems attempting to bolt on real-time guarantees, or bare-metal RTOSes attempting to bolt on complex userlands, TinyOS establishes **one** capability-gated interface—the **Agent Command Interface (ACI)**—through which human operators, local LLM agents, host services, and network peers must pass.

---

## Domain-by-Domain Comparative Matrix

### 1. Real-Time Operating Systems (RTOS)

| System Class | Representative Systems | Architectural Focus | TinyOS Structural Differentiation | Claim Status (G24/G25) |
|---|---|---|---|---|
| **Microcontroller RTOS** | FreeRTOS, Zephyr | 16/32-bit MCUs, flat memory or basic MPU protection, C codebase. | 64-bit only (ARM64 & x86_64), Rust `no_std` core, static allocation on RT paths, WCET budget enforcement, native ACI policy gate. | *Claim-gated*: Comparative latency & jitter bounds pending physical ARM64 hardware measurements (gated on `LE-09`). |
| **Commercial Safety RTOS** | VxWorks, QNX Neutrino | Microkernel/POSIX, ISO 26262 / DO-178C certification, closed commercial. | Open capability architecture, built-in host bridge protocol (HBP), automated timing regression suite in CI. | *Claim-gated*: Certification compliance unearned; architectural WCET models subject to physical timing validation. |
| **Formally Verified RTOS** | seL4, PikeOS | Mathematical proof of correctness, ARINC 653 partitioning. | Capability-scoped memory model with built-in agent supervision, local LLM integration, and fast DOS/POSIX shell UX. | *Claim-gated*: seL4 formal proof bounds are unexcelled; TinyOS relies on Rust safety + 14 `PD-*` invariants rather than formal proof. |

### 2. Secure Operating Systems & Hardened Linux

| System Class | Representative Systems | Security Model | TinyOS Structural Differentiation | Claim Status (G24/G25) |
|---|---|---|---|---|
| **Hardened Linux** | Alpine Linux, ChromeOS | seccomp-bpf, AppArmor/SELinux, dm-verity, namespace isolation. | Sub-8MB provisional core budget, zero ambient authority, no monolithic POSIX syscall surface, non-executable remote data (`RCG-01..RCG-14`). | *Claim-gated*: Vulnerability footprint hypotheses unearned without empirical fuzzing & adversarial coverage evidence. |
| **Compartmentalized OS** | Qubes OS | Hypervisor-based VM domains (Xen). | Single real-time kernel with 5 Containment Classes (C0–C4) and 25-pair communication matrix, avoiding hypervisor latency penalty for RT loops. | *Claim-gated*: Real-time latency advantage over hypervisor microVMs unearned until same-hardware comparative tests run. |

### 3. Cloud Computing & Edge Virtualization

| System Class | Representative Systems | Isolation Paradigm | TinyOS Structural Differentiation | Claim Status (G24/G25) |
|---|---|---|---|---|
| **MicroVMs** | AWS Firecracker, Cloud Hypervisor | KVM-based minimal Rust virtual machines for serverless workloads. | Runs either bare-metal or as a co-resident guest partition via Host Bridge Protocol (HBP) over `virtio-vsock` / shared memory ring buffers. | *Claim-gated*: Boot-time and memory footprint comparisons subject to benchmark published reports. |
| **Static Partitioning** | Siemens Jailhouse | Hard CPU core splitting for mixed criticality. | Software capability engine (ACI) arbitrates resource access across cores without requiring rigid physical core isolation for all subsystems. | *Claim-gated*: Cross-core interference mitigation pending hardware validation under competing loads. |

### 4. Local LLM & Edge Inference Runtimes

| System Class | Representative Systems | Runtime Profile | TinyOS Structural Differentiation | Claim Status (G24/G25) |
|---|---|---|---|---|
| **Edge AI Runtimes** | Ollama, llama.cpp, TensorRT-LLM | C++/CUDA runtimes, dynamic VRAM allocations, unbudgeted memory bandwidth usage. | Unified Memory Manager (UMM) for ARM64 zero-copy CPU/GPU buffers; GPU work is admission-controlled, never scheduler-privileged. | *Claim-gated*: Token generation throughput and RT loop non-interference claims require physical Jetson Orin Nano test evidence. |

---

## Security & Isolation Structural Guardrails

The security model rests on two orthogonal, non-conflated structures:

1. **14 Protection Domain Invariants (`PD-01`..`PD-14`)**: Governing process isolation, W^X memory mappings, capability revocation, and remote-code exclusion.
2. **5 Containment Classes (`C0`..`C4`)**: names and purposes quoted verbatim from
   [`goals/security/containment-classes.tsv`](../goals/security/containment-classes.tsv), which is
   the catalogue `check-assurance-spine` asserts exactly — do not paraphrase them here, because a
   paraphrase that drifts is indistinguishable from a charter amendment:
   - `C0` **Root of Trust** — hardware trust anchor, boot verification, measurement, and the smallest pre-kernel transfer mechanism
   - `C1` **Trusted Kernel Core** — scheduler, MMU, IPC, capability validation, fault routing, interrupt routing, resource-budget enforcement
   - `C2` **Isolated System Service** — drivers, network and storage stacks, loaders, crypto brokers, device-facing services
   - `C3` **Sandboxed Application** — signed installed applications, local inference runtimes, shells, administrative tools
   - `C4` **Hostile Transient Domain** — downloads, unknown executables, disposable parsers, renderers, scripts, documents, and model output

---

## Summary of Unearned Comparative Claims

To maintain repository integrity, the following assertions are explicitly flagged as **unearned** until raw empirical data is published in the Report catalogue:

- **RTOS Latency Advantage**: Any claim that TinyOS achieves lower worst-case latency than FreeRTOS, Zephyr, or VxWorks on identical hardware.
- **Hypervisor Overhead Advantage**: Any claim that single-kernel C0–C4 containment outperforms Qubes/Xen VM isolation in latency without same-hardware measurement.
- **Inference Non-Interference**: Any claim that local LLM execution on Jetson Orin Nano does not introduce DRAM/L3 cache contention on real-time task cycles (pending `D17`/`D25`/`G19` hardware validation).
