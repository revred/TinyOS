# STORY-P1-09-11 — The Spelling: the Lamp Reads Out the Refused Word in Decimal

Status: **Verified (functional) 2026-08-05 — all four acceptance criteria met. Criteria 1, 2 and 3 host half Green 2026-08-03 (digit extraction pinned including the ten-blink zero, the seven-group sentence pinned tick-by-tick, detail selection total per refusal arm, health untouched). **Criterion 4 Green on silicon 2026-08-03 late** (`BOARD VERDICT 1`): the lamp spelled seven counted groups, the owner transcribed them, and they decoded to **code 16, detail 57005**. Both halves of the criterion then held. The decoded value named the identity rung's *actual* readback — 57005 is `0xDEAD`, the poison word, which is what turned "the identity read is wrong" into "the whole RP1 memory window reads poison, config space excepted". And **the next fix was chosen on that number rather than on conjecture**: 57005 refuted the per-peripheral clock-gating theory the same hour it was spelled, and `STORY-P1-09-13` was written against the endpoint BARs instead. This is the readout design doing exactly the work it was specified for — a board with no serial line reported a sixteen-bit register value to a human through a single LED, and that value redirected the Feature. The canvas carried the same sentence in text on the same boot, so the lamp reading was cross-checked rather than trusted alone. **Assurance state remains `specified` and this Story is NOT release-assured**: 0 qualified platforms.**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: the 2026-08-03 enumeration boot — the count held at the identity rung (flickering 8/9) and the owner ordered the readout redesigned: decimal digits, ones first, never a sixteen-blink group

## Description

The counts have carried this session rung by rung — 3, then 4, then 9 —
but the identity rung refuses with a *value* the count cannot carry, and
the owner's verdict on nibble counting was right: sixteen blinks is not a
number a human counts twice the same way. So the lamp learns decimal, on
the owner's design: least-significant digit first, one group per digit,
**zero spelled as ten blinks** so no digit is ever silence.

The sentence is fixed-shape so a transcription is never ambiguous: seven
groups per period — two for the refusal code (ones, tens), five for the
decisive sixteen bits of the refused readback (ones through ten-thousands,
`0..=65535`). Blinks within a group keep the proven 300 ms cadence; groups
are separated by a distinctly longer dark, and the sentence ends in a
longer dark still. Every refusal arm selects its decisive detail — the
wrong module field itself, a vendor readback's low half, a status word's
low half, a window address in megabytes — pinned arm by arm. Health is
untouched: a known PHY still pulses plainly at 1 Hz, and the sentence
exists only where a refusal does.

This supersedes `STORY-P1-09-07`'s single-count pattern as the refusal
language (the code numbering is unchanged — the digits spell the same
codes); `TEST-P1-09-07-A` is amended to note the composition's successor.

## Depends on

- `STORY-P1-09-07` — the code numbering, the pattern primitive and the
  refusal-only discipline, all carried forward.
- `STORY-P1-09-08` — re-probe passes update the sentence live.

## Acceptance criteria

1. **Digit extraction is exact.** Least-significant first, zero spelled as
   ten, `0..=65535` always exactly five detail digits and the code always
   exactly two — pinned across boundary values (0, 9, 10, 65535).
2. **The sentence is pure and pinned tick-by-tick.** Group cadence,
   inter-group dark and end-of-sentence dark are distinct by construction
   and pinned for at least one full sentence; the engine is a pure function
   of (sentence, tick) with no wait and no state.
3. **Detail selection is total and health is untouched.** Every refusal arm
   yields its named sixteen decisive bits (pinned arm by arm); every known
   PHY yields no sentence and keeps the plain pulse — including while the
   link watch waits.
4. **Board: the readback is transcribed.** The next boot yields seven
   counted groups; the decoded value names the identity rung's actual
   readback, and the next fix is chosen on that number rather than on
   conjecture.

## Named debt this Story leaves open

- One sentence carries one refusal's sixteen bits; the full multi-register
  dump remains the SD recorder's future scope.

## Progress, 2026-08-03

| Criterion | State |
|---|---|
| 1 — digit extraction | **Green.** Boundary values pinned; zero is ten blinks everywhere. |
| 2 — sentence pinned | **Green.** Full-sentence tick table; gap hierarchy asserted strictly increasing. Amended after the first transcription attempt: the latch guarantees a sentence in flight is never replaced — the flickering identity readback had swapped sentences mid-read, garbling the count; now flicker reads as clean alternating sentences. |
| 3 — total selection, health untouched | **Green.** Arm-by-arm pins; known-PHY passes yield no sentence. |
| 4 — board | **Blocked on the next power-on.** |

## Tests

[`TEST-P1-09-11-A`](../tests/TEST-P1-09-11-A.md) — written before
implementation, per the TDD mandate.
