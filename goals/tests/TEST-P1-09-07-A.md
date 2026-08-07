# TEST-P1-09-07-A — One Number, Counted in the Dark, Names the Broken Rung

Status: **In progress — host clauses Green 2026-08-03; clause 4 awaits the next boot**
Story: [`STORY-P1-09-07`](../stories/STORY-P1-09-07.md)
Tier: Host unit tests (mapping totality, tick-exact pattern, composition) **plus** a Tier 1 boxed-boot blink count
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-02`, `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by a blink code. This Test raises no
timing, measurement or qualification claim; the 100 ms tick is a cadence,
not a measured period, and the lamp remains an instrument, never evidence.

## What this test is for

The board can now prove it runs (the lamp) but not *why its wire is dead* —
the answer sits in `Discovery`'s arms with no live channel to carry it. This
test pins the smallest carrier that exists on proven hardware: a counted
blink pattern. Its entire value is that the count is trustworthy, which
makes exactness — mapping, tick sequence, and when it may speak — the whole
specification.

## Specification

### 1. The mapping is total, distinct, and refusal-only (`BND-06`, `SEC-19`)

**Given** every constructible `Discovery` outcome,
**then** each outcome short of a known PHY yields a distinct nonzero code
(each `LinkAbsent` rung, each `IdentityRefused` shape, `ReleaseStuck`,
`Absent`, `PortWedged`, unknown-PHY), a known PHY yields none regardless of
link state, and the mapping is matched exhaustively so a future arm fails to
compile rather than silently sharing a code.

### 2. The pattern is a pure function of tick and code (`SEC-20`, `PD-07`)

**Given** a code N and the 10 Hz tick index,
**then** the lamp value at every tick of the period is pinned — N blinks of
equal on/off width, then a trailing gap long enough that the count is
unambiguous to a human — the sequence repeats with the pinned period, and
the engine performs no wait, no read, and no write beyond the lamp bit.

### 3. Composition: the confession never displaces the pulse (`PD-10`)

*Amended 2026-08-03 evening by `STORY-P1-09-11`: the refusal's lamp form is
now the spelled seven-group sentence (code digits then detail digits), not
the single count; this clause's obligation — refusal speaks, health pulses,
nothing re-derives the outcome per tick — is unchanged and its test updated
in place. The code numbering below is unchanged; the digits spell it.*

**Given** the park loop's per-tick lamp decision,
**then** a refused discovery drives the pattern, a known PHY drives the
plain 1 Hz pulse — including while the link watch is still waiting for the
wire — and the decision consumes the discovery outcome it was given at park
time, never re-deriving it.

### 4. Board: the count is taken

**Given** the next boxed boot on the proven board,
**then** a human counts N through the case seam and the session log records
it — the Ethernet chain's first on-silicon self-diagnosis, and the name of
the rung the next story fixes.

### 5. What this test explicitly does **not** establish

- No register readbacks — one number per boot by design; deeper evidence is
  the SD recorder's future scope.
- No timing claim of any kind for blink widths or gaps.
- No change to any serial protocol line, the splash, the heartbeat, or the
  beacon — the confession rides the lamp alone.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red
first; then the Tier 1 blink count.

## Implementation location

- `os/src/hal-arm64/src/ethernet.rs` — the refusal-to-code mapping, the
  pattern engine, and the park-loop composition.

## Reports

To be filed with the counted boot.
