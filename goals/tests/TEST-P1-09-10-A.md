# TEST-P1-09-10-A — Knock Before Entering, and Believe Only Who Answers

Status: **In progress — host clauses Green 2026-08-03; clause 4 awaits the next boot**
Story: [`STORY-P1-09-10`](../stories/STORY-P1-09-10.md)
Tier: Host unit tests (value pinning, sequence discipline, refusal shapes) **plus** a Tier 1 boxed boot (the count moves past 9)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by enumeration. This Test raises no
timing, measurement or qualification claim.

## What this test is for

The count of 9 proved memory reads reach *something* that is not RP1: the
bridge was never told where to forward them. The fix writes config space —
the most authority-shaped writes this Feature has made — so the discipline
is the specification: verify who you are talking to before every write,
mask the status half you do not own, and refuse honestly in the pinned
vocabulary when the answer is wrong.

## Specification

### 1. Every programmed value is the capture's, pinned (`BND-17`, `BND-03`)

**Given** the enumeration constants,
**then** the bus-number dword (`0x0001_0100`), the forwarding-window dword
(`0x0040_0000` ⇒ bus `0x0..0x4fffff`), the command bits, the ECAM index for
bus 1 device 0 (`1 << 20`), the config-access register offsets
(`0x9000`/`0x8000`, inside the mapped controller window), and both vendor
identities (`0x14e4`, `0x1de4`) are asserted citing the recorded `lspci`
lines and the `pcie-brcmstb.c` encoding.

### 2. The sequence is exact, gated, and masked (`PD-10`, `SEC-19`)

**Given** the introduction over a recording double whose command registers
carry hostile write-one-to-clear status bits,
**then** the root vendor is read before any write and a wrong answer
refuses with **zero** writes; the writes that follow a right answer are
exactly bus numbers, forwarding window, bridge command, endpoint-index,
endpoint command — in order, with both command writes carrying zeros in the
status half; and a wrong endpoint vendor refuses after the bridge setup
with **no endpoint write**.

### 3. New refusals are honest end-to-end (`BND-06`, `BND-07`)

**Given** the two vendor refusals,
**then** each carries its raw readback through the `TOS64-LINK/1` line in a
pinned shape (`reason=root-vendor` / `reason=endpoint-vendor`) and counts
14 / 15 on the lamp, with the distinctness test extended so no existing
code is shared.

### 4. Board: the confession moves past 9

**Given** the next boxed boot,
**then** the lamp leaves 9 behind — the plain pulse or a deeper count;
either closes the routing rung on silicon.

### 5. What this test explicitly does **not** establish

- No ECAM/MCFG, no bridge traversal, no device discovery — one controller,
  one endpoint, both named in advance (`LE-10` untouched).
- No BAR programming: RP1's peripheral BAR is a fixed aperture at bus zero
  (`lspci` `[virtual]`), unwritten by Linux and by us.
- No interrupt, no I/O space, no prefetchable window.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red
first; then the Tier 1 boxed boot.

## Implementation location

- `os/src/hal-arm64/src/pcie.rs` — constants, the introduction sequence,
  the two vendor gates.
- `os/src/hal-arm64/src/ethernet.rs` — pipeline adoption, report shapes,
  lamp codes.

## Reports

To be filed with the boot.
