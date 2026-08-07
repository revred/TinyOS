# TEST-P1-09-03-A — The Beacon Is Exact Bytes and the Laptop Is the Witness

Status: **In progress — host clauses Green 2026-08-03; clause 6 awaits the board, the cable, and a laptop packet capture**
Story: [`STORY-P1-09-03`](../stories/STORY-P1-09-03.md)
Tier: Host unit tests (frame bytes, descriptor layout, bounded transmit state machine) **plus** a Tier 1 board run witnessed by a host packet capture
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`, `D20`
Security controls: `SEC-18`, `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-02`, `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: `D20` is selected as stated open debt
([`goals/assurance/open-debt.tsv`](../assurance/open-debt.tsv)) — the domain's
subsystem does not exist and none of its 25 guardrails can close on a beacon.
This Test raises no timing, measurement or qualification claim.

## What this test is for

The beacon is simultaneously the product's first visible win (a laptop detects
the board with no serial adapter) and the repository's first transmit DMA. Both
halves are held to exact bytes: the frame is pinned on the host before the board
ever sends it, so the packet capture is a byte-comparison, not an interpretation
— and the DMA grant is provably one buffer.

## Specification

### 1. The frame is exact bytes (`BND-03`)

**Given** the frame builder,
**then** destination broadcast, the fixed locally-administered source
`02:54:4F:53:36:34`, EtherType `0x88B5`, the `TOS64-PRESENT/1` payload line,
and minimum-size padding are produced by a pure function and pinned
word-for-word by host tests, with the sequence counter the only varying field.

### 2. The descriptor ring is pinned and points at one buffer (`BND-07`, `SEC-18`, `PD-10`)

**Given** the transmit ring builder,
**then** the two-descriptor layout (frame descriptor; wrap descriptor), the
64-bit address split, and the used/length/last-buffer control bits are pure
functions pinned by host tests, and every descriptor address is asserted to lie
inside the one static beacon buffer translated by the recorded RAM offset —
nothing on this path allocates (`LE-67` names the absent IOMMU).

### 3. The transmit state machine is bounded and fail-safe (`SEC-20`, `PD-07`, `RCG-13`)

**Given** the transmit sequence over the scripted seam,
**then** enable-transmit, start, and completion-poll are each bounded; scripted
no-completion, error-status, and retry-exhausted answers are each a distinct
driven rejection that permanently stops beaconing, reports `beacon=stopped
reason=…`, and leaves the board parked — fail-safe over keep-trying, proven by
a double that counts polls and records register order.

### 4. Receive is disabled, and that absence is tested (`RCG-01`, `BND-06`)

**Given** the full transmit sequence,
**then** the seam double asserts the receive-enable bit is never set and no
receive register is ever touched — absence tested, not inferred.

### 5. Speed follows the PHY, or the beacon does not run (`PD-12`)

**Given** `STORY-P1-09-02`'s negotiated answer,
**then** the MAC speed configuration is derived from it by a pure pinned
function, and a `link=down` or unresolved negotiation skips beaconing with the
skip reported — never a frame transmitted into a dead or mismatched link.

### 6. Board: the laptop sees the board (`BND-17`)

**Given** the board, the cable, and a stock packet capture on the laptop,
**then** the beacon appears repeating at its period, byte-identical to the
pinned frame apart from the sequence field, while `TOS64-LINK/1` reports
`beacon=running` and every protocol line and the splash stay unchanged.

### 7. What this test explicitly does **not** establish

- No receive path exists, so no discovery of the host, no ARP, no IP.
- No throughput, latency, or D20 guardrail claim — the domain is open debt.
- No claim about RGMII clock provisioning beyond the board run's own evidence;
  a silent capture with `link=up` is the named risk landing in the Report.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red first;
then the Tier 1 board run with a host packet capture.

## Implementation location

- `os/src/hal-arm64/src/gem.rs` — frame builder, descriptor layout, transmit
  state machine, beacon loop integration.
- `os/src/hal-arm64/src/boot.rs` — the beacon call strictly after the verdict
  and splash, inside the park loop's period.

## Reports

To be filed with the board run; the packet capture is the raw evidence.
