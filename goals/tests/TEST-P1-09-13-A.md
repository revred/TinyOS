# TEST-P1-09-13-A — A Device Is Told Where to Listen, and Believed Only When It Repeats It Back

Status: **In progress — every clause Green 2026-08-04; clause 5 answered on silicon in its success arm (`RP1=PRESENT ID=0x0109 PHY=0x600D84A2` on the canvas; the wire trained to 1000 Mbps at 01:27:03 by the laptop's linkwatch)**
Story: [`STORY-P1-09-13`](../stories/STORY-P1-09-13.md)
Tier: Host unit tests (sizing gate, assignment readback, write order, idempotence, confession wiring)
**plus** a Tier 1 boxed-boot readback
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-02`, `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by an address assignment. No timing
claim anywhere; the sizing probe is a register protocol, not a wait.

## What this test is for

The conviction capture (09A) proved the inbound chain differs from the
working system in exactly one register class: the endpoint's BARs, which
Linux assigns and the firmware does not. This test pins the repair: size
each BAR by the architectural probe, assign the captured bus addresses,
believe only masks and readbacks, never blink a live window, and speak
every refusal as its own number.

## Specification

### 1. Sizing is probe-first and its masks are the silicon's (`SEC-19`, `BND-06`)

**Given** an endpoint whose BARs do not hold their pinned addresses,
**then** each BAR is written all-ones exactly once before any assignment,
and the readback (flag bits masked) must equal the pinned mask —
`0xFFFF_C000` (BAR0, 16 KiB), `0xFFC0_0000` (BAR1, 4 MiB),
`0xFFFF_0000` (BAR2, 64 KiB) — with a zero, all-ones, or otherwise wrong
answer refusing as the mask arm, carrying the masked readback, and
attempting no assignment against that BAR.

### 2. Assignment is believed only from readback (`SEC-19`, `PD-12`)

**Given** a sized BAR,
**then** it is written its pinned bus address exactly once —
`BAR0 = 0x0041_0000`, `BAR1 = 0x0000_0000`, `BAR2 = 0x0040_0000` — and a
readback disagreeing with the assignment refuses as the held arm with the
readback. BAR1's zero is believed only in conjunction with clause 1's
mask having answered.

### 3. The order is pinned and a settled pass writes nothing (`PD-07`, `BND-17`)

**Given** one enumeration pass,
**then** the exact write sequence is pinned: bridge setup, the config
index, then per BAR (0, 1, 2 in order) the all-ones probe and the
assignment, then — strictly last — the endpoint memory-enable. **Given**
a pass whose BARs already hold their pinned addresses, **then** zero BAR
writes occur and the sizing probe never runs — a live window is never
blinked from the re-probe loop.

### 4. The confession speaks the new rungs and nothing else changes (`SEC-20`, `PD-10`)

**Given** each refusal,
**then** the mask arm counts 19 and the held arm 20 — exhaustively
matched, no code shared — each spelling the readback's high half;
`TOS64-LINK/1` names them `bar-silent` and `bar-held` with the full
readback; a refusal upstream (vendor gates) still leaves the BARs
untouched; and every previously pinned protocol line is byte-identical.

### 5. Board: the window is claimed

**Given** the next boxed boot with the card in the TOS64 role,
**then** the canvas walks past `CLK-SILENT`: the clock rung reads a
credible `CLK_SYS_SEL`, its enables land, and the identity rung answers
module `0x0007` — or the confession names codes 19/20 with the actual
readback, recorded in the session log, and the next fix is chosen on
that number.

### 6. What this test explicitly does **not** establish

- No resource allocation — three fixed addresses from one capture.
- No claim about RP1's internal inbound translation (09A Appendix D's
  contingency, exercised only if this rung's success does not clear the
  poison).
- No change to the window registers, the clock rung, or any pinned line.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red
first; then the Tier 1 readback on silicon.

## Implementation location

- `os/src/hal-arm64/src/pcie.rs` — the BAR constants, the size/assign
  rung inside `enumerate`, the two new `LinkAbsent` arms.
- `os/src/hal-arm64/src/etherrors.rs` — codes 19/20 and their decisive
  halves.
- `os/src/hal-arm64/src/ethernet.rs` — the `bar-silent`/`bar-held`
  report reasons.

## Reports

To be filed with the boxed boot.
