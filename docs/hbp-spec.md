# Host Bridge Protocol (HBP) — Draft Spec

Status: **draft / Phase 4 (not yet implemented)**

## Purpose

HBP is the specified channel between a full-bodied host OS (Windows or Linux) and a co-resident TinyOS instance on the same physical machine. The reference use case is a CNC controller: Windows hosts the operator UI (G-code sender, jog controls, DRO), TinyOS hosts the real-time motion core.

HBP exists so the host OS can act as a caller into TinyOS without ever gaining a privileged path around the Agent Command Interface (ACI) policy engine, and without the real-time core ever depending on the host OS being alive or responsive.

## Deployment topologies

| Topology | Transport | Notes |
|---|---|---|
| AMP split, same SoC, dedicated core for TinyOS | Shared-memory ring buffers | Lowest latency; no host scheduler in the path |
| Hypervisor partition (Jailhouse / Xen / Type-1) | virtio-vsock or virtio-console | Better isolation; standard for Tier 2 laptop/NUC targets |
| Dev / early integration | Loopback TCP or UDP | Fastest to bring up; not for production timing guarantees |

## Lanes

HBP defines two logically independent lanes multiplexed over one transport connection.

### Command lane (host → TinyOS)

- Carries: motion commands, configuration changes, mode switches, agent-equivalent requests from host-side software.
- Every command is evaluated by the ACI policy engine exactly as if it came from the local shell or an LLM agent. There is no bypass path for host-originated commands.
- Each command frame requires an acknowledgment frame (accepted / denied / deferred) carrying the same provenance fields the ACI audit log uses elsewhere: caller identity, requested action, decision, reason.

### Telemetry lane (TinyOS → host)

- Carries: position/axis feedback, system status, alarms, deadline-violation notices.
- Fire-and-forget, no acknowledgment required. The host renders the most recent frame; stale frames are simply superseded.
- Published at a fixed, configurable rate independent of command lane traffic.

## Wire format (draft)

- Fixed-size binary frames. No dynamic allocation on the TinyOS side during encode/decode — parsing must stay O(1) and allocation-free so it cannot threaten scheduler deadline guarantees.
- Frame header: protocol version, lane id, sequence number, payload length, checksum.
- Exact field layout, versioning/negotiation handshake, and payload schemas per command type are TBD — to be finalized alongside the Phase 4 host bridge service implementation.

## Failure semantics

- **Host silence.** The command lane carries a heartbeat from the host. If heartbeats stop, TinyOS treats the host as "operator gone" and transitions to a safe hold/estop state rather than continuing to execute the last known command indefinitely.
- **Host crash/close.** TinyOS's real-time loop is never blocked on the host bridge connection. Loss of the host connection is handled the same way as heartbeat silence — safe hold, not fault.
- **TinyOS-side fault.** Out of scope for HBP itself; governed by the kernel's own watchdog/failsafe behavior (see main README, Non-Negotiable #5). HBP only needs to ensure the host is notified via the telemetry lane's absence.

## Host-side component

A thin host bridge service (Windows and Linux variants, per Roadmap Phase 4) owns the transport endpoint (shared memory, vsock, or loopback socket) and re-exposes it to host applications over a local named pipe or WebSocket. Host applications — e.g. a CNC UI — talk to the bridge service, never to the wire protocol directly.

## Open questions

- Frame layout and versioning/negotiation handshake.
- Authentication/identity model for the host-side caller within the ACI capability registry (is "Windows host" one fixed capability principal, or does the bridge service pass through the identity of the host application?).
- Exact heartbeat interval and hold-state re-entry conditions (does the operator have to explicitly re-arm after a host-silence hold, or does it auto-resume on reconnect?).
- Loopback TCP/UDP dev-mode transport: security posture if ever exposed beyond localhost (should be explicitly disallowed).

## Status

This document accompanies the "Inter-OS Communication" section of the top-level [README](../README.md) and will be filled in during Phase 4 implementation.
