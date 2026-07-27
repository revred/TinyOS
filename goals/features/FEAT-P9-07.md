# FEAT-P9-07 — Hardware Memory Encryption, Attestation, and the Fallback Tier

Status: **Specified — no Story started. Gated on `LE-09`.**
Epic: [`EPIC-P9`](../epics/EPIC-P9.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §6 (R4), §4c, §8

## Description

The highest-value recommendation in the proposal and the last one buildable — which is not a contradiction. AMD SME/SEV-SNP, Intel TME/MKTME/TDX, ARM CCA: the key never leaves the memory controller, so a live dump is ciphertext at approximately zero per-access latency. **It is the only item that answers A3, A4 and A5 at once, and the only thing that protects the live working set or a key at the moment of use** — neither of which any software design can reach.

It is late in the chain because [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4c is decisive: **hardware memory encryption alone is not sufficient.** SME/TDX keys are ephemeral — generated at power-on, lost at power-off. A hibernation image is written out as plaintext from the OS's point of view, and on resume the CPU holds a new key that decrypts nothing that came before. "The key lives in the CPU" is a true statement about the live system and an incomplete one about its lifetime, and any design that stops there fails at the first suspend-to-disk. `FEAT-P9-05`'s sealed root is what closes that, so this Feature depends on it.

Mostly enablement, configuration and attestation rather than kernel code.

## Crate(s) involved

`os/src/hal-x86_64/` (SME/TME enablement and capability detection), the attestation path, deployment configuration

## Depends on

`FEAT-P9-04` and `FEAT-P9-05`. **Hardware** exposing a memory-encryption engine.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P9-07-01`](../stories/STORY-P9-07-01.md) | Enablement, capability detection, attestation, and documented accepted risk where the engine is absent | Specified |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0** · subject **C0/C1/C2** · boundary tests **BND-01, -17, -20**.

Enablement changes what an *external* observer can read and changes nothing about what any subject may do — no capability, no class, no priority (`PD-01` private active address spaces, `PD-10` device-bound DMA/IRQ/MMIO, `PD-14` no ambient authority, `RCG-12` explicit attributable activation).

## Exit criteria

Its Story **Verified on hardware exposing the engine**: encryption is enabled and confirmed active by reading it back rather than by having written the enable bit, a dump taken outside the OS is ciphertext, and attestation reports the state honestly.

**And where the engine is absent, that is documented accepted risk** — [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §11.3's own falsification: if the deployment target does not expose SME/TME, A5 has **no answer at all** and must be recorded as such rather than quietly assumed covered. A Report claiming this Feature on a machine without the hardware would be the exact failure this project's assurance rules exist to prevent.
