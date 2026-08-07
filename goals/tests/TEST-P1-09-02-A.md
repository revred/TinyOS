# TEST-P1-09-02-A — The PHY Is Identified Before It Is Believed

Status: **In progress — host clauses Green 2026-08-03; clause 6 awaits the board and the serial adapter**
Story: [`STORY-P1-09-02`](../stories/STORY-P1-09-02.md)
Tier: Host unit tests (MDIO framing, bounded polls, identity and link decisions) **plus** a Tier 1 board run with the cable in and out
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-02`, `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by a management-port conversation. This
Test raises no timing, measurement or qualification claim.

## What this test is for

The device half of "the cable is the signal": before this Story the board cannot
see a cable at all. The management port is the first conversation the kernel has
with silicon it did not boot from, so the discipline is identity-first — no PHY
register is acted on until the PHY has proven who it is — and every poll is a
budget the seam double can exhaust on the host.

## Specification

### 1. Clause-22 framing is exact bits (`BND-03`)

**Given** the MDIO frame builder,
**then** read and write frames for every register/address combination are pure
functions pinned by host tests — start bits, opcode, PHY address, register
address, turnaround — with no frame construction anywhere else.

### 2. The management port is enabled, used, and disabled (`PD-10`)

**Given** the GEM management sequence over the scripted seam,
**then** the management-port-enable bit is set before the first transaction and
cleared after the last, the MDC divisor is configured conservatively before any
frame, and the seam double asserts no register outside the management set is
touched.

### 3. Every transaction is bounded (`SEC-20`, `PD-07`)

**Given** the idle-bit poll,
**then** it is a bounded countdown with a typed timeout; a scripted
never-idle answer exhausts the budget and resolves to `phy=absent
reason=timeout`, and the double asserts the poll count equals the documented
budget.

### 4. Identity before belief (`RCG-01`)

**Given** the identifier readback,
**then** the expected Broadcom identity is validated before any further
register is read; all-ones, all-zeros, and unknown identities each resolve to a
distinct reported outcome (`phy=absent` / `phy=unknown id=0x…`) that stops the
conversation there.

### 5. Link state is latched-aware (`BND-17`)

**Given** a validated PHY,
**then** the basic-status register is read twice and the second read is the
reported answer; `link=up` carries the negotiated speed/duplex resolution and
`link=down` is an honest outcome, not an error; the `TOS64-LINK/1` line's
fields are pinned by host tests and the `FEAT-P1-07` protocol lines stay
byte-identical.

### 6. Board: cable in, cable out

**Given** the board, a peer-to-peer cable, and a live laptop port,
**then** one capture shows `link=up` with the negotiated rate and one shows
`link=down`, with splash and protocol unchanged in both.

### 7. What this test explicitly does **not** establish

- No traffic, no MAC configuration, no DMA — `STORY-P1-09-03`'s territory.
- No timing figure; no claim that autonegotiation itself is driven by TinyOS
  (the PHY negotiates by hardware default; this Story only reads the result).

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red first;
then the Tier 1 board run.

## Implementation location

- `os/src/hal-arm64/src/gem.rs` — MDIO framing, management sequence, identity
  and link decisions, report fields.
- `os/src/hal-arm64/src/board.rs` — transcribed GEM window constants with their
  source revisions and pinning tests.

## Reports

To be filed with the board run.
