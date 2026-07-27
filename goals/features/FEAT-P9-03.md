# FEAT-P9-03 — Bounded `no_std` AEAD and KDF Primitives

Status: **Specified — no Story started**
Epic: [`EPIC-P9`](../epics/EPIC-P9.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4, §4b

## Description

The cryptographic primitives everything above this line in the dependency chain consumes: an AEAD whose nonce/AAD binds a block to its address and its epoch, and a KDF for the two-input derivation in [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4b.

Two constraints make this a real Feature rather than a dependency line in `Cargo.toml`. First, **`no_std`, no heap, no panic** — this kernel allocates from fixed pools and is built `panic = "abort"`. Second, and more demanding, **a declared WCET budget**: a primitive on a path the scheduler accounts for must have a bounded worst case, and a data-dependent one is a timing side channel as well as a scheduling hazard. Constant-time implementation is not a nicety here; it is both a security and a determinism requirement, and this project's own timing harness is the instrument that can check it.

## Crate(s) involved

A new `os/src/crypto/` library crate (host-testable, `no_std`), consumed by `kernel` and `exec`

## Depends on

`FEAT-P9-02` — an AEAD without unpredictable nonces is a construction with a fixed nonce per key, which is exactly the failure [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §6 identifies in Sharc.Blue's `oracle_encrypt` and legislates against in §4.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P9-03-01`](../stories/STORY-P9-03-01.md) | AEAD and KDF with published test vectors, constant-time behaviour, and measured budgets | Specified |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0** · subject **C0/C1** · boundary tests **BND-01, -02, -17**.

A primitive grants nothing and decides nothing: it transforms bytes and reports success or a typed failure. Authentication failure is a refusal that propagates, never a best-effort decrypt (`PD-12` fault containment, `RCG-13` blast radius). Key material passed in is wiped on the way out; the primitive holds no state between calls.

## Exit criteria

Its Story **Verified**: published test vectors pass, an authentication-failure path is exercised and refuses, no allocation occurs on any path, and a measured budget exists on the `D25` axis with the constant-time property argued from the implementation and checked as far as Tier 0 permits.

**Named debt on exit.** Constant-time behaviour cannot be established under TCG — QEMU does not model cache or branch predictors, so a timing-variation measurement there is a measurement of the emulator. The property must be argued structurally (no secret-dependent branches or table indices) and the empirical half deferred to `LE-09`.
