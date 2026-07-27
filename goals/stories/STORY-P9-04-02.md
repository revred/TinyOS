# STORY-P9-04-02 — Sealing Policy, the Fallback Tier, and Downgrade Resistance

Status: **Specified, not yet started. Gated on `LE-09`.**
Feature: [`FEAT-P9-04`](../features/FEAT-P9-04.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §8.1 — an objection neither the proposal's first draft nor [`session/hand-2026-07-28/08-memory-confidentiality-review.md`](../../session/hand-2026-07-28/08-memory-confidentiality-review.md) raised

## Description

This Story exists because two parts of the design pull against each other, and the tension has to be resolved deliberately rather than discovered in an incident.

[`session/hand-2026-07-28/08-memory-confidentiality-review.md`](../../session/hand-2026-07-28/08-memory-confidentiality-review.md) §13 is right that **an OS cannot refuse to boot because attestation is unreachable**, and points at Sharc.Blue's `from_local_only()` fallback. But the forged-kernel defence rests *entirely* on the TPM refusing to unseal for a boot that measures differently. A fallback that activates when the TPM is unreachable hands an attacker a strictly easier path than forging measurements: **make the TPM unreachable.** Cut the bus, disable it in firmware, boot on a board without one. Under [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4d rule 8 the adversary has read this document.

A fallback that silently degrades is not a resilience feature. It is the answer to the question the whole Epic was built to make unanswerable.

## Depends on

`STORY-P9-04-01`.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. **The tier is distinguishable, attributable and reported.** A boot that did not unseal is a different security state: its own spoor, its own queryable system property, and visible to anything that cares. Never a silent substitution.
2. **The fallback key space is disjoint from the sealed one.** A local-only key must not decrypt what the sealed tier sealed. If it can, the sealed tier's guarantee was never real — an attacker forces the fallback and reads everything. Degraded mode may protect *new* data; it may never retroactively open old data.
3. **The availability/confidentiality trade is a per-deployment policy input**, per [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4d rule 7. A data-centre node may reasonably refuse to run; a UAV in flight may not. That choice is configuration, not a default buried in a `Result` arm — and the refusing configuration must actually be reachable and tested, or it is not a choice.
4. **The downgrade is exercised adversarially.** Make the TPM unavailable and demonstrate what happens: which tier is entered, what is readable, what is reported. A fallback path nobody has attacked is a fallback path nobody has evidence for.
5. **Anti-rollback holds across the tiers** (`RCG-06`): entering degraded mode once must not permanently weaken a machine that later boots correctly.

## Explicitly out of scope

- Attestation to a remote verifier — that composes with this but is `FEAT-P9-07`.

## Tests

Not yet written — deferred until this Story starts. Hardware-tier throughout: the interesting cases are all "what happens when the hardware is absent, disabled, or lying".

## Goals verified

G-SEC-15, G-SEC-14 (every tier transition is attributable).
