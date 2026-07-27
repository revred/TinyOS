# Handover 08 — `EPIC-P1_5`: Deploy-Loop Plausibility and the Transport Decision

Follows: [`07-story-p1-02-02-double-fault.md`](07-story-p1-02-02-double-fault.md). No code, no Report — a decision-record session, same shape as [Handover 03](03-le-09-arm64-pi5-slice-proposal.md).

## Why this exists

The user has three spare machines (an old Mac mini, a Windows laptop, a Raspberry Pi 5) and asked how to use them to test the OS, how to get a fast iterate-and-reload loop going as the OS evolves, and how to pipe `spoor` telemetry back as a feedback loop. That widened into a live design conversation — a double-buffered "swap the kernel like a rendering model" idea, corrected against `docs/deploy-protocol.md`'s existing spec — and the user then asked explicitly that the resulting decisions **penetrate the planned work and context for future sessions**, not stay stranded in one chat transcript. This handover is that persistence step.

**Nothing here is new design.** Every mechanism decision below already existed in [`docs/deploy-protocol.md`](../../docs/deploy-protocol.md); this session's contribution was reconciling a live brainstorm against that document and recording which of its choices are now confirmed as the near-term plan for `EPIC-P1_5`.

## Decisions recorded

**Transport: peer-to-peer Ethernet cable first, WiFi later via WCI, USB is not a deploy transport.** `docs/deploy-protocol.md` already specifies link-local Ethernet (no switch/DHCP) as the bring-up/recovery/dev-loop mechanism, and WiFi as the same protocol under full WCI pairing rather than a separate path. Both the Mac mini and the Pi 5 have onboard Ethernet, so the cable path works on either without new hardware or a new driver design; WiFi needs the WCI certificate-provisioning flow this project hasn't built yet, so it stays a later phase. USB is not named as a deploy transport anywhere in the spec — it appears only as a general peripheral/inter-device bus for `EPIC-P3`. Concretely: whenever `EPIC-P1_5` is decomposed, its first Story-shaped slice is the peer-to-peer Ethernet link-local handshake, not a USB or WiFi path.

**No jump-to-payload primitive for kernel-core, confirmed rather than reinvented.** The brainstorm's starting idea — keep two kernel images resident in memory, quiesce, jump directly, "hot reload" via double buffering — is a real technique (`kexec`'s shape) but `docs/deploy-protocol.md` already forecloses it for kernel-core by design: *"The deploy service has no executable-mapping, process-write, driver-load, or jump-to-payload primitive."* Kernel-core updates go through **reboot deploy**: image staged to an inactive A/B partition, control only transfers on a real reboot, C0 verifies fresh every time, and a bounded boot-health check auto-rolls-back to last-known-good on failure. **Hot deploy** (the double-buffer-shaped swap) exists but is explicitly scoped to non-core tasks/drivers only, and never touches RT-scheduled kernel-core code — this is a security-model constraint (the C0–C4 containment model, `code-admission-gates.tsv`), not an oversight to design around later.

**The 8MB image ceiling (`G-DX-8`) is what makes A/B staging cheap.** Holding a full second kernel image resident (inactive-partition write) costs at most ~16MB against any of these machines' real RAM — the ceiling is a small-auditable-TCB security goal, not a constraint the deploy design fights against.

**The iterate loop is governed by the assurance spine already built, not a new gate.** "Governed by the expectations and goals of the OS," concretely: `xtask check-image-size` (`G-DX-8`) and the timing-envelope gate (`gate.rs`/`assurance.rs`) are the pre-flight checks that should run before anything is staged over the wire; `code-admission-gates.tsv`'s five stages are what admits it; the boot-health-check/auto-rollback is what makes a bad iteration self-heal. Spoor/timing telemetry riding back over the same Ethernet link (once it exists) is the closing half of the feedback loop the user asked about — same pattern the UART-borne `TINYOS-RESULT/1` line already established for Tier 0/`LE-09`, extended to a network transport instead of serial.

## What is honestly not true yet

None of this is implemented. There is no ACI capability registry, no A/B boot mechanism, and no Ethernet driver in the kernel. `EPIC-P1_5` is still backlog, "not yet decomposed," per `goals/traceability-matrix.md` and `goals/epics/backlog.md`. The `deployer` capability scope `docs/deploy-protocol.md` requires for even the direct-cable path depends on the ACI capability registry work that's currently placed under `EPIC-P5` — worth resolving explicitly when `EPIC-P1_5` is actually decomposed, since a first Story that skips authentication to get a fast dev loop working would violate the spec's own stated rule that physical possession of the cable is "necessary but not sufficient." This is the same shape of tension `LE-09` already carries (hardware evidence wanted now, prerequisites not finished yet) and should be sequenced the same deliberate way, not shortcut.

## Loose-ends register (canonical as of this handover)

Carried forward from [Handover 07](07-story-p1-02-02-double-fault.md#loose-ends-register-canonical-as-of-this-handover); no items closed, one narrowed, none new (this session decided a direction, it did not build or unblock anything).

| ID | Loose end | Origin | Owner / fix path | Status |
|---|---|---|---|---|
| LE-01 | Priority-inheritance behavioral half never verified | `STORY-P0-02-03` | `STORY-P1-04-01` criterion 2 | Open — owned |
| LE-02 | WCET enforcement has no timer and no watchdog behind it | `STORY-P0-02-04` | `STORY-P1-04-02` | Open — owned |
| LE-03 | No real fault handling for the remaining vectors | Handover 32 | `FEAT-P1-02` | Unchanged — `#XF` (19), `#MC` (18), and every other vector still reach the shared fail-closed default |
| LE-04 | No TSS/IST; a fault during fault handling triple-faults | Handover 32 | `STORY-P1-02-02` | Closed (Handover 07) |
| LE-05 | `exec::AddressSpace` built but never installed; system runs all-RWX identity-mapped | `STORY-P0-05-02` | `FEAT-P1-03` | Open — owned |
| LE-06 | `pool-bench` was a divergent sibling harness | Handover 35 | `STORY-P1-01-01` | Closed |
| LE-07 | CI has never been observed running any of this work | Standing since Handover 07 | Phase-independent | Closed |
| LE-08 | I/O APIC device-IRQ routing deferred (local APIC only) | `STORY-P0-04-02`/`-03` | First Story routing a device IRQ | Open — deferred with trigger |
| LE-09 | Pi 5 (ARM64) is the short-term hardware, but no ARM64 bring-up slice or deploy path exists | Handover 37 directive 1 | Option B with the carve-out ([Handover 03](03-le-09-arm64-pi5-slice-proposal.md)) | **Narrowed 2026-07-27 (this handover)** — the *deploy path* half now has a recorded transport decision (peer-to-peer Ethernet, per `docs/deploy-protocol.md`, this handover) rather than being fully open-ended; the *bring-up slice* half (pieces 1, 2, 5) is unchanged and no longer blocked on `FEAT-P1-02`'s functional half, which is done |
| LE-10 | ECAM/MCFG config access and PCI bridge traversal deferred | `STORY-P0-04-03` | First Story needing extended config space | Open — deferred with trigger |
| LE-11 | `Context::new` seeds task `rflags` with `IF` set | `STORY-P1-01-01` | `FEAT-P1-02` | Open — mitigated, not fixed |
| LE-12 | CI's clippy never lints target-only fixture code | `STORY-P1-01-01` | Per-fixture target clippy in the CI lint job | Open — unowned, backlog behind it is zero |
| LE-13 | Measurement ran dev-profile binaries | `STORY-P1-01-01` | `STORY-P1-01-02` | Closed |
| LE-14 | `context::switch` saves no SSE/x87 state | `STORY-P1-01-01` / ADR 0003 | `FEAT-P1-04` | Open — owned |
| LE-15 | The AArch64 generic timer is a 54 MHz system counter | `STORY-P1-01-03` | Decide when a board exists | Open — owned |
| LE-16 | The Tier 0 timing gate can only detect regressions of ~1.6x or worse | `STORY-P1-01-02` | Only a hardware tier fixes it (`LE-09`) | Open — owned |
| LE-17 | The fault path has no timing baseline | `STORY-P1-02-01` | Add a fault-latency phase to `fixture_measure` | Open — owned, next |
| LE-18 | The timing gate is host-condition-sensitive | `STORY-P1-02-02` | Needs a decision about what baselines are *of* | Open — unowned, needs a Story |

## Next session — start here

1. **`LE-17`** and **`FEAT-P1-03`** are still the actual next implementation work, unchanged from Handover 07 — this session added no new code-shaped priority ahead of them.
2. When `EPIC-P1_5` is picked up for decomposition: start from this handover's transport decision (peer-to-peer Ethernet, `docs/deploy-protocol.md`'s existing spec) rather than re-litigating USB/WiFi/Ethernet, but resolve the ACI-capability-registry sequencing question named above before writing the first Story's `story-contracts.tsv` row — a deploy endpoint with no `deployer` capability check would contradict the spec it's implementing.
3. If the user acquires the USB-TTL serial cable discussed for `LE-09`, that unblocks `LE-09` pieces 1/2/5 independent of anything in this handover — the two threads (ARM64 bring-up, deploy tooling) are related but not sequenced on each other.
