# STORY-P9-06-01 — Sealed Image Bytes at Rest, Replay and Relocation Rejected

Status: **Specified, not yet started. Gated on `LE-09`.**
Feature: [`FEAT-P9-06`](../features/FEAT-P9-06.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4, §6

## Description

Apply the AEAD to image bytes at rest and prove the integrity half adversarially — the half that is easy to leave untested because the confidentiality half looks like success on its own.

## Depends on

`STORY-P9-05-01`.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. **Image bytes at rest are ciphertext**, confirmed by `FEAT-P9-01`'s dump-scan audit finding no PE structures where they used to be. The scanner is the evidence, not the encrypting code's own assertion.
2. **A relocated block fails.** Take a valid sealed block, move it to a different address, and observe authentication failure. This is the anti-relocation property and it is the reason the AAD includes the address.
3. **A replayed block fails.** Take a block sealed in generation N and present it in generation N+1. This is the anti-replay property and the reason the AAD includes the generation.
4. **Both failures land in the existing fault path** with a spoor, and neither produces plaintext, a partial result, or a retry.
5. **The RT carve-out is respected and demonstrated**: no decode occurs on the live working set, and `D04`/`D05` parity against committed baselines is a hard gate, not a win axis.
6. **If compression is used**, the order is compress → encrypt → pad to fixed buckets, and the padding is not optional: without it, consumption becomes content-dependent and an observer watching memory usage learns about the plaintext — the CRIME/BREACH shape applied to memory, and "memory tracking" was named in the original threat.

## Explicitly out of scope

- The live working set — `FEAT-P9-07`.

## Tests

Not yet written — deferred until this Story starts. Host tests for the binding logic (relocation and replay are both expressible against an injected key); Tier 0 for the dump-scan confirmation and the `D04`/`D05` parity gate.

## Goals verified

G-SEC-2, G-SEC-13.
