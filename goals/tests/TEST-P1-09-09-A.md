# TEST-P1-09-09-A — Belief Comes From the Readback, Even When We Wrote It

Status: **In progress — host clauses Green 2026-08-03; clause 4 awaits the next boot**
Story: [`STORY-P1-09-09`](../stories/STORY-P1-09-09.md)
Tier: Host unit tests (mapping arithmetic, write discipline, final verdict) **plus** a Tier 1 boxed boot (the count moves past 4)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-02`, `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by window programming. This Test
raises no timing, measurement or qualification claim.

## What this test is for

The count of 4 proved the firmware keeps the link but not the window. The
working system's answer is in the capture: program `WIN0` from `ranges`.
What must not change while we adopt it: the probe's hostile posture. A
written register is a hope; only its readback is a fact — so the write may
happen exactly once per pass, only for window-class refusals, and the
re-read's verdict is final.

## Specification

### 1. The programmed mapping is the capture's, pinned as arithmetic (`BND-17`, `BND-03`)

**Given** the five programming values,
**then** decoding them with the already-pinned decoder yields CPU base
`0x1F_0000_0000`, PCI base `0x0`, and a CPU limit covering
`RP1_WINDOW_MIN_SPAN` — asserted against the dmesg line
(`MEM 0x1f00000000..0x1ffffffffb -> 0x0000000000`) recorded in
`pios-ground-truth-2026-08-03.txt`, hardcoded-and-verified like every board
constant in this slice.

### 2. Only window-class refusals program, exactly once per pass (`PD-10`, `SEC-19`)

**Given** a probe pass over a scripted controller,
**then** `PortNotRc`/`PhyDown`/`LinkDown` return without one write (pinned
by a panicking write path); a window-base/-pci/-span refusal writes exactly
the five `WIN0` registers, in a pinned order, once — no other register, no
second burst.

### 3. The second verdict is final and honestly reported (`PD-07`, `BND-07`)

**Given** a controller that still refuses after programming,
**then** the pass returns that refusal with the offending readback and
performs no further writes; a controller that accepts reports the validated
window — belief from the re-read alone, never from the write.

### 4. Board: the confession moves past 4

**Given** the next boxed boot,
**then** the lamp leaves the count of 4 behind — the plain pulse, or a
deeper rung's count; either closes the window rung on silicon.

### 5. What this test explicitly does **not** establish

- No device-tree parsing — the mapping is a recorded constant (`BND-03`).
- No inbound (`dma-ranges`) reprogramming — transmit still uses the
  recorded `RP1_DMA_RAM_BASE` translation untouched.
- No claim that the firmware will never program the window itself on other
  EEPROM versions — the fallback is idempotent with that world.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red
first; then the Tier 1 boxed boot.

## Implementation location

- `os/src/hal-arm64/src/pcie.rs` — the programming values, the
  probe-then-program-then-final-verdict pass.
- `os/src/hal-arm64/src/ethernet.rs` — `discover` adopting the pass.

## Reports

To be filed with the boot.
