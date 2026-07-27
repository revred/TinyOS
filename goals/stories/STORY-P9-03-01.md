# STORY-P9-03-01 — AEAD and KDF with Vectors, Constant Time, and Measured Budgets

Status: **Specified, not yet started**
Feature: [`FEAT-P9-03`](../features/FEAT-P9-03.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4, §4b

## Description

Deliver the two primitives, with the evidence that they are correct, bounded and free of secret-dependent timing.

## Depends on

`STORY-P9-02-01`.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. **Published test vectors pass**, in full, for whichever constructions are chosen. Vectors are the only reason to believe a cryptographic implementation is correct; a home-grown round-trip test proves the implementation agrees with itself.
2. **Authentication failure refuses, and is tested adversarially.** A tampered tag, a tampered ciphertext, a tampered AAD and a replayed nonce each produce a typed failure and no plaintext — not truncated plaintext, not plaintext with a flag.
3. **No allocation on any path**, and no panic: every fallible operation returns a typed error, per `agent/CODING_STANDARDS.md`'s no-stringly-typed-errors and RT rules.
4. **No secret-dependent control flow or memory access.** Argued from the implementation, with the argument written down. This is the criterion most likely to be quietly skipped and hardest to recover later.
5. **A measured budget exists** on this project's own timing harness, and it is reported — not gated — until hardware exists, for the reason `STORY-P1-03-03`'s `D04` figure made concrete.
6. **Nonce misuse is structurally impossible, not merely documented.** The API must not let a caller supply a nonce twice under the same key; that is a type/ownership problem, and solving it in the type system is the difference between this and `oracle_encrypt`.

## Explicitly out of scope

- Key derivation *policy* — which surfaces get which subkeys, and when they rotate — is `FEAT-P9-05`.
- Any consumer.

## Tests

Not yet written — deferred until this Story starts. Host tests carry almost all of this: vectors, tamper cases, allocation assertions, and API-misuse compile-fail tests.

## Goals verified

G-SEC-15 (groundwork), G-SEC-13.
