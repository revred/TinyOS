# STORY-P9-04-01 — TPM 2.0 Transport, PCR Measurement, and Seal/Unseal

Status: **Specified, not yet started. Gated on `LE-09`.**
Feature: [`FEAT-P9-04`](../features/FEAT-P9-04.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4b, §4c

## Description

The mechanism: talk to a TPM 2.0, read and extend PCRs, seal a secret against a policy over them, and unseal it on a subsequent boot that measures the same.

## Depends on

Hardware with a TPM 2.0 and a measuring boot chain.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. **A working TPM 2.0 transport** (CRB or FIFO) with bounded, fail-closed command/response handling — a device that stops answering must produce a typed timeout, never a hang on the boot path.
2. **The required measurements are specified, not assumed.** Which PCRs, extended by whom, covering what. TinyOS does not measure itself into existence; this Story states what it needs the firmware/bootloader to have done and how it verifies that expectation was met.
3. **Seal and unseal round-trip on the same machine**, and the sealed blob is inert on a different one — demonstrated with two boards, not argued.
4. **A modified kernel image does not unseal.** Change one byte, boot, and observe the refusal. This is the A6 defence and it is the single most important observation in `EPIC-P9`; nothing else in the Epic is worth anything if this does not hold.
5. **`root_secret` never reaches a log, a spoor, a serial line or a debugger-visible static.** It exists between unseal and use and nowhere else, per [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4d rule 1.

## Explicitly out of scope

- The policy question — what to do when the unseal fails — is `STORY-P9-04-02`, deliberately separated because it is a *decision* and this is a *mechanism*.
- Key derivation is `FEAT-P9-05`.

## Tests

Not yet written — deferred until this Story starts. Host tests can cover command serialization and the response state machine; **everything that matters here is hardware-tier** and this Story cannot be Verified without it.

## Goals verified

G-SEC-15, and the first evidence ever produced for `SEC-01`.
