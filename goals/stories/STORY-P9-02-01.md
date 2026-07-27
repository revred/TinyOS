# STORY-P9-02-01 — Entropy Source with Health Tests and a Determinism Carve-Out

Status: **Specified, not yet started**
Feature: [`FEAT-P9-02`](../features/FEAT-P9-02.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §3, §7.5

## Description

Build the entropy source three later Features depend on, and build it so that adopting it does not cost this project its reproducible boots.

## Depends on

Nothing.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. **A typed, arch-neutral interface with an x86_64 backend**, following `hal::time::CycleSource`'s established split: the trait and its conformance suite are host-testable, and only the `RDSEED` loop is target-only.
2. **`RDSEED` failure is handled, because `RDSEED` is documented to fail.** It returns CF=0 under load when the entropy pool is drained. A bounded retry with a typed refusal on exhaustion — never a spin, never a silent fall through to `RDRAND`, never a zero.
3. **Continuous health tests, not an init-time check.** At minimum the repetition-count and adaptive-proportion tests (NIST SP 800-90B's shape). A source that degrades after boot must be caught after boot, and a detected failure disables the source and is a spoor — it does not quietly return worse bits.
4. **Extraction is bounded and charged.** A caller cannot exhaust the pool at the expense of an RT task; the cost is measured against a declared budget rather than assumed small.
5. **The determinism carve-out is a first-class mode, not a debug flag.** Fixtures assert exact addresses, and the `D04` measurement and spoor journal rest on reproducible boots. A seeded/deterministic mode must be explicit, must be impossible to enable in a shipping image, and — per [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4d — must be distinguishable from outside, since under a measured-boot regime a deterministic build measures differently and therefore never receives a key anyway.

## Explicitly out of scope

- Any consumer of the entropy. Nonces are `FEAT-P9-03`, salt is `FEAT-P9-05`, addresses are `FEAT-P9-08`.
- Any claim about entropy *quality* at Tier 0 — see the Feature's exit criteria.

## Tests

Not yet written — deferred until this Story starts. Host tests for the health-test state machines (feed them known-bad sequences and assert they trip), the retry/refusal logic, and the conformance suite; Tier 0 for the `RDSEED` backend actually producing bits under QEMU.

## Goals verified

G-SEC-15 (groundwork).
