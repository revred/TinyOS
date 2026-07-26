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

## Design Pillars

### 1. A real multitasking RTOS core

- Preemptive, priority-based scheduler with bounded interrupt latency and no unbounded priority inversion (priority inheritance/ceiling protocols from day one).
- Deterministic memory model — static or pool-based allocation in real-time paths; no surprise heap fragmentation in the control loop.
- Time is a first-class resource: every task declares its period, deadline, and worst-case execution budget. The kernel enforces them and screams (loudly, safely) when they're violated.

### 2. UX/UI strictly separated from control

- The DOS-like shell (`TINYCMD`) is a **presentation and orchestration layer only**. It never has direct write access to real-time task state, drivers, or bus I/O.
- All shell and UI actions go through a narrow, versioned **Command & Control API** — the same API used by scripts, remote hosts, and the LLM agent. One gate, many callers, one audit trail.
- This means the UI can crash, hang, or be swapped out (text console today, a web dashboard tomorrow) without ever jeopardizing a real-time task that's mid-cycle.

### 3. Host and bus connectivity as a native concept

- **Host bridge**: a lightweight driver/service pair (Windows + Linux) that lets TinyOS run as a companion OS on the same machine (dual-boot, hypervisor partition, or a dedicated core on an AMP/SMP split) and exchange typed messages with host-side processes.
- **CAN bus**: native CAN 2.0B/CAN-FD stack for talking to vehicles, industrial controllers, and other embedded nodes.
- **USB**: device and host-mode USB stacks for peripherals, flashable storage, and tethered control links.
- **Ethernet**: lwIP-class TCP/IP stack for edge-to-cloud and edge-to-fleet communication.
- All transports terminate at the same internal message bus — a CAN frame, a USB packet, and a TCP message can all trigger the same command handler, subject to the same permission checks.

### 4. Runs where the work happens — 64-bit only

- No 32-bit targets, ever. TinyOS commits to **x86_64 and ARM64** exclusively, which simplifies the kernel's memory model, pointer/ABI assumptions, and driver interfaces from day one.
- Target hardware spans laptop-class x86_64 (as a bare-metal boot option or hosted partition) down to ARM64 edge devices such as the Jetson Orin Nano, with GPU/NPU acceleration for local inference.
- Hardware abstraction layer (HAL) keeps board-specific code in one place so the kernel, scheduler, and shell stay portable across both architectures.

### 5. LLM as a supervised operator, not a root user

- Local inference (Ollama or compatible runtime) runs in its own isolated task/partition — resource-budgeted like any other real-time citizen, never able to preempt hard-deadline control loops.
- The LLM interacts with TinyOS exclusively through the **Agent Command Interface (ACI)**: a declarative, capability-scoped API where every possible action is pre-registered, typed, rate-limited, and logged.
- **Strict rule: the LLM can request, TinyOS decides.** Every agent-issued command passes through the same policy engine as a human operator's command — no privileged bypass path exists for AI-originated actions.
- Full command provenance: every state change is tagged with *who* asked (human shell, script, remote host, or agent), *what* was requested, *what* was actually executed, and *why* the policy engine allowed it.

---

## Target Hardware & Test Matrix

TinyOS is **64-bit only** — no 32-bit boot path is planned or supported, on either architecture.

### Tier 0 — Emulated (CI gate, every commit)

- **QEMU x86_64** (`q35` machine type) and **QEMU ARM64** (`virt` machine type) — the primary dev loop; kernel, scheduler, and ACI changes are validated here before any real hardware is touched.
- **Renode** — bus/peripheral simulation for CAN, USB, and Ethernet driver work ahead of physical hardware availability.

### Tier 1 — Edge device (primary mission target)

- **Jetson Orin Nano** (ARM64) — the standard edge target for new hardware bring-up; GPU-accelerated local inference (Ollama) validation happens here.
- A second, non-NVIDIA ARM64 board (e.g. Raspberry Pi 4/5) — portability check so the HAL doesn't quietly grow Jetson-only assumptions.

### Tier 2 — Laptop / x86_64 (host-bridge + full UX validation)

- A mid-spec x86_64 laptop or NUC-class mini-PC, dual-boot or hypervisor-partitioned — validates the Windows/Linux host bridge and the DOS-style shell UX, not just the kernel.

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

---

## Repository Layout (planned)

```text
/kernel/          Real-time scheduler, IPC, memory pools, deadline monitor
/hal/              Board-specific hardware abstraction (x86_64, Jetson/ARM64, ...)
/drivers/          CAN, USB, Ethernet, storage, display, misc peripherals
/bridge/           Host-side companion services (Windows, Linux) + wire protocol
/shell/            TINYCMD, TASKMGR, DOS-style utilities and batch runtime
/aci/              Agent Command Interface: capability registry, policy engine, audit log
/agent/            Local LLM runtime integration (Ollama adapter), prompt/tooling contracts
/config/           System and boot configuration schemas + defaults
/docs/             Architecture decision records, protocol specs, hardware bring-up notes
/tests/            Kernel conformance tests, timing/determinism benchmarks, HIL test rigs
```

---

## Roadmap

- [ ] **Phase 0 — Kernel skeleton**: boot, context switch, preemptive priority scheduler, static memory pools, minimal HAL for one x86_64 target.
- [ ] **Phase 1 — Determinism proof**: deadline monitor, priority inheritance, worst-case timing benchmarks and regression suite; CI gate on timing regressions, not just functional correctness.
- [ ] **Phase 2 — Shell & UX**: TINYCMD, batch scripting, TASKMGR live view, config file loader.
- [ ] **Phase 3 — Connectivity**: CAN, USB, Ethernet stacks; unify under one internal message bus with a single command dispatch path.
- [ ] **Phase 4 — Host bridge**: Windows + Linux companion services, shared-memory or socket transport, cross-OS clock sync.
- [ ] **Phase 5 — Agent Command Interface**: capability registry, policy engine, full audit trail, human-equivalent permission model for machine callers.
- [ ] **Phase 6 — LLM integration**: Ollama runtime hosted as a budgeted task; agent tool-calling mapped 1:1 onto ACI capabilities; safety evaluation harness before any agent gets write access to a live bus.
- [ ] **Phase 7 — Edge bring-up**: Jetson Orin Nano (and successors) port with GPU/NPU-accelerated inference path.
- [ ] **Phase 8 — Fleet mode**: multiple TinyOS nodes coordinating over CAN/Ethernet with a shared policy and audit plane.

---

## Non-Negotiables

These are the rules the project will not compromise on, even under schedule pressure:

1. **The real-time core never blocks on UI, network, or LLM inference.** A hung shell or a stalled model call must never delay a scheduled task.
2. **No agent — human or AI — gets a privileged bypass around the policy engine.** Every action, from any caller, goes through the same gate.
3. **Every state-changing command is attributable and logged.** If it happened, we know who asked, what was executed, and what the system's state was before and after.
4. **Determinism is tested, not assumed.** Timing regressions are CI failures, on par with functional test failures.
5. **The system fails safe.** Watchdogs, deadline violations, and policy denials default to the safest known state, not to "keep trying."

---

## Status

Early design phase. Architecture and roadmap above are the north star; implementation is starting from the kernel skeleton outward. Contributions, critique, and hardware to test on are all welcome once Phase 0 lands.

## License

TBD.
