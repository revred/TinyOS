# TEST-P1-09-05-A — Four Timed Silences Become One Untimed Experiment

Status: **In progress — host clauses Green 2026-08-03; clause 5 awaits the board listen/sweep session**
Story: [`STORY-P1-09-05`](../stories/STORY-P1-09-05.md)
Tier: Host unit tests (heartbeat bytes, placement, fail-safe stop) **plus** the board listen/sweep session the Story exists for
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by a heartbeat. This Test raises no
timing, measurement or qualification claim.

## What this test is for

A board that speaks once can only be heard by luck; a board that speaks every
second can be heard by patience. The heartbeat converts the serial diagnosis
from a timed conjunction (wiring ∧ clock ∧ operator timing) into an untimed
one (wiring ∧ clock), and makes the clock term *observable* — a wrong UART
clock produces bytes at a findable baud instead of silence.

## Specification

### 1. The heartbeat is exact bytes (`BND-03`)

**Given** the heartbeat builder,
**then** `TOS64-BEAT/1 seq=<dec> state=<beaconing|parked>
fb=<granted|refused>\n` is a pure function pinned word-for-word, sequence
the only variance, every field's words driven. The `fb=` field is 06A's
Question-1 discriminator: it reports whether the firmware granted the
splash's framebuffer exchange, splitting a refusing mailbox path from wrong
plug conditions with no monitor involved.

### 2. Placement after everything it must never perturb (`BND-17`)

**Given** the park path,
**then** the pinned protocol lines and the single `TOS64-LINK/1` line are
byte-identical with the heartbeat present, and the first heartbeat follows
the LINK line — asserted on the host over the wire double.

### 3. Fail-safe stop (`SEC-20`, `RCG-13`)

**Given** a UART write refusal mid-heartbeat,
**then** heartbeating stops permanently, the park (and the beacon, if it was
running) continue unchanged, and nothing retries — proven over a wire double
scripted to refuse.

### 4. The visual heartbeat is pure over the Surface seam (`SEC-19`, `PD-07`)

**Given** the bounce state and renderer,
**then** the step function's reflection behavior is pinned (corners
included), every erase/paint write lands in-bounds on a mock surface for
arbitrary bounded surface sizes, the per-tick work is two small rectangles
(never a full-screen repaint), and the block color is a pinned pure function
of the discovery verdict.

### 5. Board: something reaches a human without being asked for twice

**Given** a powered, parked board,
**then** a host listener on the adapter receives repeating heartbeats with
monotonic `seq=` — at 115200 or a swept baud, in which case the found baud is
itself the finding — **or** a monitor connected from power-on shows the block
moving with the verdict color; a dark screen under those conditions is
recorded as evidence against the mailbox framebuffer path
(`STORY-P1-07-07`'s open criterion), which no observation so far has been
able to say.

### 6. What this test explicitly does **not** establish

- Nothing about the adapter, cable, or connector mux — a sweep that hears
  nothing leaves the physical branches exactly where they were.
- No hot-plug claim: the firmware negotiates the framebuffer once at
  power-on; a later-connected monitor shows nothing by design.
- No timing claim; the "period" is a rough second, not a measured one.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red
first; then the board listen/sweep session.

## Implementation location

- `os/src/hal-arm64/src/ethernet.rs` — heartbeat builder and the park-loop
  emission.
- `os/src/xtask` unchanged — the sweep listener is host-side tooling run
  ad hoc; if it becomes a Story of its own, it gets its own contract.

## Reports

To be filed with the listen/sweep session.
