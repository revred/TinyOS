# TEST-P1-09-12-A — The Current Is Proven On Before Anyone Is Asked to Speak

Status: **In progress — host clauses Green 2026-08-03; clause 5 awaits the next boot**
Story: [`STORY-P1-09-12`](../stories/STORY-P1-09-12.md)
Tier: Host unit tests (pre-flight gate, enable-by-readback, bounded run poll, pipeline order)
**plus** a Tier 1 boxed-boot identity readback
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by a clock enable. The poll budget
is an attempt count, never a duration; no rate, divider, or lock time is
asserted anywhere (`ADR 0005` discipline, and the owner's standing
no-bench-constants rule).

## What this test is for

The identity rung read `0xDEAD` — fabric poison — and the live Pi OS
capture read `0x00070109` through the same window with two enable bits
set (`goals/reports/pios-ground-truth-2026-08-03.txt`). The rung this
test pins is the delta and nothing but the delta: prove the clocks block
answers, switch the two gateable Ethernet clocks on, believe only
readbacks, and only then let the pipeline ask the GEM who it is.

## Specification

### 1. The pre-flight gate refuses poison honestly (`SEC-19`, `BND-06`)

**Given** a clocks-block double whose `CLK_SYS_SEL` reads all-zeros,
all-ones, or `0xDEAD_xxxx` fabric poison,
**then** the rung refuses before any write reaches the block (the double
panics on write), the refusal is the pre-flight arm, and its sixteen
decisive bits are the readback's high half — poison spells `57005`.
A credible one-hot readback passes the gate.

### 2. Enable is a write believed only by readback (`SEC-19`, `PD-12`)

**Given** each gateable clock in turn (`clk_eth` at `0x64`, `clk_eth_tsu`
at `0x134`),
**then** the rung writes the pinned enable value exactly once per clock
per pass — `ENABLE` set, `AUXSRC` and `DIV_INT` at the architectural
defaults from the capture — and a readback without the enable bit is the
enable-refused arm carrying the readback's low half. A clock already
enabled and running is left untouched: zero writes on the happy re-probe.

### 3. The run poll is bounded and its refusal names the status (`SEC-20`, `PD-10`)

**Given** a double whose running-status bit never sets,
**then** the poll performs exactly its budgeted attempt count and no
more, refuses with the run-refused arm, and the decisive bits are the
final readback's high half. A double that sets the bit on attempt *k*
within budget passes after exactly *k* reads. No wait, no time constant.

### 4. The rung sits between enumeration and identity, and the confession speaks it (`PD-07`, `BND-17`)

**Given** the full discovery pipeline over doubles,
**then** the clocks block is not touched until every enumeration gate
has passed (a pre-enumeration double panics on clock access), the GEM
window is not read until the rung passes (the GEM double panics
otherwise), the three refusal arms earn blink codes 16, 17, 18 —
exhaustively matched, no code shared — each with its named detail, the
`TOS64-LINK/1` line names the refused rung, health never speaks, and
every previously pinned protocol line is byte-identical.

### 5. Board: the identity answers where the poison was

**Given** the next boxed boot with the card in the TOS64 role,
**then** the canvas report moves past `ID-MODULE`: either the identity
rung reads module `0x0007` and the pipeline proceeds to the PHY rungs,
or the confession spells one of the three new codes and its readback,
recorded in the session log, and the next story is chosen on that
number.

### 6. What this test explicitly does **not** establish

- No PLL programming, no rate or divider choice, no lock-time claim —
  the PLL tree is the firmware's and is only gated on, never driven.
- No claim about any clock other than the two the GEM consumes.
- No change to the beacon, the heartbeat, the sentence engine, or any
  pinned serial line.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red
first; then the Tier 1 identity readback on silicon.

## Implementation location

- `os/src/hal-arm64/src/rp1_clocks.rs` — the block pre-flight, the two
  enables, the bounded run poll (new module, pure over the MMIO seam).
- `os/src/hal-arm64/src/ethernet.rs` — the pipeline splice, the three
  new `Discovery` arms, blink codes 16–18, details, and the report line.
- `os/src/hal-arm64/src/board.rs` — the clocks-block window offset from
  the capture (`0x18000`).

## Reports

To be filed with the boxed boot.
