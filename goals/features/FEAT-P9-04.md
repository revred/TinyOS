# FEAT-P9-04 — Hardware Root of Trust: TPM 2.0, Measured Boot, Sealing Policy

Status: **Specified — no Story started. Gated on `LE-09`: this Feature cannot be verified without hardware.**
Epic: [`EPIC-P9`](../epics/EPIC-P9.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4b, §4c, §4d, §8

## Description

The Feature the entire Epic rests on, and the only answer to A6 — an attacker who builds their own TinyOS with the encryption stubbed out and boots it on the target.

`root_secret` is sealed by the TPM against PCRs measuring the boot chain. One byte different in the kernel image produces a different measurement, and the TPM refuses to release the secret: the forged kernel boots perfectly and finds nothing but ciphertext it can never key. That property is supplied by hardware and is entirely unaffected by publication — which is what lets `EPIC-P9` satisfy Kerckhoffs's principle instead of quietly relying on obscurity.

It is also the Feature with the most external dependency in the whole project. **TinyOS is PVH direct-kernel-loaded; under QEMU nothing measures anything.** On real hardware the measuring role belongs to firmware/shim/bootloader, and this Feature has to specify what it requires of that chain rather than assume it. This is `SEC-01` — hardware-rooted verified boot — which has been declared in the Security Charter from the beginning with nothing behind it.

## Crate(s) involved

`os/src/hal-x86_64/` (TPM 2.0 transport — CRB/FIFO — and PCR access), a new sealing-policy module, `os/src/os/` (the boot-path unseal)

## Depends on

**Hardware.** A board with a TPM 2.0 and a measuring boot chain. Nothing in this Feature can be honestly verified at Tier 0: a software TPM emulator verifies the emulator, in exactly the way `STORY-P1-03-03`'s cross-space `D04` figure measured TCG's TLB model rather than a `mov cr3`.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P9-04-01`](../stories/STORY-P9-04-01.md) | TPM 2.0 transport, PCR measurement, and seal/unseal of `root_secret` | Specified |
| [`STORY-P9-04-02`](../stories/STORY-P9-04-02.md) | Sealing policy, the fallback tier, and downgrade resistance | Specified |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0** · subject **C0/C1** · boundary tests **BND-01, -02, -17, -20**.

The unseal is the highest-authority operation in the system and grants exactly one thing: bytes. It confers no capability, no class change and no scheduling priority. A failed or refused unseal is a reported system state with its own spoor, never a fallback taken silently (`PD-03` empty authority first, `PD-11` non-increasing provenance, `PD-14` no ambient authority, `RCG-05` signature and trust-path verification, `RCG-06` revocation freshness and anti-rollback).

## Exit criteria

Both Stories **Verified on real hardware** — this Feature is the reason `EPIC-P9` states it cannot exit at Tier 0. A measured boot releases the secret; a deliberately modified kernel image does not, demonstrated rather than argued; and the fallback tier's downgrade risk is resolved per `STORY-P9-04-02`.
