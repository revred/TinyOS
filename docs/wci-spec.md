# Wireless Command Interface (WCI) — Draft Spec

Status: **draft / not yet scheduled on Roadmap** (candidate for a Phase 3.5 or Phase 5 extension, alongside CAN/USB/Ethernet connectivity and the ACI); governed by [`SECURITY_CHARTER.md`](../SECURITY_CHARTER.md)

## Purpose

WCI is the specified channel between a remote caller and a TinyOS instance reachable over a WiFi node it exposes — the reference deployment is a **co-bot**: a collaborative robot arm or mobile platform whose only control surface is the network. Unlike the Host Bridge Protocol (HBP), which connects a co-resident host OS over a trusted local transport, WCI's transport is a wireless network that must be treated as hostile by default.

WCI exists so a remote operator, host application, or fleet controller can issue commands to a co-bot only after proving identity and being granted a capability scope — and so that no state on the network side, benign or malicious, can ever suppress the co-bot's physical safety systems.

Mutual authentication establishes a remote principal; it does not make remote bytes code and does not grant process-memory, executable-mapping, deploy-activation, driver-load, trust-root, or recovery-policy authority.

## Threat model

- The WiFi link may be eavesdropped, jammed, or subject to unauthorized association attempts.
- A device may be physically stolen or its firmware inspected; long-lived shared secrets are assumed to leak eventually.
- Multiple legitimate clients may be present on the same network (e.g. a supervisor's laptop and an operator's tablet); the protocol must arbitrate between them, not just between "authorized" and "unauthorized."

## Provisioning (pairing) flow

1. A controller is provisioned only via physical access to the co-bot: a provisioning button or equivalent physical trust anchor puts the co-bot into a short enrollment window.
2. During enrollment, the co-bot issues a client certificate (or accepts a certificate signing request) tied to the connecting device, scoped to a capability role chosen at provisioning time (`operator`, `supervisor`, `monitor-only`).
3. Enrollment windows are time-boxed and single-use; no standing "add device" network endpoint exists outside this window.
4. Certificates are revocable from the co-bot's local admin interface (via TinyOS's own shell, not remotely) and expire on a defined rotation schedule.

## Session establishment

1. Client associates to the co-bot's WiFi node (WPA2/3). Link-layer association alone grants no command capability.
2. Client opens a TLS connection to the WCI endpoint and authenticates via its provisioned client certificate (mutual TLS).
3. TinyOS resolves the certificate to a capability scope via the ACI capability registry and opens a session bound to that scope.
4. Sessions are time-limited and must re-authenticate on expiry; there is no indefinite-trust connection state.

## Command authority (single-writer lease)

- Any authenticated session may subscribe to the telemetry lane.
- Only one session may hold the **command authority lease** at a time. The lease is:
  - requested explicitly by a session with sufficient capability scope,
  - renewed by heartbeat while held,
  - automatically released on session timeout, disconnect, or explicit yield,
  - never assumed — a session with an expired or unheld lease has its command frames rejected by the ACI policy engine before they reach the motion core.
- A second session requesting authority while a lease is held is denied or queued per configured policy (first-come, priority-preempt, or supervisor-override — policy is deployment-specific, not protocol-specific).

## Remote-code exclusion

- WCI accepts only enumerated, fixed-schema commands and bounded data objects. It has no generic shell, eval, script, native-code, process-write, debugger, raw-syscall, or arbitrary-tool endpoint.
- Downloads, model output, configuration blobs, and deploy payloads remain immutable non-executable C4 objects with origin and session provenance.
- A `deployer`-scoped session may stage an object only through the separate deploy protocol. It cannot activate it; every `RCG-01..RCG-14` gate and fresh-domain rule still applies.
- Certificate enrollment does not confer signer or boot-root authority. Trust-root enrollment, signer-authority expansion, rollback reset, and recovery-policy change require local physical/recovery ceremony and have no standing WCI operation.
- Compromise of the WCI service remains a C2 compromise: its Protection Domain owns only bounded network/session endpoints and cannot access C0/C1 memory, another process, unrelated storage, devices, or executable mapping.

## Wire format (draft)

- All application traffic is TLS-wrapped.
- Inside TLS: fixed-size, versioned binary frames (same framing discipline as HBP — no dynamic allocation on the TinyOS side).
- Frame header: protocol version, lane id (command / telemetry / control), session id, sequence number, payload length, MAC (in addition to TLS record integrity, for defense in depth at the application layer).
- Sequence numbers are strictly monotonic per session; out-of-order or replayed sequence numbers are dropped and logged.
- Exact payload schemas per command type are TBD — to be finalized alongside the co-bot's motion command set.

## Failure semantics

- **Link loss.** Co-bot transitions to a safe hold state. It does not continue executing the last received command.
- **Auth/session expiry.** Command frames from an unauthenticated or lapsed session are rejected at the ACI boundary; the co-bot does not distinguish this from link loss for safety purposes.
- **Authority lease lapse.** If the lease holder's heartbeat stops, the lease is released and the co-bot enters safe hold, independent of whether the underlying TLS session is still technically open.
- **Reconnect.** Regaining the link or re-authenticating never auto-resumes motion. The client must explicitly re-acquire command authority, and TinyOS treats the first post-reconnect command as fresh, not a continuation of pre-disconnect state.
- **Hardware e-stop.** Physically wired into the kernel's watchdog/failsafe path, entirely outside WCI, ACI, and any network or session state. No combination of valid sessions, held leases, or in-flight commands can prevent or delay an e-stop.

## Open questions

- Certificate rotation/revocation distribution if the co-bot is offline at rotation time (grace period vs. hard cutover).
- Authority preemption policy defaults (should a `supervisor`-scope session always be able to preempt an `operator`-scope lease, or must it request and wait?).
- Multi-co-bot fleet case: does a fleet controller hold independent leases per co-bot, or is there a fleet-level authority concept layered above WCI? (Relates to Roadmap Phase 8 — Fleet mode.)
- Telemetry lane confidentiality requirements if multiple tenants with different clearance levels observe the same co-bot.

## Status

This document accompanies the "Remote Control: the Wireless Command Interface (WCI)" section of the top-level [README](../README.md). WCI is not yet scheduled to a specific roadmap phase; it will likely land alongside or after the CAN/USB/Ethernet connectivity work (Phase 3) and the Agent Command Interface (Phase 5), since it depends on both.
