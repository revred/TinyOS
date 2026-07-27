# TinyOS

**A real-time operating system with the soul of MS-DOS and the reflexes of an RTOS — built to sit between silicon and intelligence.**

TinyOS is a from-scratch, **64-bit-only** real-time operating system designed to run on anything from a modern x86_64 laptop to an ARM64 edge device like the Jetson Orin Nano, and to speak fluently to the Windows/Linux host it lives beside or the machines it's wired to over CAN bus, USB, or Ethernet. It looks and feels like MS-DOS 4+ — a fast, legible, keyboard-driven command environment — but underneath that familiar shell is a hard-partitioned multitasking core built for deterministic, real-time control. And it's designed from day one to host a local LLM (via Ollama or an equivalent runtime) as a first-class citizen: not a chat window bolted on top, but a supervised operator that can observe, propose, and — within strict, auditable limits — act.

---

## Why

Modern edge and control software is split across two worlds that don't trust each other:

- **General-purpose OSes** (Windows, Linux) — great UX, huge driver support, terrible determinism guarantees for hard real-time work.
- **RTOSes** (FreeRTOS, Zephyr, VxWorks) — excellent determinism, but primitive UX, and rarely designed with "an LLM might be issuing commands" as a threat model.

TinyOS exists to close that gap deliberately, not accidentally: a real-time core with DOS-like ergonomics for humans, and a narrow, permissioned interface for machine agents — including LLMs — to interact with it without ever being able to touch the real-time guarantees directly.

---

## Language & Coding Standards

TinyOS is written primarily in **Rust** — the kernel, HAL, drivers, ACI, shell, and host bridge services are all Rust by default. Assembly is confined to boot/entry and context-switch glue; C is confined to isolated `-sys` binding crates wrapping vendor SDKs (GPU drivers, CAN transceiver code) that don't yet have a safe Rust equivalent. `unsafe` is forbidden at the application layer (`aci`, `agent`, `shell`) and permitted only in `hal`/`drivers`/`-sys` crates, each block justified with a `// SAFETY:` comment.

See [`CODING_STANDARDS.md`](agent/CODING_STANDARDS.md) for the full standard, including the `no_std` policy, real-time coding discipline (no allocation or unbounded blocking in scheduler/IPC/interrupt paths), and toolchain/lint requirements (`rustfmt`, `clippy -D warnings`, `#![deny(missing_docs)]`).

---

## Design Pillars

### 1. A real multitasking RTOS core

- Preemptive, priority-based scheduler with bounded interrupt latency and no unbounded priority inversion (priority inheritance/ceiling protocols from day one).
- Deterministic memory model — static or pool-based allocation in real-time paths; no surprise heap fragmentation in the control loop.
- Time is a first-class resource: every task declares its period, deadline, and worst-case execution budget. The kernel enforces them and screams (loudly, safely) when they're violated.

### 2. UX/UI strictly separated from control

- **Remote control over an established secure channel is the primary means of UX and control** — [HBP](#inter-os-communication-the-host-bridge-protocol-hbp) for a co-resident host, [WCI](#remote-control-the-wireless-command-interface-wci) for wireless/networked callers. The local `TINYCMD` console is a first-class interface but not a privileged one: it authenticates and authorizes through the same gate as any remote caller.
- The DOS-like shell (`TINYCMD`) is a **presentation and orchestration layer only**. It never has direct write access to real-time task state, drivers, or bus I/O.
- All shell and UI actions go through a narrow, versioned **Command & Control API** — the same API used by scripts, remote hosts, and the LLM agent. One gate, many callers, one audit trail.
- This means the UI can crash, hang, or be swapped out (text console today, a web dashboard tomorrow) without ever jeopardizing a real-time task that's mid-cycle.

### 3. Host and bus connectivity as a native concept

- **Host bridge**: a lightweight driver/service pair (Windows + Linux) that lets TinyOS run as a companion OS on the same machine (dual-boot, hypervisor partition, or a dedicated core on an AMP/SMP split) and exchange typed messages with host-side processes.
- **CAN bus**: native CAN 2.0B/CAN-FD stack for talking to vehicles, industrial controllers, and other embedded nodes.
- **USB**: device and host-mode USB stacks for peripherals, flashable storage, and tethered control links.
- **Ethernet**: lwIP-class TCP/IP stack for edge-to-cloud and edge-to-fleet communication.
- All transports terminate at the same internal message bus — a CAN frame, a USB packet, and a TCP message can all trigger the same command handler, subject to the same permission checks.
- **Drivers run outside the kernel's trust boundary, not inside it.** A crashing or misbehaving driver never faults the RT core — it's admitted, capability-scoped, and restarted like any other ACI-gated resource, not a privileged kernel-mode extension. See [Universal Driver Model](docs/universal-driver-model.md).

### 4. Runs where the work happens — 64-bit only

- No 32-bit targets, ever. TinyOS commits to **x86_64 and ARM64** exclusively, which simplifies the kernel's memory model, pointer/ABI assumptions, and driver interfaces from day one.
- Target hardware spans laptop-class x86_64 (as a bare-metal boot option or hosted partition) down to ARM64 edge devices such as the Jetson Orin Nano, with GPU/NPU acceleration for local inference.
- Hardware abstraction layer (HAL) keeps board-specific code in one place so the kernel, scheduler, and shell stay portable across both architectures.

### 5. LLM as a supervised operator, not a root user

- Local inference (Ollama or compatible runtime) runs in its own isolated task/partition — resource-budgeted like any other real-time citizen, never able to preempt hard-deadline control loops.
- The LLM interacts with TinyOS exclusively through the **Agent Command Interface (ACI)**: a declarative, capability-scoped API where every possible action is pre-registered, typed, rate-limited, and logged.
- **Strict rule: the LLM can request, TinyOS decides.** Every agent-issued command passes through the same policy engine as a human operator's command — no privileged bypass path exists for AI-originated actions.
- Full command provenance: every state change is tagged with *who* asked (human shell, script, remote host, or agent), *what* was requested, *what* was actually executed, and *why* the policy engine allowed it.
- On hardware with a GPU/VRAM and shared CPU/GPU memory, inference work is admission-controlled rather than scheduled on the RT path, and can be split across daisy-chained TinyOS nodes for larger models — see [Heterogeneous Compute & Distributed Inference](docs/inference-architecture.md).

### 6. Sandbox-first security with measurable absence

- [`SECURITY_CHARTER.md`](SECURITY_CHARTER.md) is the governing process-isolation and remote-code exclusion contract: 14 Protection Domain invariants, 14 mandatory code-admission gates, and a complete 25-pair C0–C4 communication matrix are checked by CI.
- Boot stages, OS updates, TXE executables, future TON libraries, and drivers are signed, content-addressed, origin-labelled, revocation-aware, and anti-rollback checked before execution.
- Every process gets a private active address space, W^X/NX mappings, guard pages, and an empty initial capability set. Cross-process memory exists only through rights-sized, revocable shared-memory grants.
- Every remote packet, host frame, download, model output, file, and deploy payload begins as non-executable C4 data. No ingress, parser, debug, compatibility, or deploy path can create executable memory; only the complete `RCG-01..RCG-14` admission chain may produce a fresh C3 process.
- Files and downloads retain origin, signer, entitlement, quarantine, and derivation labels across rename, copy, extraction, conversion, IPC, and storage. Untrusted bytes never become executable merely because they were downloaded or renamed.
- Network endpoints, active-content parsers, cookies/tracking state, and drivers are opt-in. An unselected component must contribute zero linked bytes and zero live authority—not an idle attack surface.
- The threat model includes ransomware, worms, browser/parser attacks, malicious peripherals, and a project-defined **Fable-class** frontier AI adversary capable of long-horizon adaptive exploit campaigns. Provider-side model safeguards are never trusted as the OS boundary.
- See [`docs/security-spine.md`](docs/security-spine.md), [`goals/security/`](goals/security/), and the mandatory [`goals/assurance/`](goals/assurance/) Story contracts.

### 7. Goals, performance, applications, and security steer together

- [`goals/context/landing-zones.tsv`](goals/context/landing-zones.tsv) keeps each destination's goal IDs, selected portions of the 625-test performance catalogue, concrete application workloads, security controls, containment classes, roadmap horizon, and claim gate in one machine-checked row.
- The concrete destination set includes the RT/Physical-AI core, Blue Atom and local-LLM pipelines, Wails, Tauri, .NET 10-or-later C#, Node, research-stage Bun, Dangerous Dave, DOOM, Quake II/III, a Chrome-class browser, TinySpot remote UX, TLE, WST, fleet/data-centre workloads, and a browser-hosted TinyOS laboratory.
- “Native support” has explicit levels. Only the minimal execution/protection substrate is `core-native`; most programs are signed `native-txe` or `managed-aot` C3 applications; large runtimes and browsers are optional compartment systems; Linux compatibility is a guest/personality; the browser build is a lab.
- C# is coherent with the Security Charter through .NET Native AOT, generated capability-safe bindings, hash-pinned native interop, and an OS Protection Domain. Runtime code emission, arbitrary assembly/native loading, unrestricted P/Invoke, debugger/process access, and cross-process writes remain absent.
- See [`docs/whole-system-context.md`](docs/whole-system-context.md) for the complete flight plan and [`goals/context/application-platforms.tsv`](goals/context/application-platforms.tsv) for the canonical workload contracts.

---

## Deployment Modes

A TinyOS device is configured into one of a defined set of modes at boot/provisioning time. Modes are an explicit, auditable configuration choice, not an emergent side effect of which subsystems happen to be running.

1. **Inference-only mode** — the device hosts a local (or externally supplied) LLM and serves tokens/results exclusively through an authenticated secure channel (HBP or WCI). No RT control task is active; the ACI exposes only inference-related capabilities. This is the mode for a pure "edge inference appliance" deployment with no physical actuation to protect.
2. **Real-Time control mode** — the device runs RT control tasks (motion, actuation, sensing) with no inference workload resident at all. This is the mode for the CNC/co-bot-style deployments described in HBP/WCI, where determinism is the only concern and inference capability is simply absent, not merely idle.
3. **Inference + Real-Time Execution mode** — both subsystems run concurrently on the same device: inference (local or driven by an external LLM over a secure channel) proposes actions, RT control tasks execute them, and every inference-originated command still passes through the ACI policy engine exactly as Design Pillar 5 requires. This is the mode where Non-Negotiable #6 (GPU/inference never jeopardizes CPU RT guarantees) is load-bearing rather than theoretical.

More modes may be added as deployments demand them; each new mode is defined by which capability classes the ACI exposes and which subsystems are permitted to be resident, not by ad hoc configuration flags.

### Physical AI reference workloads

Real-Time control and combined mode are validated against three deliberately different physical-AI workloads, so the RT core's motion and process-synchronization primitives are proven general rather than accidentally CNC-shaped:

1. **5-axis CNC controller — the flagship MVP demonstration.** Fanuc-class operator experience (G-code interpretation, work coordinate systems, tool compensation, override dials, alarm/diagnostics), full RTCP/TCPC kinematics for simultaneous 5-axis contouring. The motion/interpolation/kinematics software is built to full, no-compromises correctness from the start; physical positional-accuracy validation is deferred until real encoders and drives are bolted onto the MVP compute hardware.
2. **Wire DED robot arm** — a directed-energy-deposition additive process where wire feed rate and energy-source power must track instantaneous path velocity, not just fire on a timer, with the energy source gated by a motion-active safety interlock.
3. **Resin-printer UV curing array** — near-trivial motion (one lift axis) paired with a high-channel-count, precisely-timed UV array output gated by an exposure-window safety interlock.

See [`docs/physical-ai-reference-workloads.md`](docs/physical-ai-reference-workloads.md) for the full specification, including the shared RT primitives (Motion & Interpolation Service, Process-Synchronized Output Service, Position Feedback Abstraction, Safety Interlock) that let all three workloads run on one kernel rather than three bespoke control subsystems.

Beyond these three committed reference workloads, [`docs/extended-domain-workloads.md`](docs/extended-domain-workloads.md) explores how far the same architecture generalizes — from near-term-credible domains (a washing machine, an automatic transmission, engine valve timing, a drone flight controller) to deliberately-labeled extreme-tail cases (a sea-landing rocket controller, a rotary detonation engine) that are included to be honest about the architecture's ceiling, not as roadmap commitments.

---

## Target Hardware & Test Matrix

TinyOS is **64-bit only** — no 32-bit boot path is planned or supported, on either architecture.

**Committed hardware scope:** x86_64/Intel-and-AMD-chipset PCs and ARM64 boards that expose a standard hardware description (ACPI or Device Tree/SBSA/EBBR) — this includes Windows-PC-class laptops/NUCs and Jetson-class or comparable ARM64 SBCs. **Apple Silicon is explicitly tracked as best-effort, not committed**, because Apple does not publish public hardware interfaces for it; see the [Universal Driver Model](docs/universal-driver-model.md#the-apple-silicon-constraint-stated-plainly) for why this isn't a design gap TinyOS can architect around on its own.

### Tier 0 — Emulated (CI gate, every commit)

- **QEMU x86_64** (`q35` machine type) and **QEMU ARM64** (`virt` machine type) — the primary dev loop; kernel, scheduler, and ACI changes are validated here before any real hardware is touched.
- **Renode** — bus/peripheral simulation for CAN, USB, and Ethernet driver work ahead of physical hardware availability.

### Tier 1 — Edge device (primary mission target)

- **Jetson Orin Nano** (ARM64) — the standard edge target for new hardware bring-up; its integrated GPU and unified CPU/GPU memory make it the reference platform for the Unified Memory Manager and GPU-accelerated local inference (Ollama) validation — see [Heterogeneous Compute & Distributed Inference](docs/inference-architecture.md).
- A second, non-NVIDIA ARM64 board (e.g. Raspberry Pi 4/5) — portability check so the HAL doesn't quietly grow Jetson-only assumptions. Not expected to have comparable GPU/VRAM capability; used for CPU-side/HAL portability, not inference validation.

### Tier 2 — Laptop / x86_64 (host-bridge + full UX validation)

- A mid-spec x86_64 laptop or NUC-class mini-PC, dual-boot or hypervisor-partitioned — validates the Windows/Linux host bridge and the DOS-style shell UX, not just the kernel.
- At least one Tier 2 machine should carry a discrete GPU with dedicated VRAM, to validate the Unified Memory Manager's non-unified (explicit copy) fallback path against the Jetson's true-unified-memory path.

### MVP hardware (bring-up / evaluation, to be purchased)

For an economical first pass covering all three [Deployment Modes](#deployment-modes) across both supported architectures, two boards are enough to start:

1. **NVIDIA Jetson Orin Nano Super Developer Kit (8GB)** — ARM64 with a CUDA-capable GPU and unified CPU/GPU memory. Covers Inference-only mode (small quantized model via Ollama), and — once TinyOS boots bare-metal on it — Real-Time control mode and the combined Inference + RT mode on the same board, since both subsystems need to coexist there to validate Non-Negotiable #6.
2. **A budget x86_64 mini-PC / NUC-class box (Intel N100/N305 class)** — no meaningful GPU, so it isolates Real-Time control mode validation on x86_64 from any inference/GPU variable, and doubles as the Tier 2 host-bridge target.

This pairing deliberately avoids a third, non-64-bit microcontroller board for "pure RT" testing — TinyOS's bare-metal, 64-bit-only design means real determinism numbers come from the Jetson and mini-PC directly, consistent with the [64-bit-only policy](#4-runs-where-the-work-happens--64-bit-only). Expand toward the full Tier 0–2 matrix below as the project grows past initial bring-up.

### Default supported set for v1

1. QEMU x86_64 + QEMU ARM64 — CI gate, every commit.
2. Jetson Orin Nano — primary real-world edge target.
3. Generic x86_64 laptop/NUC — host-bridge and shell UX target.

Raspberry Pi and real CAN/USB hardware-in-the-loop rigs are added from Phase 3 onward, once the bus stack exists.

---

## Inter-OS Communication: the Host Bridge Protocol (HBP)

TinyOS is frequently co-resident with a full-bodied OS on the same physical machine — the canonical example being a **CNC controller**, where Windows runs the operator-facing G-code sender/DRO/jog UI and TinyOS runs the real-time motion core on the same box. The Host Bridge Protocol (HBP) is the specified, versioned channel between them. It is the concrete implementation of Roadmap Phase 4.

### Design principle

The channel is layered so that latency-critical, safety-critical traffic never depends on the health of the general-purpose OS above it. Windows (or Linux) is a **caller**, subject to the same Agent Command Interface policy engine as a human shell user or an LLM agent — it gets no privileged bypass.

### Transport layer — how bytes move

- **Shared-memory ring buffers** (lock-free, single-producer/single-consumer) when TinyOS runs on a dedicated core in an AMP split on the same SoC/CPU — lowest latency, no host-OS scheduler in the path on the TinyOS side.
- **Virtio (virtio-vsock / virtio-console)** when TinyOS runs in a hypervisor partition (e.g. Jailhouse, Xen, or a lightweight Type-1 hypervisor) alongside Windows/Linux — better isolation, more portable across laptop-class hardware; the standard choice for Tier 2 targets.
- **Local loopback TCP/UDP** as a fallback and early dev-mode transport — quickest to bring up, useful for integration testing before the shared-memory/vsock path exists.

### Protocol layer — what goes over the wire

- A small, versioned, binary message protocol — fixed-size frames, no allocation on the TinyOS side, so parsing stays O(1) and doesn't threaten deadline guarantees.
- Two logically separate lanes over the same transport:
  - **Command lane** (host → TinyOS): motion commands, config changes, mode switches. Every message passes through the ACI policy engine like any other caller.
  - **Telemetry lane** (TinyOS → host): position feedback, axis status, alarms, deadline-violation events. High frequency, no acknowledgment required — the host renders latest state.

### Host-side component

- A thin **host bridge service** (Windows and Linux variants) owns the shared-memory/vsock endpoint and re-exposes it to host applications over a local named pipe or WebSocket, so an app like a CNC UI never speaks the wire protocol directly.

### Failure semantics

- If the host OS hangs, crashes, or is closed, TinyOS **must not** stall or fault. The real-time control loop continues from its last safe command or drops to a safe hold state — the RT core never blocks on the UI (Non-Negotiable #1), and the system fails safe (Non-Negotiable #5).
- The command lane carries a heartbeat. If the host stops heartbeating, TinyOS treats it as "operator gone" and transitions to a safe hold/estop state rather than continuing to execute the last known command indefinitely.

See [`docs/hbp-spec.md`](docs/hbp-spec.md) for the detailed wire format and state machine (draft).

---

## Remote Control: the Wireless Command Interface (WCI)

Not every TinyOS deployment shares a chassis with its caller. The second reference case is a **TinyOS-controlled co-bot** that exposes a WiFi node, where a remote operator, host application, or fleet controller connects over the network — and where, unlike HBP, the transport itself is untrusted and must not be allowed to become an unauthenticated command path. WCI is the specified channel for this topology.

### Design principle (extends HBP over an untrusted link)

Everything HBP guarantees for a co-resident host still applies here — the caller is subject to the ACI policy engine, no bypass exists, and the real-time core never blocks on the network. WCI adds what a wireless, multi-tenant-capable link requires on top: **no command is honored from a connection that has not completed authentication, and no network state can ever suppress the co-bot's hardware e-stop.**

### Authentication & authorization

- The co-bot's WiFi node accepts connections at the link layer (WPA2/3), but link-layer association is **not** authorization to issue commands — it only gets a client to the application-layer authentication handshake.
- Application-layer identity uses mutual TLS: each authorized controller holds a client certificate issued during an out-of-band provisioning/pairing step (physical access to the co-bot required, e.g. a provisioning button plus short-lived enrollment code) — no over-the-air enrollment without a physical trust anchor.
- An authenticated connection is mapped to a capability scope in the ACI capability registry, same as any other caller (e.g. `operator`, `supervisor`, `monitor-only`). A valid certificate proves identity; the ACI policy engine still decides what that identity may do.
- Sessions are short-lived and re-authenticated periodically; there is no standing, indefinitely-trusted connection.

### Command authority (single-writer lock)

- Multiple clients may hold read-only telemetry sessions concurrently, but only **one** client may hold command authority at a time — a leasable authority token issued by TinyOS, renewed by heartbeat, and revocable.
- A second client requesting command authority while a lease is held is denied (or queued, per policy) rather than silently allowed to issue conflicting motion commands.

### Protocol layer — wire format over TLS

- TLS-wrapped, versioned binary frames (same fixed-size, allocation-free framing discipline as HBP) — sequence-numbered to detect replay and reordering, since the transport is a real network, not a trusted local bus.
- Same two-lane structure as HBP: a **command lane** (gated by ACI + authority lease) and a **telemetry lane** (broadcast to all authenticated sessions regardless of authority).

### Failure semantics (network + authority specific)

- **Link loss or auth expiry.** The co-bot treats loss of the WiFi link, an expired session, or a lapsed authority lease identically to HBP's "operator gone" case: transition to a safe hold state, not continued execution of the last received command.
- **Hardware e-stop is out of band.** The physical e-stop on the co-bot is wired directly into the kernel's watchdog/failsafe path (Non-Negotiable #5) and is never mediated by WCI, ACI, or any network state — a compromised, jammed, or simply disconnected network can never prevent an e-stop from taking effect.
- **No silent reconnect-and-resume.** Regaining the link does not automatically resume motion; the reconnecting client must re-authenticate and re-acquire command authority, and TinyOS treats resumption as a fresh command, not a continuation.

See [`docs/wci-spec.md`](docs/wci-spec.md) for the detailed pairing flow, wire format, and authority state machine (draft).

---

## Tooling & Deployment

Remote control over a secure channel isn't just the runtime UX model — it's the primary development workflow. A device running TinyOS is reachable for reboot or hot-deploy over:

- a **peer-to-peer Ethernet cable** (link-local addressing, no switch or DHCP required) — for bring-up, recovery, and WiFi-unavailable situations, or
- **WiFi**, reusing the same authenticated pairing and session model as [WCI](#remote-control-the-wireless-command-interface-wci), scoped to a distinct `deployer` capability.

Non-core tasks and drivers can be **hot-deployed** without a reboot (atomic swap, automatic abort-and-keep-old-instance on a failed health check). Kernel-core updates go through a **reboot deploy** using A/B partition boot with automatic rollback to the last-known-good partition if the new image fails its boot-health check. A deploy can never leave a device unable to boot, and never blocks or delays an RT task while in progress.

See [`docs/deploy-protocol.md`](docs/deploy-protocol.md) for the full spec, and [`CODING_STANDARDS.md`](agent/CODING_STANDARDS.md#tooling) for the tooling standard this implements.

---

## System Architecture (target)

```text
┌─────────────────────────────────────────────────────────────────┐
│                         TinyOS Shell (UX)                        │
│   TINYCMD (DOS-style CLI)  │  Status/Monitor TUI  │  Web Console  │
└──────────────────────────┬────────────────────────────────────--┘
                            │  Command & Control API (versioned, audited)
┌──────────────────────────▼──────────────────────────────────────┐
│                     Agent Command Interface (ACI)                │
│   Capability registry │ Policy engine │ Rate limits │ Audit log   │
└───────┬───────────────────────┬──────────────────────┬──────────┘
        │                       │                       │
┌───────▼───────┐     ┌─────────▼─────────┐   ┌─────────▼─────────┐
│  Local Agent   │     │   Human Operator   │   │  Remote / Fleet   │
│ (Ollama / LLM) │     │   (shell / scripts)│   │ (host, bus, net)  │
└───────┬───────┘     └─────────┬─────────┘   └─────────┬─────────┘
        └───────────────────────┴──────────────────────-┘
                                 │
┌────────────────────────────────▼────────────────────────────────┐
│                      TinyOS Real-Time Kernel                     │
│  Preemptive scheduler │ Priority inheritance │ Deterministic IPC  │
│  Static memory pools  │ Deadline monitor      │ Watchdog/failsafe │
└──────┬────────────┬────────────┬──────────────┬──────────────┬──┘
       │             │            │              │              │
   ┌───▼───┐    ┌────▼────┐  ┌────▼────┐   ┌─────▼─────┐  ┌─────▼─────┐
   │  CAN   │    │   USB   │  │Ethernet │   │Host Bridge│  │   HAL /    │
   │  Bus   │    │ Stack   │  │/TCP-IP  │   │ (Win/Lin) │  │  Drivers   │
   └───────┘    └────────┘  └────────┘   └───────────┘  └───────────┘
```

---

## The DOS Inheritance

TinyOS borrows MS-DOS 4+'s ergonomics deliberately — not out of nostalgia, but because that era of interface got a lot right for operators who need clarity under pressure:

- A fast, single-purpose command shell (`TINYCMD`) with a familiar `C:\>`-style prompt, batch scripting (`.TCB` files), and terse, composable commands.
- A blue-screen full-view task manager (`TASKMGR.SYS`-style) for real-time task/thread status, deadlines, and bus traffic — evoking the DOS 4 `MEM`/`DOSSHELL` aesthetic.
- Configuration via plain, human-readable text (`TINYOS.CFG`, `AUTOEXEC.TCB`) — inspectable, diffable, versionable, no hidden registry.
- No modality trap: the shell never blocks the kernel, and the kernel never depends on the shell being alive.
- **DOS and POSIX/Linux command compatibility, side by side.** `TINYCMD` accepts both `DIR`/`COPY`/`DEL`-style DOS syntax and `ls`/`cp`/`rm`-style POSIX syntax against one canonical command core, so operators from either world are at home immediately. See [`docs/cli-compatibility-mvp.md`](docs/cli-compatibility-mvp.md) for the MVP verb set and architecture. (The [`MsDOS`](MsDOS) submodule — Microsoft's officially released MS-DOS source — is kept as a reference for historical command behavior, not as code TinyOS builds on.)

---

## Repository Layout (planned)

```text
/agent.md                   Single entry point for any coding agent (tool-agnostic)
/agent/                      Development guidelines (CODING_STANDARDS.md) — not code, not under os/
/docs/                      Architecture decision records, protocol specs, hardware bring-up notes
/goals/                      Verification & Validation model: Goals → Epics → Features → Stories → Tests → Reports
/session/                   Dated handover snapshots
/os/                        The OS project — everything below this line compiles
  /Cargo.toml                Workspace manifest
  /targets/                  Custom Rust target-spec JSON files for bare-metal builds (build data, not source)
  /src/                      ALL code lives here — every crate, no exceptions
    /kernel/                  Real-time scheduler, IPC, memory pools, deadline monitor
    /hal/, /hal-x86_64/, /hal-arm64/   Board-specific hardware abstraction
    /drivers/, /drivers-can/  Storage, network, HID class drivers; CAN bus stack
    /bridge-device/, /bridge-host/     HBP device-side protocol; Windows/Linux host service
    /wci/                     Wireless Command Interface: mutual TLS, authority lease
    /deploy-device/, /deploy-client/   On-device deploy/hot-swap/A-B-boot logic; host tool
    /shell/                   TINYCMD, TASKMGR, DOS-style utilities and batch runtime
    /aci/                     Agent Command Interface: capability registry, policy engine, audit log
    /inference/               Local/external LLM runtime integration (Ollama adapter), ACI tool-call mapping
    /motion/                  Motion & Interpolation, Process-Synchronized Output, Position Feedback, Safety Interlock — including the CNC trunnion-table kinematics module as a swappable submodule
    /compute/                 Unified Memory Manager, GPU admission control, -sys bindings for vendor drivers
    /config/                  System and boot configuration schemas + defaults
    /xtask/                   Host-side build/test/QEMU-launch/deploy orchestration (Rust, not shell scripts)
    /tests/                   Kernel conformance tests, timing/determinism benchmarks, HIL test rigs
```

Each crate under `os/src/` is a member of a single Cargo workspace, per [`agent/CODING_STANDARDS.md`](agent/CODING_STANDARDS.md#toolchain). See [`docs/mvp-delivery-strategy.md`](docs/mvp-delivery-strategy.md) for the full crate map (which Roadmap phase creates each one, `no_std`/`std` split, `unsafe` policy) and the phased delivery strategy from an empty repository to the CNC flagship milestone.

---

## Roadmap

- [ ] **Phase 0 — Kernel skeleton**: boot, context switch, preemptive priority scheduler, static memory pools, minimal HAL for one x86_64 target; HAL includes the initial ACPI/Device-Tree-normalizing hardware manifest per the [Universal Driver Model](docs/universal-driver-model.md).
- [ ] **Phase 1 — Determinism proof**: deadline monitor, priority inheritance, worst-case timing benchmarks and regression suite; CI gate on timing regressions, not just functional correctness.
- [ ] **Phase 1.5 — Deploy tooling**: peer-to-peer Ethernet and WiFi deploy client, A/B partition boot with automatic rollback, hot-deploy for non-core tasks. See [`docs/deploy-protocol.md`](docs/deploy-protocol.md). Ships early because remote deploy is the primary development loop, not a later convenience.
- [ ] **Phase 2 — Shell & UX**: TINYCMD core verb engine + DOS/POSIX front-ends (MVP set per [`docs/cli-compatibility-mvp.md`](docs/cli-compatibility-mvp.md)), batch scripting, TASKMGR live view, config file loader.
- [ ] **Phase 3 — Connectivity**: CAN, USB, Ethernet stacks; unify under one internal message bus with a single command dispatch path; ship mandatory class drivers (storage, network, HID) per the [Universal Driver Model](docs/universal-driver-model.md) so common hardware works before any vendor extension exists.
- [ ] **Phase 4 — Host bridge**: Windows + Linux companion services, shared-memory or socket transport, cross-OS clock sync.
- [ ] **Phase 5 — Agent Command Interface**: capability registry, policy engine, full audit trail, human-equivalent permission model for machine callers.
- [ ] **Phase 6 — LLM integration**: Ollama runtime hosted as a budgeted task; agent tool-calling mapped 1:1 onto ACI capabilities; safety evaluation harness before any agent gets write access to a live bus.
- [ ] **Phase 6b — Heterogeneous compute**: GPU admission control, Unified Memory Manager (unified and explicit-copy paths), first end-to-end local inference on Jetson Orin Nano. See [`docs/inference-architecture.md`](docs/inference-architecture.md).
- [ ] **Phase 7 — Edge bring-up**: Jetson Orin Nano (and successors) port with GPU/NPU-accelerated inference path.
- [ ] **Phase 8 — Fleet mode**: multiple TinyOS nodes coordinating over CAN/Ethernet with a shared policy and audit plane, including distributed/daisy-chained inference across nodes.

Beyond the numbered MVP path, six **destination horizons** prevent mid-flight architectural lockout without pretending the work is already scheduled: H1 application ABI/graphics/audio/games; H2 Wails/Tauri/.NET/JavaScript runtimes; H3 browser/TinySpot; H4 TLE/WST compatibility; H5 browser-hosted laboratory; H6 edge/data-centre application coordination. They are catalogued in [`goals/epics/backlog.md`](goals/epics/backlog.md) and remain undecomposed until their prerequisites are real.

---

## Non-Negotiables

These are the rules the project will not compromise on, even under schedule pressure. They resolve in strict priority order — safety first, then security, then correctness, then performance — see [`CODING_STANDARDS.md`](agent/CODING_STANDARDS.md#priority-ordering) for how that ordering governs day-to-day trade-offs.

1. **The real-time core never blocks on UI, network, or LLM inference.** A hung shell or a stalled model call must never delay a scheduled task.
2. **No agent — human or AI — gets a privileged bypass around the policy engine.** Every action, from any caller, goes through the same gate.
3. **Every state-changing command is attributable and logged.** If it happened, we know who asked, what was executed, and what the system's state was before and after.
4. **Determinism is tested, not assumed.** Timing regressions are CI failures, on par with functional test failures.
5. **The system fails safe.** Watchdogs, deadline violations, and policy denials default to the safest known state, not to "keep trying."
6. **GPU and inference work never jeopardizes CPU real-time guarantees.** Admission-controlled, never scheduler-privileged; a stalled or failed inference degrades or errors out through the ACI, it never blocks an RT task on any node.
7. **Every feature is test-driven.** Tests are written before the implementation, security- and safety-relevant code gets adversarial tests, and a PR without corresponding tests doesn't merge. See [`CODING_STANDARDS.md`](agent/CODING_STANDARDS.md#test-driven-development-mandatory).
8. **Performance is a first-class goal, pursued only after 1–3 above hold.** TinyOS aims to extract the maximum throughput and lowest latency the target hardware allows — on constrained edge devices and full-capability laptops alike — but never by weakening safety, security, or correctness guarantees.
9. **A driver fault never faults the kernel.** Drivers run outside the RT trust boundary, capability-scoped and admission-controlled like any other caller; a crashing driver is contained and restarted, never a path to a kernel panic. See the [Universal Driver Model](docs/universal-driver-model.md).
10. **Unsigned or origin-ambiguous code never executes.** Verified boot, signed content-addressed executable objects, revocation, quarantine, and anti-rollback checks happen before mapping or promotion.
11. **Process memory is private; sharing is explicit and revocable.** No ambient process handle, shared address space, or pointer can authorize cross-process access.
12. **Network, active content, tracking state, and drivers are absent unless opted in.** Absence is proven through link maps and zero registrations/grants/queues/listeners/parser surfaces.
13. **Frontier AI output is hostile input.** Fable-class campaign tests exercise long-horizon adaptive chaining, retries, races, and parallel probing; capability and resource bounds remain in force.
14. **Every Feature and Story is bound to performance, security, and containment before implementation.** The 625 performance tests, 20 security controls, five containment classes, and 20 cross-class boundary tests form the assurance spine; functional `Verified` never silently means assurance evidence exists.
15. **Remote data has no route to executable memory.** Network, HBP, WCI, ACI, shell, model, file, debug, compatibility, and deploy inputs remain non-executable until every code-admission gate passes; inspection is destroyed and admitted code starts as a fresh empty-authority domain. No production fallback, JIT exception, in-place patch, remote process-memory write, or remote trust-root enrollment bypasses this rule.
16. **One compromised process is not a system takeover.** Private active memory, unforgeable capabilities, charged CPU/resources, mediated IPC, device isolation, immutable system state, and revoke-before-reuse remain enforced even when one C2, C3, or C4 component is fully attacker-controlled. See the [`Security Charter`](SECURITY_CHARTER.md).
17. **A runtime is never the OS security boundary.** CLR, Go, Node, Bun, V8, JavaScriptCore, WebAssembly, a webview, Chromium, Linux compatibility, and game engines all run inside Protection Domains with OS-enforced authority. Their own permissions are defence in depth.
18. **Product ambition stays joined across four planes.** Every landing destination names its goals, performance domains, application workloads, and security/containment contracts together. A fast benchmark without its application and invariant, or an application promise without measurable performance and containment, cannot become a release claim.

---

## Status

Phase-0 implementation is in progress. Functional tests cover the current skeleton, but the assurance dashboard deliberately records performance/security baseline debt until raw timing, frugality, isolation, signing, adversarial, and HIL evidence exists.

New here? If you're a coding agent, start with [`agent.md`](agent.md). If you're a human, start with [`SeedMVP.md`](SeedMVP.md) for the founding intent, then the latest dated handover under [`session/`](session/) — currently [`session/hand-2026-07-27/index.html`](session/hand-2026-07-27/index.html) — for a snapshot of what's decided, what's open, and what to work on next. For traceable, testable work items, see [`goals/`](goals/).

## License

TBD.
