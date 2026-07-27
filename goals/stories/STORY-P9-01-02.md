# STORY-P9-01-02 — Wipe the Staging Arena Once the Image Is Mapped and Sealed

Status: **Specified, not yet started — landable in the current phase**
Feature: [`FEAT-P9-01`](../features/FEAT-P9-01.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §6 (R2's cheap half)

## Description

The single most dump-friendly artifact in this system is a static `.bss` buffer holding a fully decoded PE — headers, section table, import name strings and all — at a link-time-fixed address, for the entire lifetime of the process.

It is also **already dead**. `AddressSpace::create` copies image bytes through the staging alias into the task's own mapped frames; from the moment sealing closes that alias, nothing reads the arena again. It sits there as plaintext for the rest of the run purely because nobody wipes it.

In the shipping `os` image the arena is 16KiB (the capability probe). In `first-task-fixture` it is 8.3MiB of `blue-sharc.exe`. Either way the fix is a memset and a stated invariant, and it closes the largest plaintext puddle in the image for a cost that does not register against any budget.

**This is R2's cheap half and only its cheap half.** R2's *sliding-window* form — decoding into a small window rather than one large puddle — needs decode-on-fault, which collides with `kernel::fault` deliberately having no `Resume` arm. That module's own doc comment says the day a genuine recoverable case exists it arrives with its own Story, its own enumeration and its own test. It does not arrive attached to a memset.

## Depends on

Nothing to implement. [`STORY-P9-01-01`](STORY-P9-01-01.md) should land first so the result is *measured* rather than asserted — a wipe verified only by the code that performed it is exactly the shape of claim this project does not accept.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. **The arena is zero after load.** Once the image is mapped and `seal_kernel_alias` has run, every byte of the staging buffer reads zero. Checked in the fixture by reading it back, and — the part that matters — checked externally by `check-dump-residue` finding no PE structures at that address in a guest dump.
2. **The wipe is not optimizable away.** A plain `= [0; N]` over a buffer nothing reads again is precisely what a compiler is entitled to delete. The write must be volatile (or otherwise explicitly ordered), and a host test must assert the bytes are actually zero through a channel the optimizer cannot fold — this is the defect that would make the whole Story a no-op while every assertion in it passed.
3. **The invariant is stated where the arena is declared**, not only where it is wiped, so the next person to add a staging buffer sees the rule at the point they would otherwise break it.
4. **Ordering is explicit and tested: wipe after sealing, never before.** Sealing re-protects the kernel's view of every frame the task maps non-writable; the loader writes through the staging alias before that. A wipe placed earlier either faults under `CR0.WP` or destroys bytes the loader still needs — and the second is silent. `STORY-P1-03-03` already established the patch-then-seal ordering as load-bearing; this extends the same sequence by one step.
5. **The dump-scan count drops, measurably.** The `07` §9 dump-scan-hits axis is lower after this Story than the baseline `STORY-P9-01-01` recorded, and the Report quotes both numbers. "The wipe happened" is an implementation detail; "the analyst finds less" is the claim.

## Explicitly out of scope

- R2's sliding-window decode, per the Description.
- The fixed constants (`0x1_4000_0000`, `0xdead_0000`) that will still fingerprint a dump after this Story — `FEAT-P9-08`.
- Any change to the mapped image itself. The task's own frames still hold plaintext; that is the live working set, and only hardware memory encryption (`FEAT-P9-07`) touches it.

## Tests

Not yet written — deferred until this Story starts. Host tests for the wipe primitive (including the optimizer-elision case in criterion 2), plus a Tier 0 dump-scan comparison against `STORY-P9-01-01`'s recorded baseline.

## Goals verified

G-SEC-2, G-SEC-13.
