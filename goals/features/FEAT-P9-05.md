# FEAT-P9-05 — Key Lifecycle: Derive, Use, Wipe

Status: **Specified — no Story started. Gated on `LE-09`.**
Epic: [`EPIC-P9`](../epics/EPIC-P9.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4b, from [`session/hand-2026-07-28/08-memory-confidentiality-review.md`](../../session/hand-2026-07-28/08-memory-confidentiality-review.md) §5

## Description

The section the proposal's first draft was missing, and the one that closes its second trap.

That draft specified the AEAD construction with care and never said where the key lives. Against an offline analyst holding a memory dump that is the whole question: **the dump contains the key next to the ciphertext**, and an at-rest AEAD keyed from `.bss` buys A3 approximately nothing.

The construction is a two-input derivation. Everything about a process that is already public goes in as salt; exactly one input is secret and comes from hardware:

```
process_key = KDF(root_secret,
                  image_base ‖ image_size ‖ layout_hash ‖ process_id ‖ teardown_generation)
```

The metadata binds — relocate a block and it fails, replay an old one and it fails — and it conceals nothing, because every value in it is in the dump. It carries **zero bits** of secrecy and must never be credited with any. `root_secret` carries all of it.

Derive at the point of use and wipe, rather than holding a key ring resident: R2's shrink-the-puddle argument applied to the key itself, and the direct analogue of Sharc.Blue dropping its `Chapter` handle after a single read. Per-surface subkeys (image-at-rest, spoor, future crash dump, IPC) stop a compromise crossing surfaces and make `FEAT-P9-01`'s invariant auditable per surface instead of as one global claim. Rotation keys to `TeardownGeneration` — one epoch counter, not two.

## Crate(s) involved

`os/src/kernel/` (the lifecycle and its wipe discipline), `os/src/crypto/`, `os/src/exec/` (per-process derivation at load)

## Depends on

`FEAT-P9-02` (salt diversity), `FEAT-P9-03` (the KDF), `FEAT-P9-04` (`root_secret` — without it this is a reversible transform whose parameters ship alongside the ciphertext).

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P9-05-01`](../stories/STORY-P9-05-01.md) | Per-process derivation, per-surface subkeys, wipe discipline, generation-keyed rotation | Specified |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0** · subject **C0/C1** · boundary tests **BND-02, -17, -20**.

A derived key is bytes, not authority: holding one lets a subject read what it was already entitled to read and nothing more. Per-surface separation is the containment claim (`PD-11` non-increasing provenance and partitioned state, `PD-13` revoke-wipe-advance, `RCG-13` blast radius).

## Exit criteria

Its Story **Verified on hardware**: keys derive correctly and reproducibly on the same machine, differ across machines, differ across surfaces and generations, and are provably absent from memory after use — checked by `FEAT-P9-01`'s dump-scan audit rather than by the code that wiped them.

**Named limit, carried into every Report.** Derive-use-wipe shrinks residency; it does not eliminate it. During the operation the key is in registers and possibly on the stack, and a dump captures both. Only `FEAT-P9-07` protects it at the moment of use.
