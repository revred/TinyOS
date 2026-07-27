# STORY-P9-07-01 — Enablement, Attestation, and Documented Accepted Risk

Status: **Specified, not yet started. Gated on `LE-09`.**
Feature: [`FEAT-P9-07`](../features/FEAT-P9-07.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §6 (R4)

## Description

Enable hardware memory encryption, confirm it is genuinely active, attest to it, and be honest where it does not exist.

## Depends on

`STORY-P9-04-02`, `STORY-P9-05-01`. Hardware exposing SME/TME or equivalent.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. **Capability detection precedes enablement**, and an absent engine is a reported, first-class state — not an error, and emphatically not a silent no-op. A system that believes it is encrypted and is not is worse than one that knows it is not.
2. **Activation is confirmed by reading it back**, not by having written an enable bit. This project has been here before: `STORY-P1-03-02` found that `map_4k` had been writing PTE bit 63 since `STORY-P0-05-02` with no hardware honouring it, because nothing had set `EFER.NXE`. A write is not evidence of an effect.
3. **A dump taken outside the OS is ciphertext**, demonstrated — which is the claim, and it cannot be checked from inside the guest.
4. **Attestation reports the true state**, including degraded and unavailable, and composes with `STORY-P9-04-02`'s tier reporting rather than being a second, independent notion of "are we secure".
5. **Where the engine is absent, the Report documents accepted risk** with A5 explicitly unanswered, per [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §11.3.

## Explicitly out of scope

- IOMMU / DMA isolation (`SEC-18`). A4 benefits from this Feature, but the IOMMU is its own Feature on its own merits.

## Tests

Not yet written — deferred until this Story starts. Almost entirely hardware-tier; host tests cover only capability-decode logic.

## Goals verified

G-SEC-2, G-SEC-13.
