# STORY-P9-01-01 — The Dump-Scan Audit

Status: **Specified, not yet started — landable in the current phase**
Feature: [`FEAT-P9-01`](../features/FEAT-P9-01.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §6 (R1's acceptance test), from [`08`](../../session/hand-2026-07-28/08-memory-confidentiality-review.md) §10

## Description

Turn "no plaintext survives teardown" from a property that happens to hold into an invariant with a number behind it.

`PD-13` teardown wipes staged frames and advances the generation before reuse; `PD-12` keeps addresses, error codes and register content out of spoor. Together those give real anti-forensics — but nothing measures it, so nobody knows when it stops being true. This Story builds the instrument: a `xtask` command that boots a fixture under QEMU with guest memory dumped to a host file, scans that dump for the structures an analyst would look for, and reports a count.

The precedent is Sharc.Blue's own verification steps, which transfer directly:

> *"Binary inspection: `grep -ao "pattern" target/release/blue-sharc.exe` finds no recognizable envelope/key/crypto strings."*
> *"File inspection: `xxd .sharc/cold.start | head` shows no magic bytes, no JSON, no recognizable structure."*

**This instrument is load-bearing well beyond R1.** `07` §11.1 says the whole randomization argument — the claim that drives the recommendation ordering — is falsifiable by relocating the image base and measuring how much harder identification becomes. That measurement is a diff of two dump-scan counts. Without this command that falsification is rhetoric; with it, it is an experiment. Every later Feature in `EPIC-P9` is read through the same instrument.

## Depends on

Nothing. QEMU can dump guest memory today, and the `os` fixture already runs a real workload to termination.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. **`cargo run -p xtask -- check-dump-residue` exists, is deterministic, and fails closed.** It boots a named fixture with guest memory captured, scans the dump, and exits non-zero on any hit. A missing or unreadable dump is a harness error (exit 2), never a silent pass — the same exit-code discipline `qemu-x86_64` and `check-timing-regression` already use.
2. **The scanned fingerprint set is declared, not ad hoc.** At minimum: the PE `MZ`/`PE\0\0` signatures, section names (`.text`, `.rdata`, `.data`), the import name strings the loader resolves, the image base `0x1_4000_0000`, and `CAPABILITY_TRAP_VIRT` `0xdead_0000`. Each entry names what an analyst learns from finding it, so the list is reviewable rather than a bag of constants.
3. **It reports counts and offsets, not a bare boolean.** "Three hits at these offsets" is a diagnosis; "failed" is not. The count is `EPIC-P9`'s declared **dump-scan hits** axis (`07` §9) and later Stories are scored against it.
4. **It is proven able to fail.** A deliberate positive control — scanning a dump taken *before* teardown, or with a known plaintext marker planted — must produce hits. The same "prove the gate can fail" discipline `fixture-broken-boot` established for boot and `--inject-regression` for the timing gate. A scanner that has only ever reported zero is a scanner nobody has evidence for.
5. **A baseline is recorded, and it is expected to be non-zero.** Today's `os` image *will* have hits: the image base is hardcoded in 15 sites and the trap constant is fixed. The honest first result is a documented count that later Stories reduce — not a green tick obtained by scanning for things that were never there.

## Explicitly out of scope

- Any change to what the kernel does. This Story adds an instrument and changes no runtime behaviour.
- Fixing the hits it finds. Reducing them is `STORY-P9-01-02` (the staging arena) and `FEAT-P9-08` (the fixed constants).

## Tests

Not yet written — deferred until this Story starts. Host tests for the scanner's own matching logic (a scanner with a bug reports zero hits and looks like success, so its matcher needs pinning against a synthetic dump with known contents at known offsets), plus a Tier 0 run producing the baseline count.

## Goals verified

G-SEC-2 (the measured half), G-SEC-14.
