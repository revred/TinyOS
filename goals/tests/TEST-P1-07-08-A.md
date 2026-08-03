# TEST-P1-07-08-A — A Lit Lamp Is the Cheapest True Sentence a Board Can Say

Status: **In progress — host clauses Green 2026-08-03; clause 4 awaits the next power-on**
Story: [`STORY-P1-07-08`](../stories/STORY-P1-07-08.md)
Tier: Host unit tests (transcription pinning, RMW discipline, placement) **plus** a Tier 1 board power-on (the lamp itself)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by an LED. This Test raises no timing,
measurement or qualification claim; the lamp is an instrument, never
evidence, and no capture may cite it.

## What this test is for

07A's charge 1: no TinyOS build has ever produced an observable effect on
this board, and every channel that could have said otherwise routes through a
peripheral that might itself be the fault. The ACT LED is the one device on
the SoC side of every suspect — this test pins the two register writes that
light it, so that when the board finally speaks, the sentence is trusted.

## Specification

### 1. The transcriptions are pinned with their sources (`BND-17`)

**Given** the status-LED constants,
**then** the `gpio-brcmstb` block base `0x10_7D51_7C00`, the bank-0
`DATA`/`IODIR` offsets, pin 9, and the active-low polarity are asserted
against the on-silicon capture (`pios-ground-truth-2026-08-03.txt`: the
`rpi-gpiomem` window line and the `/sys/kernel/debug/gpio` line naming pin 9
`2712_STAT_LED`/`ACT` active-low) the same way `board.rs` pins the UART and
PCIe transcriptions — and the base is asserted not to fit in 32 bits, because
that truncation is silent and lands in RAM.

### 2. Drive and toggle are exact RMWs (`SEC-19`, `PD-14`)

**Given** the drive, toggle and direction operations over a recording seam
double with hostile readbacks,
**then** direction-to-output clears exactly bit 9 of `IODIR`, on/off and
toggle change exactly bit 9 of `DATA`, `on` drives the bit **high** — the
debug listing said active-low, the 2026-08-03 confession boot's bright
gap measured active-high at this pin, and the measurement governs (amended
that evening; the polarity is pinned as arithmetic with the observation as
its source) — and every other bit of both registers is preserved as found.
Nothing else is read or written.

### 3. Placement adds no wait and no authority (`SEC-20`, `PD-07`, `BND-02`)

**Given** the entry stub and the park loop,
**then** the entry-time force-on happens before the UART is configured and
consumes no inherited state (the write is unconditional, state-agnostic);
the park-loop toggle rides the existing once-per-second block beside the
heartbeat and introduces no wait of its own — a stuck counter freezes the
lamp exactly as it stops every other periodic channel.

### 4. Board: the lamp lights

**Given** this image on the proven board,
**then** the ACT LED is held on from power-on through boot and blinks at
~1 Hz once parked. This is the first observable effect TinyOS produces on
this board; it discharges 07A's charge 1 and criterion 4.

### 5. What this test explicitly does **not** establish

- Nothing about the UART, the splash, the PCIe chain or the beacon — the
  lamp deliberately depends on none of them, which is its entire value.
- No timing claim: 1 Hz is a human-eye cadence, not a measured period.
- No claim about the LED's inherited state at firmware handoff — the design
  is force-then-toggle precisely so none is needed.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red
first; then the Tier 1 power-on observation.

## Implementation location

- `os/src/hal-arm64/src/stat_led.rs` — constants, drive/toggle over the
  `Mmio` seam, pinning and RMW tests.
- `os/src/hal-arm64/src/boot.rs` — the entry-time force-on.
- `os/src/hal-arm64/src/ethernet.rs` — the park-loop toggle.

## Reports

To be filed with the board power-on.
