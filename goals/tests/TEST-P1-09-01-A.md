# TEST-P1-09-01-A — A Window the Firmware May Not Have Kept Is Hostile Territory

Status: **In progress — host clauses Green 2026-08-03; clause 6 awaits the board and the serial adapter**
Story: [`STORY-P1-09-01`](../stories/STORY-P1-09-01.md)
Tier: Host unit tests (config generation, scripted-seam probe, report line) **plus** a Tier 1 board run in both configurations (keep-line present and deliberately absent)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by a presence probe. This Test raises no
timing, measurement or qualification claim.

## What this test is for

The probe's honest failure mode is the whole point: a kept window answers with an
identity, a reset window answers with a data abort, and both must come back as
one diagnosable line. A probe that can hang, or that believes an all-ones float,
would spend the board session's credibility on a convenience.

## Specification

### 1. The generated card contents carry the keep-line (`BND-17`)

**Given** the `xtask pi5` build,
**then** the emitted `config.txt` contains `pciex4_reset=0` *and*, verbatim and
still pinned, `os_check=0` and `kernel=kernel8.img`; the placement instructions
name why the line exists; host tests pin the full generated contents.

### 2. The root complex answers before the window is touched (`PD-10`, `SEC-19`)

**Given** the probe over a scripted MMIO seam,
**then** no read through the `0x1F` window occurs unless the always-mapped PCIe2
controller registers first report RC mode, PHY link up, and data-link active,
*and* the outbound-window readback maps CPU `0x1F_0000_0000` onto RP1's
peripheral space — each missing bit and each mismatched window field a distinct
driven rejection resolving to `rp1=absent` with its reason, proven by a seam
double that records access order.

### 3. Identity before belief (`RCG-01`, `PD-10`)

**Given** a probe whose both gates passed,
**then** `rp1=present` is reported only after the module-identification readback
validates against the Cadence GEM identity, and all-ones (floating bus),
all-zeros, and wrong-module answers are each a distinct driven rejection. The
residual completer-abort case is named in the Story and lands in the ordinary
fault-report path — asserted here only as *named*, not simulated.

### 4. Every wait is bounded (`SEC-20`, `PD-07`)

**Given** the probe path,
**then** there is no unbounded loop anywhere in it; scripted never-settling
answers exhaust a typed budget and resolve to `rp1=absent reason=timeout`.

### 5. The report line and the protocol (`BND-17`)

**Given** a completed probe,
**then** exactly one `TOS64-LINK/1` line follows `TOS64-RESULT/1`, its fields
pinned by host tests, and every `FEAT-P1-07` protocol line stays byte-identical.

### 6. Board: both configurations diagnosable

**Given** the board and two staged cards (keep-line present / absent),
**then** the captures show `rp1=present id=0x…` and `rp1=absent` respectively,
with no hang and the splash unchanged in both.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/` and `os/src/xtask`),
written Red first; then the Tier 1 board runs.

## Implementation location

- `os/src/hal-arm64/src/` — probe, seam trait, guarded-read state machine,
  `TOS64-LINK/1` formatting.
- `os/src/xtask/src/pi5.rs` — `config.txt` generation.

## Reports

To be filed with the board runs.
