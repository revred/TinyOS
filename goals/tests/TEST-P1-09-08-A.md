# TEST-P1-09-08-A — A Gate That May Open Late Is Re-Read, Never Re-Trusted

Status: **In progress — host clauses Green 2026-08-03; clause 4 awaits the next boot**
Story: [`STORY-P1-09-08`](../stories/STORY-P1-09-08.md)
Tier: Host unit tests (eligibility, gate discipline across retries, late-settle pipeline) **plus** a Tier 1 boxed boot (pulse or sharpened confession)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by a re-probe. This Test raises no
timing, measurement or qualification claim; in particular no settle-time
constant exists to claim — that absence is clause 1's point.

## What this test is for

The confession boot measured the failure exactly: `DL_ACTIVE` clear at the
one moment discovery looked, both earlier gates up. Whether the bit rises
late or never, a single early read cannot tell — so the probe joins the
park loop's watch cadence. What must stay true while it does: the gate
discipline (a refused probe touches nothing downstream) and the release's
exactly-once contract, both of which held only because discovery ran once.
These clauses make them hold under repetition.

## Specification

### 1. Re-probe eligibility is total and refusal-only (`BND-06`)

**Given** every constructible `Discovery` outcome,
**then** each outcome short of a present GEM is due a re-probe and every
present GEM — any PHY outcome, any link state — is final; the decision is
matched exhaustively and contains no timing constant.

### 2. A refused probe touches nothing downstream (`PD-10`, `SEC-19`)

**Given** repeated probes against a controller that keeps refusing (any
rung),
**then** across all of them the GEM window and the RP1 GPIO registers are
never read or written — pinned with panicking doubles, the boot pass's
discipline surviving repetition unchanged.

### 3. A late settle runs the pipeline once, the release exactly once (`RCG-13`, `PD-07`)

**Given** a controller scripted to raise `DL_ACTIVE` only from the Nth
probe,
**then** the Nth pass validates identity, runs the reset release **exactly
once counted across every pass**, scans, reads the link; and the adopted
outcome drives the lamp code, the link watch and beacon eligibility exactly
as a boot-time success would have.

### 4. Board: the chain clears or the confession sharpens

**Given** the next boxed boot,
**then** either the lamp reaches the plain pulse (and the NIC watch owns
the story from there) or it holds the count of 3 through a minute of
once-per-second retries — on-silicon evidence that `DL_ACTIVE` requires
intervention, warranting the bring-up story.

### 5. What this test explicitly does **not** establish

- No second `TOS64-LINK/1` line — the serial report keeps its exactly-once
  contract and describes the first look only (named debt in the Story).
- No link bring-up of our own: this Story only waits well.
- No claim about how long `DL_ACTIVE` takes on any hardware.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red
first; then the Tier 1 boxed boot.

## Implementation location

- `os/src/hal-arm64/src/ethernet.rs` — `reprobe_due`, the park-loop
  re-probe composition, and the channel adoption on late success.

## Reports

To be filed with the boot.
