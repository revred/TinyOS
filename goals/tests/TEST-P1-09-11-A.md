# TEST-P1-09-11-A — Seven Groups a Human Counts the Same Way Twice

Status: **In progress — host clauses Green 2026-08-03; clause 4 awaits the next boot**
Story: [`STORY-P1-09-11`](../stories/STORY-P1-09-11.md)
Tier: Host unit tests (digit extraction, sentence timing, selection totality) **plus** a Tier 1 boxed-boot transcription
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-02`, `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by a readout. This Test raises no
timing, measurement or qualification claim; the cadences are human-eye
conventions, not measured periods.

## What this test is for

The identity rung refuses with a value, and the value is the diagnosis.
The lamp is the one live channel, so its language must satisfy one
property above all: **two honest counts of the same sentence agree.** That
is what fixed shape, decimal digits, the ten-blink zero, and a strictly
increasing gap hierarchy are for — and what these clauses pin.

## Specification

### 1. Digit extraction is exact at the boundaries (`SEC-19`)

**Given** the code and a sixteen-bit detail,
**then** the code yields exactly two groups (ones, tens) and the detail
exactly five (ones through ten-thousands), least-significant first, each
digit 1–9 as itself and **every zero as ten blinks** — pinned at 0, 9, 10,
and 65535.

### 2. The sentence is pure and its gaps are unmistakable (`SEC-20`, `PD-07`)

*Amended the same evening, after the first transcription attempt: a
sentence in flight is **never replaced** — a changed outcome is offered to
a latch and adopted only at a period boundary, so a flickering readback
reads as clean alternating sentences rather than hash. Health adopts
immediately (nothing is in flight); a cleared refusal finishes its sentence
and then pulses.*

**Given** a sentence and the 10 Hz tick index,
**then** the lamp value at every tick is a pure function; blink cadence,
inter-group dark and end-of-sentence dark are pinned and **strictly
increasing**, the sentence repeats with its pinned period, and the engine
performs no wait and touches nothing but the lamp.

### 3. Detail selection is total; health never spells (`BND-06`, `PD-10`)

**Given** every constructible refusal,
**then** each arm yields its named decisive sixteen bits — the wrong
module itself, a vendor dword's low half, a status word's low half, a
window address in whole megabytes — pinned arm by arm; and every known-PHY
outcome yields no sentence, keeping the plain 1 Hz pulse even while the
link watch waits.

### 4. Board: the readback is transcribed

**Given** the next boxed boot,
**then** seven counted groups decode to the identity rung's actual
readback, recorded in the session log; the next story is chosen on that
number.

### 5. What this test explicitly does **not** establish

- No multi-register dump — one refusal, sixteen bits, by design.
- No change to the serial protocol, the heartbeat, or the beacon.
- No claim about blink or gap durations beyond their strict ordering.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red
first; then the Tier 1 transcription.

## Implementation location

- `os/src/hal-arm64/src/ethernet.rs` — digit extraction, the sentence
  engine, detail selection, and the park-loop composition.

## Reports

To be filed with the transcribed boot.
