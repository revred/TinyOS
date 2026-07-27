# FEAT-P9-06 — At-Rest AEAD Bound to (Address, Generation)

Status: **Specified — no Story started. Gated on `LE-09`.**
Epic: [`EPIC-P9`](../epics/EPIC-P9.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4, §6 (R3)

## Description

Encryption alone gives confidentiality and not integrity, and the gap between them is where this Feature lives. An attacker who can write memory does not need the key: they can **replay** a previously-valid ciphertext block or **relocate** one to a different address, and a naive decryptor accepts both. This is not theoretical — it is why AMD had to add SEV-SNP's Reverse Map Table on top of SEV/SEV-ES, whose encryption was sound and whose integrity was not.

So every block's AEAD nonce/AAD binds it to **its address and its epoch**: valid only where and when it was written. TinyOS already has the epoch — `TeardownGeneration`, landed for `PD-13` — and reusing it keeps the invariant in one place.

Scope is *at rest* only: image bytes before and after use, plus any future crash-dump or journal export (none exists today — verified). The live working set is deliberately excluded, per [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §5's RT carve-out, and is `FEAT-P9-07`'s subject.

## Crate(s) involved

`os/src/exec/` (image bytes at rest), `os/src/crypto/`, `os/src/kernel/` (journal/export surfaces when they exist)

## Depends on

`FEAT-P9-05` — an at-rest AEAD keyed from `.bss` is [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4's trap 2 and buys A3 nothing.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P9-06-01`](../stories/STORY-P9-06-01.md) | Sealed image bytes at rest, with replay and relocation both rejected | Specified |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0** · subject **C0/C1/C2** · boundary tests **BND-04, -05, -17**.

A failed authentication is a refusal that propagates into the existing fault path, never a best-effort decrypt and never a partial result (`PD-04` executable memory is sealed, `PD-12` fault containment, `PD-13` revoke-wipe-advance, `RCG-10` exact fresh process mapping, `RCG-11` executable seal).

## Exit criteria

Its Story **Verified**: image bytes at rest are ciphertext; a relocated block fails authentication; a replayed block from a previous generation fails authentication; and both failures land in the fault path rather than producing plaintext. Compression, if present, is **compress-then-encrypt with padding to fixed buckets** and is credited with zero confidentiality.
