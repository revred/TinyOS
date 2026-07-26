# TinyOS

**A real-time operating system with the soul of MS-DOS and the reflexes of an RTOS — built to sit between silicon and intelligence.**

TinyOS is a from-scratch real-time operating system designed to run on anything from a modern laptop to a Jetson Nano-class edge device, and to speak fluently to the Windows/Linux host it lives beside or the machines it's wired to over CAN bus, USB, or Ethernet. It looks and feels like MS-DOS 4+ — a fast, legible, keyboard-driven command environment — but underneath that familiar shell is a hard-partitioned multitasking core built for deterministic, real-time control. And it's designed from day one to host a local LLM (via Ollama or an equivalent runtime) as a first-class citizen: not a chat window bolted on top, but a supervised operator that can observe, propose, and — within strict, auditable limits — act.

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

### 4. Runs where the work happens
- Target hardware spans laptop-class x86_64 (as a bare-metal boot option or hosted partition) down to Jetson Nano-class ARM64 edge devices with GPU/NPU acceleration.
- Hardware abstraction layer (HAL) keeps board-specific code in one place so the kernel, scheduler, and shell stay portable.

### 5. LLM as a supervised operator, not a root user
- Local inference (Ollama or compatible runtime) runs in its own isolated task/partition — resource-budgeted like any other real-time citizen, never able to preempt hard-deadline control loops.
- The LLM interacts with TinyOS exclusively through the **Agent Command Interface (ACI)**: a declarative, capability-scoped API where every possible action is pre-registered, typed, rate-limited, and logged.
- **Strict rule: the LLM can request, TinyOS decides.** Every agent-issued command passes through the same policy engine as a human operator's command — no privileged bypass path exists for AI-originated actions.
- Full command provenance: every state change is tagged with *who* asked (human shell, script, remote host, or agent), *what* was requested, *what* was actually executed, and *why* the policy engine allowed it.

---

## System Architecture (target)

```
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

```
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
- [ ] **Phase 7 — Edge bring-up**: Jetson Nano (and successors) port with GPU/NPU-accelerated inference path.
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
