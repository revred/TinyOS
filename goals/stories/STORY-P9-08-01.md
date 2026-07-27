# STORY-P9-08-01 — Randomized Image Base and Process Id, Fingerprint Constants Removed

Status: **Specified, not yet started**
Feature: [`FEAT-P9-08`](../features/FEAT-P9-08.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §6 (R5), §12

## Description

Make the two salt inputs that currently carry zero bits actually carry some, and remove the constants that identify a dump as TinyOS in one pass.

## Depends on

`STORY-P9-02-01`. [`STORY-P9-01-01`](STORY-P9-01-01.md) should land first, since the claim here is a reduction in dump-scan hits and that needs a measured before.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. **The image base is chosen at load time, not linked in.** It is currently hardcoded in 15 sites; every one becomes a value threaded from the loader, and a host test asserts no literal remains.
2. **Process ids are not sequential**, and the entropy of both inputs is measured and stated rather than assumed from the design.
3. **`CAPABILITY_TRAP_VIRT` stops being a fixed constant.** `0xdead_0000` is a fingerprint and, per [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4d rule 6, a value chosen to be memorable is a value chosen to be recognizable. It must stay unmapped and diagnosable — the whole point of `STORY-P1-03-03`'s trap is that `cr2` alone identifies a refused capability — so this trades one diagnostic property for another and the trade must be made deliberately, not by deleting the constant and discovering the diagnostic is gone.
4. **Dump-scan hits fall**, measured against `STORY-P9-01-01`'s baseline, with both numbers in the Report.
5. **Tier 0 evidence survives.** Fixtures asserting exact addresses, the `D04` measurement and the spoor journal all rest on reproducible boots. The deterministic mode from `STORY-P9-02-01` criterion 5 is what those use, and it must be impossible to enable in a shipping image.
6. **Randomization is never credited as secrecy.** Even fully randomized, these inputs carry roughly 25 and 16 bits — brute-forceable, and present in the dump anyway. They are salt diversity and nothing more, and the code says so where they enter the derivation.

## Explicitly out of scope

- Stack, heap and kernel-image randomization — separate work with separate costs.

## Tests

Not yet written — deferred until this Story starts. Host tests for the threading of the base through the loader and for the absence of literals; Tier 0 for the dump-scan reduction and baseline parity.

## Goals verified

G-SEC-2 (partial — A2 only), G-SEC-13.
