# Deploy & Hot-Reboot Protocol — Draft Spec

Status: **draft / tooling companion to [`CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md#tooling)**

## Purpose

TinyOS is designed to be developed and operated remotely by default — per [Non-Negotiable ordering](../README.md#non-negotiables), remote control over an established secure channel is the primary means of UX and control, not a fallback bolted on after a local console workflow. This document specifies how a developer or operator connects to a running TinyOS device to reboot it onto a new image or hot-deploy an updated component, over either a peer-to-peer Ethernet cable or WiFi.

## Connection mechanisms

### Peer-to-peer Ethernet cable

- No switch, router, or DHCP server required. The device and the connecting host negotiate link-local addressing directly over the cable (IPv6 link-local by default; IPv4 link-local as fallback for tooling that doesn't support v6).
- This mechanism is intended for bring-up, recovery, and situations where WiFi is unavailable or deliberately disabled — physical possession of the cable is itself part of the trust model here, similar in spirit to HBP's physical trust anchor for host-bridge pairing.
- Even over a direct cable, the deploy endpoint still requires authentication (see below) — physical access to the cable is necessary but not sufficient.

### WiFi

- Reuses the [WCI](../README.md#remote-control-the-wireless-command-interface-wci) pairing and session model in full: a deploy client must hold a certificate issued through the same out-of-band, physical-access provisioning flow as any other WCI controller, scoped to a `deployer` capability rather than `operator`/`supervisor`.
- No separate "developer backdoor" WiFi path exists — deploy over WiFi is WCI with a different capability scope, not a parallel unauthenticated protocol.

## Capability model

- Deploy is gated by the ACI exactly like any other action: a session authenticates, is resolved to a capability scope, and only a session holding `deployer` (or a scope that includes it) may initiate a reboot or hot-deploy.
- Deploy actions are logged with full provenance (who, what image/component, what hash, what the device's state was before and after) — the same audit trail discipline as every other ACI-gated action.
- `deployer` is a distinct scope from `operator`/`supervisor` in the capability registry; an operator authorized to send motion/control commands is not automatically authorized to deploy new code, and vice versa.

## Deploy modes

### Hot deploy (no reboot)

- Applies to non-core tasks and drivers that declare themselves hot-swappable (per their own component metadata).
- The deploy tool pushes the new component image, TinyOS validates it (signature/hash check, compatibility check against the running kernel's ABI version), then performs an atomic swap: the new instance is started, state is handed off (if the component defines a state-transfer routine), and the old instance is torn down only after the new one is confirmed healthy.
- If the new instance fails its health check, the swap is aborted and the old instance keeps running — a failed hot deploy never leaves the device without a running instance of that component.
- Hot deploy never touches RT-scheduled kernel-core code; it is scoped to non-core tasks by construction, so it cannot itself introduce an RT-path safety issue.

### Reboot deploy (kernel-core updates)

- Used for kernel, HAL, or other core-image updates that can't be hot-swapped.
- The device uses **A/B partition boot**: the new image is written to the inactive partition while the active partition keeps running normally; the device only switches boot targets on the next reboot.
- On boot into the new (B) partition, a boot-health check must pass within a bounded time window (kernel initializes, scheduler comes up, watchdog reports healthy). If the check fails or times out, the bootloader automatically rolls back to the last-known-good (A) partition on the following boot — no manual recovery step required for a bad deploy.
- A reboot deploy is never allowed to leave a device unable to boot at all: the previous known-good partition is only overwritten once the newly deployed partition has passed its health check and been explicitly promoted, per the deploy tool's confirmation step.

## Failure semantics

- **Connection drop mid-transfer.** The device discards the partial image; no partial or corrupt image is ever written to a bootable partition or hot-swapped into a running task slot.
- **Health check failure.** Automatic rollback (reboot deploy) or automatic abort-and-keep-old-instance (hot deploy) — in both cases, without requiring operator intervention to recover a functioning device.
- **Authentication failure.** No different from any other ACI-gated action: the request is rejected and logged; there is no degraded "try again without auth" path.
- **RT guarantees during deploy.** A deploy operation, hot or reboot, never blocks or delays an RT task's execution on the device — deploy traffic is handled the same way HBP/WCI command traffic is: off the RT path, admission-controlled, consistent with Non-Negotiable #1.

## Open questions

- Exact signature/hash scheme for image validation (single project key vs. per-deployment-environment key hierarchy).
- Whether hot-deploy state-transfer routines are a required part of a component's public contract or an opt-in capability.
- Multi-device fleet deploy (deploying the same image to many nodes at once) — likely builds on this protocol plus the Fleet mode work in Roadmap Phase 8, but is not specified here.

## Status

This document accompanies the Tooling section of [`CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md#tooling) and the remote-control-first philosophy stated in the README. It will be refined once the ACI capability registry and A/B boot mechanism land in the kernel skeleton.
