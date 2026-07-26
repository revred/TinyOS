# TinyOS — 26th July Seed MVP: Master Specification

Status: **comprehensive reference document — goals, configurations, hardware, MVP narrowing, testing, reliability, security, and codebase governance**

## 0. Document Purpose and How to Read This

This document has two jobs that pull in different directions, and it's worth naming that tension up front so the structure makes sense.

The first job is to be the **founding record**: the original ambition that started TinyOS, preserved so that as the project grows across dozens of specs, hundreds of PRs, and — eventually — millions of lines of code, anyone can come back to this file and know exactly why the project exists, in the words it was first described in. That part of this document, [Section 1](#1-founding-intent-the-original-ambition-preserved), never changes.

The second job, added as the project's design work matured, is to be the **exhaustive reference**: every goal TinyOS is meant to serve, every configuration and hardware option seriously considered, the reasoning that narrows all of that down to a buildable MVP, and — because this is where projects like this most often quietly fail — precisely how reliability, security, testing, and codebase discipline are guaranteed rather than merely hoped for. That part is large by design. It is meant to be read selectively (jump to the section you need via the table of contents below) rather than linearly every time, but it is meant to be *complete* enough that no important decision about TinyOS has to be re-derived from scratch or re-argued from memory.

Everything else in the repository — [`README.md`](README.md), [`agent/CODING_STANDARDS.md`](agent/CODING_STANDARDS.md), and the specs under [`docs/`](docs/) — is either a distillation of material that lives here in full, or a deeper technical dive into one topic this document only summarizes. The latest dated handover under [`session/`](session/) is the short pointer that tells a new reader where to start; this document is what they land on once they want the full picture.

### Table of Contents

1. Founding Intent (the original ambition, preserved)
2. Executive Summary of Scope
3. Goal Taxonomy
   - 3.1 Physical AI Goals
   - 3.2 Agentic Inference Goals
   - 3.3 Remote Control Goals
   - 3.4 Real-Time Platform Goals
   - 3.5 Universal Hardware Support Goals
   - 3.6 Developer Experience & Governance Goals
4. Full Configuration & Setup Exploration
   - 4.1 Deployment Mode Matrix (expanded)
   - 4.2 Hardware Configuration Catalog
   - 4.3 Software Configuration Profiles
   - 4.4 Network & Security Configuration Profiles
5. Narrowing to the MVP Configuration
6. Testing Strategy
7. Reliability Guarantees
8. Security Guarantees
9. Codebase Governance — Crate Size and SOLID Principles
10. Roadmap Alignment
11. Glossary
12. Cross-Reference Index
13. Document Maintenance Note

---

## 1. Founding Intent (the original ambition, preserved)

This section is fixed. It records, without later elaboration, what TinyOS was originally conceived to be:

Build a real-time operating system that:

1. Can communicate with Windows or Linux running on the **same machine**, or run as an **edge device OS** — configurable over CAN bus, USB, or Ethernet.
2. **Looks and behaves like MS-DOS 4+** — a fast, legible, keyboard-driven command environment.
3. Has a **solid multitasking core** that keeps UX/UI strictly separate from real-time control, with strict rules governing how real-time actions are triggered.
4. **Loads onto any laptop of today**, down to **Jetson Nano-class edge devices**.
5. Can **host something like Ollama** to interface with local LLMs.
6. Can **take orders from an LLM** — under strict, auditable control, never as an unsupervised root user.

At the time this was written, no hardware had been purchased, no code had been written, and no repository existed — only this statement of intent. If the project's direction ever needs to be sanity-checked, this is the paragraph to check it against.

---

## 2. Executive Summary of Scope

TinyOS sits at the intersection of three domains that are usually built by entirely separate teams, with entirely separate tooling, and — historically — entirely separate and incompatible safety/security assumptions:

- **Real-time control** (the RTOS discipline: deterministic scheduling, bounded interrupt latency, watchdog/failsafe behavior), traditionally the domain of FreeRTOS/Zephyr/VxWorks-class systems.
- **Agentic AI inference** (hosting and supervising a local or remote LLM that can propose actions), a domain that barely existed as an OS-level concern before large language models became practical to run at the edge.
- **Remote, secured control** (operating and developing against a device over a network or a co-resident host OS, with the same rigor a safety-critical system demands), a domain usually associated with industrial SCADA/fieldbus systems, not general-purpose OS design.

TinyOS's central bet is that these three domains can share **one** kernel, **one** capability-gated command interface (the Agent Command Interface, ACI), and **one** coding discipline, without any of the three compromising the others — specifically, without inference or remote-control traffic ever being able to compromise real-time determinism or safety, and without real-time control code becoming so specialized that it can't also serve as a normal, DOS/POSIX-familiar command environment for a human operator.

This document exists to make that bet legible: what exactly is being attempted (Section 3), what hardware and configuration space was considered before narrowing down (Section 4), what the actual buildable MVP is (Section 5), and — because ambition without discipline produces vaporware — exactly how correctness, reliability, security, and code quality are enforced rather than assumed (Sections 6 through 9).

---

## 3. Goal Taxonomy

Six goal categories cover everything TinyOS is meant to achieve. Each is stated as a set of concrete, falsifiable goals — not aspirations — because a goal that can't be checked against isn't a goal, it's a mood.

### 3.1 Physical AI Goals

"Physical AI" here means any deployment where TinyOS is driving, sensing, or arbitrating a physical process — the CNC controller and co-bot reference cases already specified in [`docs/hbp-spec.md`](docs/hbp-spec.md) and [`docs/wci-spec.md`](docs/wci-spec.md) are the concrete instances, but the goal category is broader than those two examples.

- **G-PA-1: Deterministic actuation.** Every physical actuation command TinyOS issues has a bounded, tested worst-case latency from decision to actuation, and that bound is enforced by the scheduler, not merely observed in testing.
- **G-PA-2: Sensor-to-decision loop integrity.** Sensor input feeding a control loop is timestamped, bounded in staleness, and a control decision made on stale sensor data is treated as a fault condition, not silently accepted.
- **G-PA-3: Fail-safe as default, not exception.** Every physical-AI deployment mode has an explicitly designed safe state (hold, estop, controlled deceleration — deployment-specific), and every fault path — software fault, communication loss, power anomaly — resolves to that safe state without operator intervention required to *reach* safety (intervention may be required to *resume*).
- **G-PA-4: Hardware e-stop supremacy.** Where a physical e-stop exists, it is wired outside every software layer (kernel, ACI, network stack) and cannot be masked, delayed, or overridden by any combination of software state, consistent with the WCI spec's e-stop handling.
- **G-PA-5: Multi-actuator coordination.** TinyOS can coordinate multiple actuators/axes (as in CNC motion) with synchronized timing guarantees, not just single-actuator control.
- **G-PA-6: Physical process auditability.** Every commanded physical action and its outcome (as reported by sensors) is logged with enough fidelity to reconstruct what happened after the fact — this serves both debugging and, in regulated deployments, compliance.
- **G-PA-7: Graceful degradation under partial hardware failure.** Losing one non-critical sensor or one redundant actuator degrades capability (reduced speed, reduced precision, alarm state) rather than causing an undefined or unsafe outcome.
- **G-PA-8: One RT core serves genuinely different physical-AI workloads.** Proven concretely, not just asserted, via three reference workloads chosen to stress different combinations of the RT core's shared primitives: a 5-axis CNC controller (high kinematic complexity, mode-based process sync), a Wire DED robot arm (continuous-velocity-based process sync), and a resin-printer UV curing array (high-channel-count array output, event/window-based process sync). See [`docs/physical-ai-reference-workloads.md`](docs/physical-ai-reference-workloads.md) for the full specification and the shared-primitive architecture that makes this true.

### 3.2 Agentic Inference Goals

"Agentic inference" means TinyOS hosting an LLM (local, via something Ollama-compatible, or a supervisory relationship with an external LLM reached over a secure channel) that can *propose* actions the ACI then adjudicates.

- **G-AI-1: Local inference hosting.** TinyOS can host a locally-running quantized LLM (in the 1B–14B parameter class realistically achievable on the [MVP hardware](#523-hardware-chosen-for-mvp-and-why), scaling up on more capable hardware) as a budgeted, isolated task.
- **G-AI-2: Tool-calling mapped to capabilities.** Every action the model can request maps 1:1 onto a pre-registered ACI capability — the model never gets a generic "run arbitrary command" tool.
- **G-AI-3: No privileged bypass for AI callers.** An LLM-originated command is evaluated by the exact same policy engine, with the exact same audit trail, as a human- or remote-host-originated command — this is restated from the README's Non-Negotiables because it is the single most important agentic-inference goal and the one most tempting to compromise under deadline pressure ("just let the model touch the GPIO directly, it's faster").
- **G-AI-4: External LLM support.** Beyond a locally-hosted model, TinyOS supports a supervisory relationship with an externally-hosted LLM reached over HBP or WCI — the same ACI gating applies regardless of where the model itself runs.
- **G-AI-5: Explainable denial.** When the ACI denies an agent-requested action, the denial reason is structured and machine-readable, so the calling agent (or its human supervisor) can understand *why*, not just *that* it failed.
- **G-AI-6: Heterogeneous compute utilization.** Where GPU/VRAM/unified-memory hardware is present, inference workloads use it, admission-controlled per [`docs/inference-architecture.md`](docs/inference-architecture.md), without ever contending with CPU real-time guarantees.
- **G-AI-7: Distributed inference across daisy-chained nodes.** For models too large for one device, TinyOS nodes can split inference work across a coordinator/worker chain, per the same document, reusing HBP/WCI-style transports rather than inventing a new unauthenticated compute protocol.
- **G-AI-8: Inference degrades, it never hangs a control loop.** A stalled, failed, or resource-starved inference request returns an error through the ACI in bounded time; it is structurally incapable of blocking an RT task on any node in the system.

### 3.3 Remote Control Goals

- **G-RC-1: Remote control as primary UX.** As stated in the README's Design Pillar 2, an operator connecting over HBP or WCI has a first-class experience — not a degraded fallback compared to a local console.
- **G-RC-2: Authenticated-only command paths.** No command lane, of any transport (HBP, WCI, future transports), accepts a command from an unauthenticated or unauthorized caller — link-layer connectivity (a plugged-in cable, an associated WiFi client) is never sufficient on its own.
- **G-RC-3: Single-writer command authority where physical safety is at stake.** For any deployment where conflicting simultaneous commands could be unsafe (motion control, actuation), only one authenticated caller holds command authority at a time, per the WCI authority-lease model.
- **G-RC-4: Fleet-scale remote control.** Beyond single-device HBP/WCI, TinyOS supports coordinating multiple devices under one remote policy/audit plane (Roadmap Phase 8), so a fleet of edge devices is operable as a fleet, not as N independent connections a human has to juggle.
- **G-RC-5: Remote control survives partial connectivity loss gracefully.** A dropped link resolves to the same fail-safe state as any other fault (G-PA-3), and reconnection never silently resumes a prior command stream (per WCI's explicit no-silent-resume rule).
- **G-RC-6: Remote deploy and reboot as a control-plane primitive, not an afterthought.** Per [`docs/deploy-protocol.md`](docs/deploy-protocol.md), updating or rebooting a device is itself a remotely-controlled, authenticated, audited action — not a separate, less-rigorous tool bolted on next to the "real" control system.

### 3.4 Real-Time Platform Goals

- **G-RT-1: Preemptive, priority-based scheduling** with bounded interrupt latency and documented priority-inversion avoidance (priority inheritance/ceiling protocols).
- **G-RT-2: Deterministic memory behavior.** No unbounded heap fragmentation or allocation-time variance on any RT-scheduled path.
- **G-RT-3: WCET-aware task model.** Every RT task declares a worst-case execution time budget; the scheduler and the CI timing regression suite both hold code to that budget.
- **G-RT-4: UX/UI strictly outside the trust and timing boundary.** A hung or crashed shell, TUI, or web console never delays or corrupts a scheduled task (README Design Pillar 2, Non-Negotiable #1).
- **G-RT-5: DOS-familiar, POSIX-familiar operator experience.** A human operator gets a fast, legible, keyboard-driven shell that supports both DOS-style and POSIX-style command syntax against one canonical command core, per [`docs/cli-compatibility-mvp.md`](docs/cli-compatibility-mvp.md).
- **G-RT-6: Plain-text, versionable configuration.** System configuration is inspectable, diffable text — never a hidden binary registry.
- **G-RT-7: 64-bit-only, two-architecture portability.** The kernel, HAL, and every driver run correctly on both committed architectures (x86_64 and ARM64) from shared source, not architecture-forked implementations.

### 3.5 Universal Hardware Support Goals

- **G-HW-1: Driver isolation.** A driver fault never faults the kernel, per the [Universal Driver Model](docs/universal-driver-model.md) — drivers run outside the RT trust boundary, capability-scoped like any other ACI caller.
- **G-HW-2: Class-driver baseline for common hardware.** Storage, network, HID, display, GPU-compute, CAN, and sensor devices work at baseline functionality with zero vendor driver installed, via mandatory generic class drivers.
- **G-HW-3: Stable driver contract independent of kernel internals.** The Driver Capability Interface is versioned and stable, so a driver written today keeps working across kernel releases within that DCI version's support window.
- **G-HW-4: Unified hardware description.** ACPI (x86_64/Intel-chipset) and Device Tree/SBSA/EBBR (ARM64) are normalized into one internal hardware topology model, so higher layers never branch on firmware description format.
- **G-HW-5: Honest hardware scope.** Committed hardware support (Intel/AMD PC-class + ARM64 boards with standard firmware description) is clearly distinguished from best-effort support (Apple Silicon, pending public hardware interfaces), per the Universal Driver Model — no overselling parity that doesn't exist.
- **G-HW-6: GPU/VRAM and unified-memory support.** Where present, a Unified Memory Manager provides zero-copy CPU/GPU buffer sharing on true-unified-memory hardware, falling back to an explicit-copy path elsewhere, behind one API.

### 3.6 Developer Experience & Governance Goals

- **G-DX-1: Rust-primary, with a documented and minimal non-Rust footprint** (boot glue, isolated vendor `-sys` bindings only).
- **G-DX-2: Mandatory test-driven development** for every feature, with adversarial tests required for every safety- and security-relevant subsystem.
- **G-DX-3: Remote-first, secure development loop** — peer-to-peer Ethernet or WiFi deploy/hot-deploy as the default way to iterate against real hardware, per [`docs/deploy-protocol.md`](docs/deploy-protocol.md).
- **G-DX-4: A strict, never-relaxed priority ordering** for every design and code trade-off: safety, then security, then correctness, then performance.
- **G-DX-5: Bounded crate size** — no crate exceeds 20,000 lines of code excluding tests, enforced by CI, so the codebase never accumulates an incomprehensible monolith.
- **G-DX-6: Strict, reviewer-enforced SOLID principles**, adapted to idiomatic Rust, treated as blocking on every PR — detailed in full in [Section 9](#9-codebase-governance--crate-size-and-solid-principles).
- **G-DX-7: Performance as a genuine, measured goal**, pursued only once G-DX-2 through G-DX-4 hold, with the explicit intent of extracting the maximum throughput and lowest latency the target hardware allows.

---

## 4. Full Configuration & Setup Exploration

This section deliberately casts a wide net — every deployment mode variant, hardware category, and configuration axis that was seriously considered — before [Section 5](#5-narrowing-to-the-mvp-configuration) narrows it down. The point of documenting the wide net, not just the final answer, is so that "why didn't we just do X" has a written answer instead of requiring the decision to be re-argued from memory every time it comes up.

### 4.1 Deployment Mode Matrix (expanded)

The README documents three deployment modes at a summary level. Here they're expanded with the sub-configurations considered within each.

#### Mode 1: Inference-only

| Sub-configuration | Description | Status |
|---|---|---|
| Local-model, single-device | Device hosts and serves a local quantized model over HBP/WCI, no external LLM dependency | In MVP scope |
| Proxy/relay mode | Device relays requests to an externally-hosted LLM over a secured uplink, applying local ACI policy to the *response*-driven actions but not the inference itself | Considered, deferred post-MVP |
| Multi-tenant inference | Multiple authenticated sessions query the same hosted model concurrently, each capability-scoped independently | Considered, deferred — depends on ACI multi-session maturity |

#### Mode 2: Real-Time control

| Sub-configuration | Description | Status |
|---|---|---|
| Single-axis / single-actuator | One controlled degree of freedom (e.g. a single-axis positioner) | In MVP scope (simplest RT validation case; the first milestone toward the 5-axis CNC below) |
| **5-axis CNC controller (Fanuc-class UX)** | Multi-axis coordinated motion with full RTCP/TCPC kinematics, G-code interpretation, work coordinate systems, tool compensation | **Flagship MVP demonstration, full depth, no compromises on the motion/kinematics software** — see [`docs/physical-ai-reference-workloads.md`](docs/physical-ai-reference-workloads.md) |
| Wire DED robot arm | Multi-axis serial-arm motion with continuous-velocity-based process synchronization (wire feed, energy source) | In MVP scope, architecture-validated depth (proves primitive generality; not built to full production depth at MVP) — see [`docs/physical-ai-reference-workloads.md`](docs/physical-ai-reference-workloads.md) |
| Resin-printer UV curing array | Near-trivial motion (one lift axis) with high-channel-count, event/window-synchronized array output | In MVP scope, architecture-validated depth (same rationale as Wire DED) — see [`docs/physical-ai-reference-workloads.md`](docs/physical-ai-reference-workloads.md) |
| Sensor-arbitration-only (no actuation) | Device only ingests/arbitrates sensor data, issuing no physical commands — a lower-risk RT validation mode | Considered as an even-lighter-weight bring-up mode; folded into single-axis mode's early milestones rather than kept separate |

Physical validation (encoder feedback, drive hardware, wire-feed/energy-source hardware, UV array hardware) for all three of the above is explicitly deferred until that hardware is bolted onto the MVP compute pair (Section 5.3) — the software/architecture commitment is not deferred; see [`docs/physical-ai-reference-workloads.md`](docs/physical-ai-reference-workloads.md#what-no-compromises-means-precisely) for exactly where that line is drawn.

#### Mode 3: Inference + Real-Time Execution

| Sub-configuration | Description | Status |
|---|---|---|
| Inference-proposes, RT-executes (co-bot pattern) | Model proposes a motion/action; RT task executes only after ACI approval | In MVP scope |
| Inference-monitors, RT-executes autonomously | RT control runs independently; inference only observes and can raise alerts, never directly command | Considered as a lower-risk variant; useful for early combined-mode testing before granting the model command-adjacent capabilities |
| Full closed-loop agentic control | Model both proposes and receives outcome feedback in a tight loop, adjusting subsequent proposals | Explicitly post-MVP — highest-risk mode, deferred until G-AI-2 through G-AI-5 are proven independently |

#### Modes considered and explicitly not adopted (yet)

- **Fully autonomous mode (no ACI gate for a designated trusted agent).** Rejected outright, not just deferred — this directly violates G-AI-3 and Non-Negotiable #2. No future mode may exempt any caller from the ACI, regardless of how "trusted" it's claimed to be.
- **Headless-forever mode (no local shell ever provisioned).** Considered for pure appliance deployments, but rejected as a *build-time* mode — TINYCMD's presence doesn't compromise anything (it's gated identically to remote callers per G-RC-1/G-RT-4), so removing it saves nothing and costs debuggability. A deployment can simply not expose local console *access* (a provisioning-time policy choice) without removing the capability from the build.

### 4.2 Hardware Configuration Catalog

This is the broad survey of hardware categories considered relevant to Physical AI, Agentic Inference, and Remote Control deployments — the full space [Section 5](#5-narrowing-to-the-mvp-configuration) narrows down from. Entries describe device *categories*, not endorsements of specific commercial products beyond what's needed to make the category concrete.

#### 4.2.1 Physical AI / robotics-adjacent compute hardware

| Category | Representative characteristics | Relevance to TinyOS |
|---|---|---|
| Industrial PC (fanless, DIN-rail) | x86_64, passive cooling, extended temperature range, multiple isolated I/O | Natural Tier-2-class target for factory-floor RT control deployments; no GPU typically |
| Single-board computer (SBC), ARM64 | Compact, low power, GPIO-rich | Tier-1-class edge target; varies widely in GPU capability |
| Motion controller add-on boards | Dedicated stepper/servo driver ICs, often exposed over SPI/CAN | Not a TinyOS host itself, but a peripheral the CAN/USB driver stack must speak to |
| CAN transceiver modules | Physical-layer CAN 2.0B/CAN-FD interface hardware | Required peripheral for any Physical AI deployment using CAN, per README Design Pillar 3 |
| Co-bot arm controllers | Vendor-specific, usually closed | TinyOS interfaces with these as an external actuator system via CAN/USB/Ethernet, not by replacing their firmware |

#### 4.2.2 Agentic inference / GPU-VRAM-capable hardware

| Category | Representative characteristics | Relevance to TinyOS |
|---|---|---|
| ARM64 SoC with integrated GPU and unified memory (Jetson-class) | CUDA-capable integrated GPU, shared CPU/GPU physical memory | Primary reference platform for the Unified Memory Manager (true-unified-memory path); see [`docs/inference-architecture.md`](docs/inference-architecture.md) |
| x86_64 laptop/mini-PC with discrete GPU | Dedicated VRAM, PCIe-attached | Reference platform for the UMM's explicit-copy fallback path |
| x86_64 with no GPU | CPU-only inference, much smaller/quantized models only | Still valid for Inference-only mode at reduced model scale; useful as a worst-case/lower-bound test target |
| Multi-GPU workstation-class hardware | Multiple discrete GPUs, high VRAM | Out of scope for edge deployment but relevant as a Tier-3-equivalent development/CI accelerator for training conformance tests against larger models, not a target for the OS itself at MVP |

#### 4.2.3 Remote control / connectivity hardware

| Category | Representative characteristics | Relevance to TinyOS |
|---|---|---|
| WiFi radio modules (WPA2/3-capable) | Standard 802.11 chipsets with driver support in the class-driver baseline | Required peripheral for WCI |
| Wired Ethernet (onboard or add-on NIC) | Standard 802.3 | Required for HBP loopback fallback, deploy protocol's P2P Ethernet path, and general Ethernet connectivity per README Design Pillar 3 |
| USB host/device controllers | Standard xHCI/EHCI-class controllers | Required for USB stack (README Design Pillar 3) and for deploy-tooling-over-USB as a future consideration (not in MVP scope) |
| Cellular/LTE-5G modules | Add-on modems | Considered for fleet-scale remote deployments (Roadmap Phase 8 relevance); explicitly out of MVP scope |

#### 4.2.4 Sensor and actuator I/O categories

| Category | Representative characteristics | Relevance to TinyOS |
|---|---|---|
| GPIO-exposed digital I/O | Simple on/off sensing/actuation | Baseline I/O class driver target |
| Analog sensor interfaces (ADC) | Temperature, pressure, position feedback | Class driver target; feeds G-PA-2's sensor-to-decision loop integrity goal |
| Encoder/position feedback interfaces | Quadrature or absolute encoders | Required for closed-loop motion control validation |
| Camera/vision sensors | USB or MIPI-CSI class | Relevant to future Physical AI perception work; explicitly out of MVP scope (no vision pipeline specified yet) |

#### 4.2.5 Power and enclosure considerations

- **Bench/development power**: standard USB-PD or barrel-jack supply, no special consideration beyond what the chosen dev boards ship with.
- **Field/industrial power**: wide-input DC (9–36V), surge/reverse-polarity protection — relevant to eventual field deployment, not to MVP bring-up, which runs on bench power.
- **Enclosure/thermal**: fanless/passive cooling preferred for field Physical AI deployments (fewer moving parts to fail); active cooling acceptable for bench/dev hardware and for Agentic Inference deployments where sustained GPU load requires it.

### 4.3 Software Configuration Profiles

Each deployment mode (Section 4.1) maps to a feature-flag profile at build time — TinyOS does not ship one monolithic binary with every subsystem always present, both for the crate-size discipline in [Section 9](#9-codebase-governance--crate-size-and-solid-principles) and because an Inference-only deployment genuinely has no business including RT-motion-control code in its trusted computing base at all.

| Profile | Kernel scheduler | ACI capability set | Inference host | Driver set |
|---|---|---|---|---|
| `inference-only` | Present (still real-time-capable, just lightly loaded) | Inference-related capabilities only | Present, budgeted | Storage, network, HID (no actuator classes) |
| `rt-control` | Present, fully loaded | Motion/actuation-related capabilities only | Absent entirely (not just idle — not compiled in) | Storage, network, HID, CAN, actuator classes |
| `rt-inference-combined` | Present, fully loaded | Union of the above, admission-controlled per G-AI-8 | Present, budgeted, admission-controlled | Union of the above |

Feature-flagging out an entire subsystem (rather than just leaving it unconfigured) is itself a security and reliability measure: an `rt-control`-profile device has no inference code in its binary at all, so there is no inference-related attack surface or bug class to worry about on that class of device, full stop.

### 4.4 Network & Security Configuration Profiles

| Profile | Applies to | Authentication model |
|---|---|---|
| Co-resident (HBP) | Same-machine host bridge (Windows/Linux ↔ TinyOS) | Physical trust anchor at pairing time; ACI capability scope per caller |
| Wireless (WCI) | Remote/networked callers (co-bot reference case) | Mutual TLS via device-issued certificates, physical-access provisioning only, ACI capability scope per session |
| Deploy | Peer-to-peer Ethernet or WiFi deploy/hot-deploy | Same as HBP/WCI respectively, scoped to a distinct `deployer` capability |
| Development/bring-up (QEMU/Renode, Tier 0) | CI and local emulated testing only | Simplified/disabled auth acceptable **only** in the emulated Tier 0 environment, which never touches real hardware or real credentials — explicitly not a pattern that's allowed to leak into Tier 1/2 configuration |

---

## 5. Narrowing to the MVP Configuration

### 5.1 Selection Criteria

Given everything cataloged in Section 4, the MVP configuration is chosen against four criteria, applied in this order:

1. **Coverage.** The MVP must exercise all three deployment modes and both committed architectures — otherwise "MVP" would just mean "the easy 20%."
2. **Cost.** Economical enough to purchase without a large capital commitment before the design has been validated against real silicon at all.
3. **Isolation of variables.** Where possible, one board should isolate one variable (e.g. RT determinism without a GPU confusing the picture) so a test failure has an obvious, narrow cause.
4. **Reuse.** Hardware chosen for the MVP should remain useful as permanent CI/bring-up infrastructure post-MVP, not be a throwaway evaluation purchase.

### 5.2 Modes and profiles in MVP scope

From Section 4.1: **single-axis/single-actuator RT control** (as the first bring-up milestone), the **5-axis CNC controller** (as the flagship, full-depth MVP demonstration), the **Wire DED robot arm** and **resin-printer UV curing array** (as architecture-validated reference workloads proving the RT core's primitives generalize — see [`docs/physical-ai-reference-workloads.md`](docs/physical-ai-reference-workloads.md)), **local-model single-device inference**, and **inference-proposes/RT-executes combined mode** are all in scope. Proxy/relay inference, multi-tenant inference, full closed-loop agentic control, and physical hardware validation (encoders, drives, wire-feed/energy-source, UV array hardware) for all three Physical AI workloads are explicitly deferred — each is called out in Section 4.1 or in `docs/physical-ai-reference-workloads.md` as post-MVP or hardware-dependent, and none is silently dropped without a documented reason.

### 5.3 Hardware chosen for MVP, and why

Two boards, one per committed architecture:

**NVIDIA Jetson Orin Nano Super Developer Kit (8GB)** — ARM64, CUDA-capable GPU with unified CPU/GPU memory.

- Satisfies Criterion 1 (coverage): the only board in the catalog that lets Inference-only, RT-control, and combined mode all run on the *same physical device*, which is required to validate G-AI-8 (inference never blocks RT) as a real property rather than an assumption — that property can only be falsified by trying to violate it on hardware where both subsystems genuinely coexist.
- Satisfies Criterion 2 (cost): sits at the accessible end of GPU-capable ARM64 dev hardware.
- Satisfies Criterion 3 partially: because it has a GPU, it cannot in isolation prove that RT determinism holds *independent of* any GPU/inference variable — hence the second board.
- Satisfies Criterion 4: remains the permanent ARM64/GPU CI and bring-up target well past MVP; nothing about it is MVP-only infrastructure.

**A budget x86_64 mini-PC / NUC-class box (Intel N100/N305-class), no discrete GPU**

- Satisfies Criterion 3 directly: RT-control mode validated here has no GPU variable to confuse a timing result — a timing regression measured on this board is unambiguously a scheduler/kernel issue, not a GPU-contention artifact.
- Doubles as the Tier 2 host-bridge (HBP) target per the README's Target Hardware & Test Matrix, satisfying Criterion 4.
- Low cost, satisfying Criterion 2.
- Deliberately does *not* attempt to cover Inference-only or combined mode — that's intentional, not a gap: those modes are already covered by the Jetson, and asking one board to do everything would violate Criterion 3.

**Total spend**: roughly $400–470 for both boards, against a catalog (Section 4.2) that included options from under $100 (Raspberry Pi-class, rejected for lacking GPU capability needed for G-AI-1/G-AI-6 validation) up to multi-GPU workstation-class hardware (rejected as out of scope for edge-deployment validation at MVP, though noted as relevant future CI infrastructure).

**Explicitly not purchased for MVP, with reasons:**

- A dedicated microcontroller board for "pure RT" timing validation — rejected because it would be a non-64-bit target, violating [Non-Negotiable §4](README.md#4-runs-where-the-work-happens--64-bit-only)'s 64-bit-only policy, and because TinyOS's bare-metal design already produces real determinism numbers on the two chosen boards without needing a third architecture class.
- A second, non-NVIDIA ARM64 board (e.g. a Raspberry Pi-class SBC) for HAL portability checking — deferred, not rejected; it remains on the Target Hardware & Test Matrix's Tier 1 list as a near-term follow-up once the Jetson-based bring-up is stable, specifically to catch "quietly grew Jetson-only assumptions" bugs per the README's existing caveat.
- Cellular/LTE modules, camera/vision sensors, multi-axis motion controller hardware — all explicitly deferred per their entries in Section 4.2, consistent with the deferred sub-configurations in Section 4.1.

### 5.4 What "MVP" means here, precisely

The MVP is not "a demo." It is the smallest hardware and software configuration that can **falsify** every goal in Section 3 that's in scope — i.e., for each in-scope goal, there exists a test (Section 6) that would fail on this configuration if the goal weren't actually met. A goal that can't be tested on the MVP configuration is either out of MVP scope (and documented as such, per Section 4) or the MVP configuration is wrong. This framing — MVP as a falsification rig, not a showcase — is deliberate, and ties directly into Section 6.

---

## 6. Testing Strategy

Testing is not a phase that happens after implementation — per [`agent/CODING_STANDARDS.md`](agent/CODING_STANDARDS.md#test-driven-development-mandatory), every feature is built test-first, and this section describes the full taxonomy of tests TinyOS uses and how they map onto the goals in Section 3 and the hardware tiers already defined in the [Target Hardware & Test Matrix](README.md#target-hardware--test-matrix).

### 6.1 TDD discipline recap

Red, green, refactor, for every feature, with no exceptions for "trivial" code, and mandatory adversarial tests for every safety- and security-relevant subsystem — this is specified in full in `agent/CODING_STANDARDS.md` and is not repeated here beyond this pointer, because that document is the authoritative source and this document should not risk drifting out of sync with it by duplicating its detail.

### 6.2 Test taxonomy

| Test type | Purpose | Primary goals validated |
|---|---|---|
| **Unit tests** | Verify individual function/type behavior in isolation, co-located with the code per `agent/CODING_STANDARDS.md` | All — the baseline for every crate |
| **Integration tests** | Verify correct behavior across module/crate boundaries within one binary | G-RT-1 through G-RT-7, G-HW-1 through G-HW-4 |
| **Property-based tests** | Generate a wide range of inputs against an invariant (e.g. "the scheduler never assigns two RT tasks overlapping exclusive time on the same core") rather than fixed examples | G-RT-1, G-RT-2, G-PA-1 |
| **Fuzz testing** | Feed malformed/adversarial byte streams into any parser that accepts external input — HBP/WCI frame parsers, TINYCMD's DOS/POSIX front-ends, deploy image validation | G-RC-2, G-HW-3, all frame-parsing code |
| **Mutation testing** | Deliberately mutate implementation code and confirm the test suite catches the mutation — a check on test *quality*, not just test *presence* | Applied selectively to safety/security-critical crates (ACI policy engine, HBP/WCI auth, watchdog) given its cost |
| **Timing regression / WCET benchmarks** | Measure worst-case execution time for every RT task and fail CI on regression, per Roadmap Phase 1 | G-RT-1, G-RT-3, G-PA-1 |
| **Adversarial/security tests** | Actively attempt to violate a security invariant: unauthenticated command, expired session, replayed frame, capability-scope escalation, path traversal in shell commands | G-RC-2, G-RC-3, G-AI-3, all of Section 8 |
| **Golden-file / acceptance tests** | For TINYCMD, run each MVP verb through both DOS and POSIX front-ends against a fixture and assert equivalent underlying action, per `docs/cli-compatibility-mvp.md` | G-RT-5 |
| **Driver conformance suites** | Every class driver and vendor extension runs the same conformance suite for its device class, per the Universal Driver Model | G-HW-2, G-HW-3, Liskov Substitution (Section 9.2) |
| **Chaos / fault-injection tests** | Deliberately kill a task, drop a link mid-session, corrupt a deploy transfer, starve a resource — and assert the system reaches its documented safe state, not an undefined one | G-PA-3, G-PA-7, G-RC-5, all of Section 7 |
| **Hardware-in-the-loop (HIL) tests** | Run against real Tier 1/2 hardware (the MVP boards), not emulation, for anything where QEMU/Renode can't faithfully represent timing or peripheral behavior | Final validation gate for every goal before a feature is considered done on real hardware |
| **Soak / burn-in tests** | Run a configuration continuously for an extended period under representative load, watching for slow leaks, drift, or rare-event failures that short tests miss | G-PA-3, G-AI-8, general reliability (Section 7) |

### 6.3 CI pipeline stages

1. **Format & lint** — `rustfmt` check, `clippy -D warnings`, crate-size ceiling check (Section 9.1), `missing_docs` check.
2. **Unit + integration tests** — run on every PR, every crate.
3. **Property/fuzz (scoped)** — run on every PR for crates touched by the PR; full-corpus fuzz runs on a scheduled cadence (not every PR, to keep PR turnaround fast) with any newly-found crash added to the regression corpus immediately.
4. **Tier 0 emulated tests (QEMU/Renode)** — run on every PR; this is the primary, fast feedback loop per the Target Hardware & Test Matrix.
5. **Timing regression suite** — run on every PR touching RT-path code; a regression is a hard CI failure, equal in severity to a functional test failure, per Non-Negotiable #4.
6. **Adversarial/security suite** — run on every PR touching ACI, HBP, WCI, deploy, or watchdog code.
7. **Tier 1/2 HIL tests** — run on a merge-to-main cadence against the actual MVP hardware (not every PR, since hardware runners are a shared, slower resource), with results visible before a release is cut.
8. **Mutation testing** — run on a scheduled cadence against the designated safety/security-critical crate list, not on every PR, given its runtime cost.
9. **Soak tests** — run continuously against a standing HIL rig, independent of the PR cadence, with any failure treated as a release blocker regardless of when it's discovered.

### 6.4 Coverage & quality gates

- Code coverage is tracked per crate, but is treated as a **diagnostic signal, not a target to game** — a crate with 100% line coverage and no adversarial or property-based tests is worse than a crate with 80% coverage that includes both, and review judgment overrides a raw coverage number.
- A PR is not blocked purely on a coverage percentage threshold; it is blocked on the TDD process requirement (tests present before/with implementation) and on the specific test-type requirements in Section 6.2 for the kind of code it touches (e.g. new frame-parsing code without a corresponding fuzz target is blocked, regardless of what the line-coverage number says).

### 6.5 Test environment matrix mapped to hardware tiers

| Tier | Environment | Test types run here |
|---|---|---|
| Tier 0 | QEMU x86_64/ARM64, Renode | Unit, integration, property, fuzz (scoped), most adversarial tests, golden-file |
| Tier 1 | Jetson Orin Nano Super (MVP) | Driver conformance (GPU/UMM path), timing regression (ARM64), combined-mode chaos tests, soak tests |
| Tier 2 | x86_64 mini-PC (MVP) | Timing regression (x86_64, GPU-isolated), HBP host-bridge integration, driver conformance (non-GPU classes) |

This mirrors, and is the testing-specific elaboration of, the [Target Hardware & Test Matrix](README.md#target-hardware--test-matrix) already defined in the README.

---

## 7. Reliability Guarantees

Reliability is treated as an engineered property with specific mechanisms behind it, not a claim made in prose. Each subsection below states the mechanism and how it's verified (cross-referencing Section 6).

### 7.1 Failure model and watchdog design

- TinyOS assumes that any given task — RT or non-RT, driver or application-level — can fault, hang, or misbehave, and designs the *system* to remain safe under that assumption rather than trying to make every individual component unconditionally fault-free (an impossible bar).
- A kernel-level watchdog monitors RT task liveness against each task's declared WCET budget (G-RT-3); a task that overruns is treated as faulted and handled per its configured fault policy (restart, degrade, or trip the system to its safe state), never silently ignored.
- Watchdog behavior itself is chaos-tested (Section 6.2) by deliberately inducing task hangs and asserting the correct fault policy fires within its bounded detection time.

### 7.2 A/B boot and rollback

- Per [`docs/deploy-protocol.md`](docs/deploy-protocol.md), kernel-core updates use A/B partition boot with an automatic, bounded-time boot-health check; failure to pass rolls back to the last-known-good partition without requiring manual recovery.
- This mechanism is itself soak- and chaos-tested: repeated deploy-and-intentionally-break cycles must reliably recover, not just "usually" recover.

### 7.3 Redundancy patterns for Physical AI / RT control

- Where a deployment has redundant sensors or actuators, TinyOS's task model supports graceful degradation (G-PA-7) rather than an all-or-nothing failure — losing one redundant input degrades reported confidence/precision rather than halting the control loop outright, unless the specific deployment's safety policy demands a full stop on any redundancy loss (a deployment-time policy choice, not a hardcoded kernel behavior).
- The MVP hardware configuration (Section 5.3) does not include redundant actuators by default — this pattern is specified for when a deployment's hardware includes them, and validated in the test suite via simulated redundancy loss in Tier 0 (Renode) even before real redundant hardware is purchased.

### 7.4 Formal methods consideration (lightweight, targeted)

- Full formal verification of the entire kernel is not an MVP commitment — it's expensive and the tooling/expertise investment isn't justified across the whole codebase at this stage.
- However, the highest-consequence invariants — the ACI policy engine's "no privileged bypass" property, and the scheduler's "no two RT tasks hold conflicting exclusive resources" property — are strong enough, narrow enough, and consequential enough candidates for targeted formal or semi-formal verification (e.g. model checking a simplified state-machine representation of the ACI decision logic) that this is tracked as a deliberate future investment, not ruled out. This is recorded here specifically so it isn't lost as an idea between now and when the ACI policy engine is mature enough (Roadmap Phase 5) to apply it to.

### 7.5 Reliability metrics and error budgets

- Once Tier 1/2 HIL soak testing (Section 6.5) is running continuously, TinyOS tracks: mean time between watchdog trips, mean time between unplanned safe-state transitions, and deploy success/rollback rate, as the core reliability metrics.
- No numeric MTBF target is fixed in this document prematurely — setting a target before any soak-test baseline exists would be a number pulled from nowhere. The first soak-test results establish the baseline; subsequent targets are set as measurable improvements against that baseline, not against an arbitrary industry figure that may not be meaningful for TinyOS's specific hardware and workload.

---

## 8. Security Guarantees

### 8.1 Threat model

TinyOS's threat model assumes: an attacker may have network access to a WCI-exposed device (it's WiFi-reachable by design); an attacker may have physical access to a device in some deployments (a co-bot in a shared workspace is not physically locked away the way a data-center server is); a dependency in the supply chain may be compromised; and an LLM agent — local or external — may be manipulated (via prompt injection or otherwise) into requesting actions it shouldn't. The security architecture is built assuming all four of these will actually be attempted, not merely theorized about.

### 8.2 Capability-based security recap

The Agent Command Interface is TinyOS's single security boundary: every caller (human shell, HBP host, WCI remote controller, local or external LLM agent) is authenticated, resolved to a capability scope, and every action is checked against that scope before execution, with full audit provenance. This is specified in depth in the README (Design Pillar 5) and in `docs/hbp-spec.md`/`docs/wci-spec.md`; this section states the security *guarantee* that architecture produces: **no caller, regardless of type, has an implicit or bypassable privilege** — this is Non-Negotiable #2, restated here because it is the load-bearing security guarantee everything else in this section depends on.

### 8.3 Supply chain security

- **Dependency minimalism.** Per `agent/CODING_STANDARDS.md`, every new dependency in a `no_std` crate is justified in its introducing PR — both audit surface and binary size matter on edge targets, and an unjustified dependency is a review blocker, not a nitpick.
- **Reproducible builds.** The full system image builds deterministically from a pinned toolchain (`rust-toolchain.toml`) and a locked dependency set (`Cargo.lock` committed, not gitignored), so a given commit always produces a byte-identical (or at minimum, behavior-identical) build — a prerequisite for trusting that what's deployed matches what was reviewed.
- **Dependency auditing.** Automated dependency vulnerability scanning (`cargo-audit`-class tooling) and license/policy scanning (`cargo-deny`-class tooling) run in CI on every dependency change, not just on a periodic schedule — a newly disclosed vulnerability in an existing dependency should be caught the next time CI runs against it, not discovered months later during an ad hoc review.
- **Software Bill of Materials (SBOM).** Every released image ships with a generated SBOM, so a downstream deployer (or TinyOS's own incident response process, Section 8.6) can immediately determine whether a given disclosed vulnerability affects a specific deployed device.

### 8.4 Secure boot and attestation

- The A/B boot mechanism (Section 7.2) is extended with image signature verification: a partition is only booted if its signature validates against a trust anchor held in hardware/firmware, not just checked by software that could itself be tampered with.
- Remote attestation — a device being able to cryptographically prove to a WCI/HBP caller which image it's currently running — is specified as a goal for the deploy protocol's maturity but is explicitly flagged as an open question in `docs/deploy-protocol.md` pending the signature scheme decision; this document does not resolve that open question, it inherits it.

### 8.5 Key and credential management

- WCI client certificates and HBP pairing credentials are issued only through physical-access provisioning (Section 3.3, G-RC-2's underlying mechanism) — there is no network-reachable "add a new trusted device" endpoint, by design, per the WCI spec.
- Certificate/credential rotation and revocation are handled locally on the device (via TinyOS's own shell, not a remote/network path) specifically so that revoking a compromised credential doesn't itself depend on network trust that might be the thing that's compromised.
- No long-lived shared secrets are used anywhere in the HBP/WCI/deploy trust model — every credential is device- and session-scoped, consistent with the threat model's assumption that any given secret may eventually leak.

### 8.6 Incident response and disclosure process

- A security-relevant defect (in TinyOS's own code or in a dependency, per Section 8.3's scanning) is triaged against the priority ordering in `agent/CODING_STANDARDS.md`: security is priority 2, ahead of any feature work, and a confirmed vulnerability with a plausible exploitation path in a deployed configuration is treated as a release-blocking issue for any pending release, and a hotfix-worthy issue for already-deployed devices via the same deploy-protocol mechanism used for any other update.
- Given the project's current design-only stage, a formal public disclosure policy (CVE process, disclosure timeline commitments) is deferred until there is shipped code for it to apply to — but the *mechanism* for fixing and deploying a fix quickly (the deploy protocol) is being built early (Roadmap Phase 1.5) specifically so that mechanism already exists by the time it's needed.

### 8.7 Security testing cadence

- Adversarial tests (Section 6.2) for ACI, HBP, WCI, and deploy code run on every PR touching those subsystems — not periodically, because these are exactly the subsystems where a regression is most consequential.
- A broader, periodic adversarial review (effectively an internal red-team pass against the current HIL hardware) is planned once Tier 1/2 hardware and the ACI capability registry are both mature enough (post Roadmap Phase 5) to make such a review meaningful rather than premature.

---

## 9. Codebase Governance — Crate Size and SOLID Principles

This section is called out by name because it governs *how the entire codebase is built*, not just one subsystem — every other section in this document describes a goal or property of the finished system; this section describes the discipline that has to hold on every single PR, forever, for those goals to remain true as the codebase grows past what any one person can hold in their head.

### 9.1 Crate size ceiling

**No crate may exceed 20,000 lines of code, excluding test code.** This is stated here as policy; the authoritative, enforceable specification — measurement method, CI enforcement, splitting strategy, and a worked example — lives in [`agent/CODING_STANDARDS.md`](agent/CODING_STANDARDS.md#crate-size-ceiling-hard-limit-no-exceptions), and this document defers to that one for detail rather than duplicating and risking drift between the two.

The reason this matters enough to restate here, at the level of a project-wide goal rather than just a style rule: TinyOS's entire safety and security architecture depends on components being genuinely comprehensible and genuinely isolated (Section 8.2's capability model, the Universal Driver Model's driver isolation, Section 7's fault containment). A 40,000-line crate is not just harder to review — it's a crate where the isolation boundaries the rest of this document assumes have almost certainly eroded, because nothing forced anyone to keep them clean. The ceiling is a structural precondition for every other guarantee in this document, not an independent nicety.

### 9.2 SOLID principles, Rust-adapted, never compromised

Again, the authoritative enforceable specification — the five principles translated into Rust idiom, and their per-principle enforcement mechanisms — lives in [`agent/CODING_STANDARDS.md`](agent/CODING_STANDARDS.md#solid-principles--rust-adapted-never-compromised). This section states why "never compromised" is the right bar rather than "encouraged" or "best-effort":

- **Single Responsibility** is what keeps the crate-size ceiling (9.1) achievable in the first place — a codebase of well-factored, single-purpose types splits naturally along crate boundaries; a codebase that ignores Single Responsibility hits the 20K-line ceiling by accident, with no clean seam to split along, which is exactly the failure mode 9.1's ceiling is meant to force out into the open early.
- **Open/Closed** is what makes the Universal Driver Model's class-driver/vendor-extension pattern (G-HW-2, G-HW-3) actually work in practice — adding a vendor extension must never require editing the generic class driver's code, or every vendor extension becomes a fork risk.
- **Liskov Substitution**, enforced via shared conformance test suites, is the concrete mechanism that makes G-HW-3 ("a driver written against DCI v1 keeps working") a testable claim rather than a hope — a Liskov violation is precisely a driver that technically implements the trait but doesn't honor its contract, which is exactly the failure mode that quietly breaks driver portability over time.
- **Interface Segregation** is a direct security property here, not just a code-quality one: a narrow trait is a narrow capability, and G-HW-1's driver isolation guarantee is only as strong as the traits drivers are actually granted — a driver depending on an overly broad trait has more attack surface than its actual job requires, regardless of what the ACI's declared capability scope says.
- **Dependency Inversion** is, concretely, the architectural pattern the ACI/HBP/WCI/shell relationship already commits to (Section 8.2) — a high-level policy engine that depends on concrete transport types instead of a `Caller` abstraction would make G-AI-3/G-RC-2's "same gate for every caller" guarantee an accident of current code structure rather than an enforced property, which is precisely the gap Dependency Inversion closes.

None of these five principles is optional or "nice to have" in TinyOS's specific context — each one is load-bearing for a guarantee already made elsewhere in this document (Sections 3, 7, and 8), which is the concrete reason "never compromised" is the correct bar rather than aspirational guidance.

### 9.3 Additional governance

- **Coupling/cohesion as a review signal.** Beyond the five SOLID principles, reviewers watch for high coupling between crates that should be independent (e.g. a `drivers-storage` crate that reaches into `kernel` internals rather than going through the DCI) as an early warning that a boundary is eroding before it becomes a crate-size or SOLID violation outright.
- **Documentation coverage.** `#![deny(missing_docs)]` on library crates (per `agent/CODING_STANDARDS.md`) is treated as governance, not decoration — a public API without a documented invariant is exactly the kind of interface where a Liskov violation or a capability-scope mistake goes unnoticed, because there's no written contract to check a change against.
- **Dependency minimalism as governance, not just security.** Section 8.3's dependency-justification requirement also serves crate-size and SOLID discipline: a dependency pulled in to avoid writing 200 lines of well-factored code is sometimes the right call, but a dependency pulled in because it was easier than doing Single Responsibility properly is exactly the kind of shortcut this section exists to prevent.

---

## 10. Roadmap Alignment

This table maps every Roadmap phase (as specified in [`README.md`](README.md#roadmap)) to the goals, hardware, and testing this document specifies, so "what does Phase N actually need to prove" has one clear answer.

| Roadmap Phase | Goals primarily validated | MVP hardware involved | Key test types |
|---|---|---|---|
| Phase 0 — Kernel skeleton | G-RT-1, G-RT-2, G-RT-7, G-HW-4 | Tier 0 (QEMU), then both MVP boards | Unit, integration, Tier 0 |
| Phase 1 — Determinism proof | G-RT-1, G-RT-3, G-PA-1 | Both MVP boards | Timing regression, property-based |
| Phase 1.5 — Deploy tooling | G-RC-6, G-DX-3 | Both MVP boards | Adversarial (deploy), chaos (interrupted deploy) |
| Phase 2 — Shell & UX | G-RT-5, G-RT-6 | Both MVP boards | Golden-file/acceptance |
| Phase 3 — Connectivity | G-HW-2, G-PA-4 (via CAN/e-stop wiring where applicable) | Both MVP boards + peripheral hardware (Section 4.2.3/4.2.4) | Driver conformance, Tier 0 (Renode) |
| Phase 4 — Host bridge | G-RC-1, G-RC-2 | x86_64 mini-PC (HBP target) | Adversarial (HBP auth), integration |
| Phase 5 — Agent Command Interface | G-AI-2 through G-AI-5, G-RC-2, G-RC-3 | Both MVP boards | Adversarial, property-based (policy engine) |
| Phase 6 — LLM integration | G-AI-1, G-AI-2, G-AI-3 | Jetson Orin Nano Super | Integration, adversarial |
| Phase 6b — Heterogeneous compute | G-AI-6, G-HW-6 | Jetson Orin Nano Super | Driver conformance (UMM), timing regression (admission control isolation) |
| Phase 7 — Edge bring-up | G-HW-1 through G-HW-5 | Jetson Orin Nano Super | Driver conformance, HIL |
| Phase 8 — Fleet mode | G-RC-4, G-AI-7 | Multiple units of both MVP board types (post-MVP purchase) | Integration, chaos (partial fleet connectivity loss) |

The **5-axis CNC flagship demonstration** (G-PA-8) is not a single phase — it's the integration milestone that Phases 0, 1, 2, and 3 build toward jointly (RT scheduler, timing determinism, TINYCMD/G-code front-end, and connectivity all have to land before simultaneous 5-axis contouring is demonstrable end-to-end), with the Wire DED and resin-curing reference workloads following as architecture-validation checkpoints on the same timeline rather than separate phases of their own.

---

## 11. Glossary

- **ACI** — Agent Command Interface; the single capability-gated policy engine every caller (shell, HBP, WCI, agent) routes through.
- **DCI** — Driver Capability Interface; the versioned, stable contract drivers are written against, independent of kernel internals.
- **HBP** — Host Bridge Protocol; the same-machine channel between a co-resident host OS (Windows/Linux) and TinyOS.
- **WCI** — Wireless Command Interface; the authenticated, networked channel for remote/wireless callers (the co-bot reference case).
- **UDM** — Universal Driver Model; the overall driver architecture (userspace isolation, class drivers, DCI, unified hardware manifest).
- **UMM** — Unified Memory Manager; the abstraction sharing CPU/GPU memory buffers, with a true-unified and an explicit-copy path.
- **WCET** — Worst-Case Execution Time; the bounded timing budget every RT task declares and is held to.
- **TCB** — the `.TCB` batch script file extension used by TINYCMD's DOS-flavored scripting.
- **Class driver** — a mandatory, vendor-independent driver providing baseline functionality for a device class (storage, network, HID, etc.).
- **Vendor extension** — an additive, optional capability layer on top of a class driver, never required for baseline function.
- **Deployment mode** — a build-time-selected profile (Inference-only, Real-Time control, Inference + Real-Time Execution, …) defining which ACI capabilities and subsystems are present.
- **SOLID** — Single Responsibility, Open/Closed, Liskov Substitution, Interface Segregation, Dependency Inversion; five design principles, Rust-adapted and enforced without exception per Section 9.2.

---

## 12. Cross-Reference Index

Every document in the repository, one line each, for quick navigation from this master specification:

- [`README.md`](README.md) — the living, current-state design document; the first place to check if anything here seems out of date.
- [`session/`](session/) — dated handover documents, one folder per calendar date (`session/hand-YYYY-MM-DD/`), each with an `index.html` indexing that date's numbered `NN-*.md` handover write-ups in order; see [`session/README.md`](session/README.md) for the naming convention. Start with the most recent dated folder's `index.html`.
- [`agent/CODING_STANDARDS.md`](agent/CODING_STANDARDS.md) — the authoritative, binding coding rules: language policy, priority ordering, crate size ceiling, SOLID enforcement, TDD mandate, tooling standard.
- [`docs/hbp-spec.md`](docs/hbp-spec.md) — Host Bridge Protocol wire-level spec (same-machine host comms).
- [`docs/physical-ai-reference-workloads.md`](docs/physical-ai-reference-workloads.md) — the 5-axis CNC (flagship MVP demonstration), Wire DED robot arm, and resin-curing UV array reference workloads, and the shared RT primitives that let all three run on one kernel.
- [`docs/extended-domain-workloads.md`](docs/extended-domain-workloads.md) — vision-tier exploration of how far the architecture generalizes (washing machine through rotary detonation engine), tiered honestly by realism; not a roadmap commitment.
- [`docs/wci-spec.md`](docs/wci-spec.md) — Wireless Command Interface wire-level spec (remote/wireless comms, co-bot reference case).
- [`docs/deploy-protocol.md`](docs/deploy-protocol.md) — peer-to-peer Ethernet/WiFi deploy and reboot protocol, A/B partition boot.
- [`docs/inference-architecture.md`](docs/inference-architecture.md) — GPU admission control, Unified Memory Manager, distributed daisy-chained inference.
- [`docs/cli-compatibility-mvp.md`](docs/cli-compatibility-mvp.md) — TINYCMD DOS + POSIX command MVP spec.
- [`docs/universal-driver-model.md`](docs/universal-driver-model.md) — driver isolation, DCI, class drivers, unified hardware manifest, the Apple Silicon scope caveat.
- [`docs/mvp-delivery-strategy.md`](docs/mvp-delivery-strategy.md) — the concrete Cargo workspace crate map, custom target specs, `xtask` build/deploy tooling, and the phased walking-skeleton delivery strategy from an empty repository to the CNC flagship milestone.
- [`MsDOS/`](MsDOS) (submodule) — Microsoft's officially released MS-DOS source, kept as a historical command-behavior reference only, not built upon.

---

## 13. Document Maintenance Note

Unlike [Section 1](#1-founding-intent-the-original-ambition-preserved), which is fixed permanently, the rest of this document is expected to be revised as the project's design work continues — new hardware evaluated, new goals added, MVP scope adjusted based on what Phase 0/1 actually discover on real hardware. When it is revised, the change should be reflected here directly (this document is the canonical, exhaustive reference) and, where relevant, distilled back into the shorter [`README.md`](README.md) so a reader who only wants the current-state summary never has to read this entire document to find it. This document's size and thoroughness are intentional — a project spanning real-time control, agentic AI, and remote-controlled physical hardware has enough genuine surface area that a short document would necessarily be an oversimplification of decisions that carry real safety and security weight.
