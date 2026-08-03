# TEST-P1-07-07-A — The Splash Serves the Screen and Never the Other Way Around

Status: **In progress — host clauses Green 2026-08-03; clauses 5 and 6 await the board and the serial adapter**
Story: [`STORY-P1-07-07`](../stories/STORY-P1-07-07.md)
Tier: Host unit tests (message layout, hostile-response validation, renderer) **plus** a Tier 1 board run (photograph + unchanged serial capture) when the adapter arrives
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by a splash. This Test raises no timing,
measurement or qualification claim; its board evidence is a photograph and a
byte-compared serial capture.

## What this test is for

The owner's finding, verbatim in effect: a successful boot that looks identical to
a dead board is the worst possible UX. The fix must not corrupt the thing the
Feature exists for — serial evidence — so this Test pins both halves: the screen
gets painted, and the protocol cannot tell the difference.

## Specification

### 1. The property message is exact bytes

**Given** the splash's two property messages,
**then** the native-size query and the framebuffer request (header,
set-physical-size at the chosen mode, set-virtual-size, set-depth 32,
allocate-buffer alignment 4096, get-pitch, end tag) are produced by pure
functions and pinned word-for-word by host tests, with 16-byte alignment
guaranteed by the types, not by luck. The chosen mode is the display's native
size when the query's hostile-validated answer is sane (bounded both ends),
1280×720 otherwise — adaptation and centring are proven at 1920×1080,
3840×2160 and 1024×600 in clause 3.

### 2. The firmware's response is hostile input (`BND-02`, `PD-12`, `RCG-01`)

**Given** a mailbox response,
**then** a framebuffer descriptor is believed only after typed validation, and
each of these is a distinct driven rejection: wrong response code, missing
allocate/pitch tag, zero base, zero or implausible size, depth not 32,
pitch inconsistent with width, dimensions beyond the sanity bound. A rejected
descriptor paints nothing and touches no memory.

### 3. The renderer is pure, bounded and centred (`SEC-19`)

**Given** the splash renderer over a mock surface,
**then** every write lands inside the surface bounds for arbitrary (bounded)
surface sizes; the background fill covers the surface; the glyph pass paints a
non-trivial pixel count; and spot-checked pixels confirm "TinyOS" is centred.

### 4. Every board-side wait is bounded (`SEC-20`, `PD-07`)

**Given** the mailbox handshake implementation,
**then** each status poll is a bounded loop with a typed timeout, verified by
inspection tests over the pure state machine where one exists and by review for
the two volatile spins — there is no unbounded wait anywhere on the splash path,
and every failure resolves to silent-continue into the same `park()`.

### 5. Board: the serial protocol is unchanged (`BND-17`)

**Given** the board and the serial adapter,
**then** the capture with splash code present is byte-identical in its protocol
lines (entry report, READY sequence, `vbar` report, `TOS64-RESULT/1` verdict) to
the pre-splash image's specification — splash success and splash failure alike.

### 6. Board: the screen says TinyOS

**Given** the boxed board, a monitor and power,
**then** "TinyOS" in block letters on a filled background is visible within
firmware-boot time, photographed beside the capture for the Report.

### 7. What this test explicitly does **not** establish

- No display driver, console, compositor or EPIC-H2 claim — one static frame.
- No timing figure of any kind; no change to any `-01`…`-06` criterion.
- Until the adapter arrives, a dark screen plus green verdict is **undebuggable
  and unclaimable** — the blind-flight caveat is part of the specification.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red first;
then the Tier 1 board run.

## Implementation location

- `os/src/hal-arm64/src/hdmi.rs` — property message, response validation, font,
  renderer, mailbox handshake, splash orchestration.
- `os/src/hal-arm64/src/boot.rs` — one call after the verdict, before `park()`.

## Reports

To be filed with the board run.
