# TEST-P1-09-04-A — A Reset Line's First Driven Value Is the Assertion

Status: **In progress — host clauses Green 2026-08-03; clause 5 awaits the board link watch**
Story: [`STORY-P1-09-04`](../stories/STORY-P1-09-04.md)
Tier: Host unit tests (sequence order, address pinning, bounded waits, pipeline placement) **plus** a Tier 1 board run (link watch, and the capture when serial works)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-02`, `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by a reset release. This Test raises no
timing, measurement or qualification claim.

## What this test is for

`LE-68`'s evidence: a PHY that is never released is indistinguishable, from
the far end of the cable, from a PHY that does not exist. The release is a
five-step register dance on transcribed addresses nobody in this repository
has watched work, so the host tests pin the dance and the board run is the
only thing allowed to call it real.

## Specification

### 1. The transcriptions are pinned with their sources (`BND-17`)

**Given** the bank-1 constants,
**then** `io_bank1`/`sys_rio1`/`pads_bank1` offsets, the pin-4-of-bank-1
arithmetic for GPIO 32, the atomic alias offsets, and the active-low polarity
with its 5 ms hold are asserted against the recorded source revisions
(`bcm2712-rpi-5-b.dts`, the RP1 bank stride) the same way `board.rs` pins the
UART and PCIe transcriptions.

### 2. The sequence is exact and glitch-ordered (`PD-10`, `SEC-19`)

**Given** the release over a recording seam double,
**then** the writes land in exactly this order — pad RMW with output-disable
cleared and no unrelated bit disturbed, RIO out-low via the clear alias, RIO
output-enable via the set alias, function-select RMW to RIO preserving every
field it does not own, the hold, out-high via the set alias, the settle — and
nothing else is touched: no other pin's bit, no other bank's register.

### 3. The waits are bounded (`SEC-20`, `PD-07`)

**Given** the hold and settle waits,
**then** both run on the bounded counter-tick wait; a scripted stuck counter
aborts the release with a reported reason instead of hanging the boot.

### 4. Pipeline placement (`RCG-13`)

**Given** the discovery pipeline,
**then** the release runs exactly once, strictly after the GEM identity
validates and strictly before the management port opens — and a pipeline that
never reaches identity never touches the GPIO registers at all.

### 5. Board: the wire wakes up

**Given** the release in the image, the cable, and the laptop,
**then** the NIC link watch records a training transition where every earlier
attempt was flat; and when serial works, `TOS64-LINK/1` reports the Broadcom
identity at address 1 with a live link state. This clause closes `LE-68`.

### 6. What this test explicitly does **not** establish

- No claim that 10 ms of settle is the PHY's requirement — it is a named
  margin, revisited on board evidence.
- No LED, EEE, wake-on-LAN or power-down configuration — reset release only.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red
first; then the Tier 1 board run.

## Implementation location

- `os/src/hal-arm64/src/rp1_gpio.rs` — bank-1 constants, the release
  sequence over the `Mmio` seam.
- `os/src/hal-arm64/src/ethernet.rs` — pipeline placement and the counter
  wait.

## Reports

To be filed with the board run.
