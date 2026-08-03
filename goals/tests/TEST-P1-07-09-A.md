# TEST-P1-07-09-A — Paint the Canvas That Was Already There

Status: **In progress — host clauses Green 2026-08-03; clause 4 awaits the next boot**
Story: [`STORY-P1-07-09`](../stories/STORY-P1-07-09.md)
Tier: Host unit tests (conversion, bounds, font, composition) **plus** a Tier 1 boot with the monitor connected
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by a status panel. This Test raises
no timing, measurement or qualification claim; like the splash and the
lamp, the canvas is an instrument and UX, never evidence — no capture may
cite what was painted.

## What this test is for

The firmware scans out a 1920×1080 RGB565 buffer at a known address while
every diagnosis crawls through a one-bit lamp. The risk in painting it is
prosaic and real: a wrong stride shears every glyph, a wrong bound writes
3.9 MiB into someone else's memory, a missing glyph silently truncates the
one line that mattered. These clauses pin exactly those three things.

## Specification

### 1. The constants and the conversion are pinned (`BND-17`, `BND-03`)

**Given** the canvas constants and the RGB565 conversion,
**then** address `0x3F80_0000`, size `0x3F_4800`, `1920×1080`, stride
`3840` and 16 bits per pixel are asserted citing the captured dmesg line
(`format=r5g6b5, mode=1920x1080x16, linelength=3840`) and the fb0 sysfs
cross-check; and the conversion is pinned at black, white, each pure
channel, and the splash navy — arithmetic, not comment.

### 2. The surface is bounds-honest (`SEC-19`, `PD-07`)

**Given** a host surface with the pinned geometry,
**then** out-of-range puts are ignored, a row of width w occupies exactly
`stride` bytes with no bleed into its neighbour, and the full clear writes
every addressable pixel and not one byte beyond the pinned size.

### 3. The font and the console are total over the report language (`BND-02`)

**Given** every byte the report lines, heartbeat states and spelled
refusals can emit,
**then** each renders a glyph (letters case-folded, unknown bytes as a
visible block, nothing skipped or truncated), and the console composition
places the title, the `TOS64-LINK/1` text, the heartbeat state and the
refusal sentence (`CODE NN DETAIL NNNNN`) at pinned positions — pure over
the `Surface` seam, no waits, no reads of anything but its inputs.

### 4. Board: the monitor speaks

**Given** the next boot on the proven chain,
**then** the connected display shows the title and the live report as
readable text where every earlier boot was dark — the owner reads the
diagnosis directly. A wrong-geometry outcome (shear, wrap, blank) is
recorded honestly and moves the constants, not the goalposts.

### 5. What this test explicitly does **not** establish

- Nothing about the mailbox path — it stays as `TEST-P1-07-07-A` pinned
  it, untouched beside the canvas.
- No EDID, mode-setting or hotplug behaviour: one pinned geometry.
- No claim the canvas survives firmware or display changes — the constants
  are this chain's, cited, and move with it.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red
first; then the Tier 1 boot observation.

## Implementation location

- `os/src/hal-arm64/src/canvas.rs` — constants, conversion, surface,
  font, console composition.
- `os/src/hal-arm64/src/board.rs` — the pinned canvas constants.
- `os/src/hal-arm64/src/ethernet.rs` — the park-loop composition writing
  the live lines.

## Reports

To be filed with the boot.
