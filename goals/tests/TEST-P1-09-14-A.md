# TEST-P1-09-14-A — One Word Per State, and No State Left Mute

Status: **In progress — every clause Green 2026-08-04; clause 4 answered on silicon (`STATE=STOPPED REASON=TIMEOUT` with the wire trained — the watch cleared, the transmit convicted)**
Story: [`STORY-P1-09-14`](../stories/STORY-P1-09-14.md)
Tier: Host unit tests (line shapes, wedge/resolve distinction, stopped persistence)
**plus** a Tier 1 boxed-boot transcription
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by a status line. No timing claim;
the beat cadence is unchanged and unmeasured.

## What this test is for

The first trained wire exposed three park-loop states that all printed
`STATE=PARKED`: a dead watch, a live-but-unresolved watch, and a
beacon whose first transmit refused and was silently re-labelled parked.
This test pins the repair: every state a distinct line, the wedge
distinguished from the resolve at the one call site that can tell them
apart, and a stopped beacon that stays stopped and stays spoken.

## Specification

### 1. Every park state prints a distinct pinned line (`SEC-20`)

**Given** each park verdict,
**then** the beat line is exact bytes:
`state=beaconing`; `state=parked watch=alive`;
`state=parked watch=dead`; `state=parked watch=none`;
`state=stopped reason=timeout`;
`state=stopped reason=mac detail=0x` + eight hex digits — every field
driven, no two verdicts sharing a line, `seq` and `fb` unchanged in
shape.

### 2. The wedge is distinguished from the resolve (`PD-10`, `BND-06`)

**Given** a watch step that takes the watch to `None`,
**then** a resolve (the step returned a speed) yields `beaconing`, and a
wedge (the step returned nothing) yields `watch=dead` on that beat and
every subsequent one — terminal, matching the watch's own contract.

### 3. A stopped beacon stays spoken (`SEC-19`, `PD-12`)

**Given** a transmit refusal after the watch resolved,
**then** every subsequent beat carries `state=stopped` with the same
`TxError` (timeout, or mac with its status word) — never re-labelled
`parked` — and a settled re-probe pass resets the verdict along with the
other channels.

### 4. Board: the silence names its arm

**Given** the next boxed boot with the wire connected,
**then** the beat line reads one of the distinct states; the session log
records the word and (for `stopped reason=mac`) the detail; the next fix
is chosen on it.

### 5. What this test explicitly does **not** establish

- No lamp code for park verdicts — the lamp remains the discovery
  ladder's channel.
- No change to `TOS64-LINK/1`, the beacon frame, or any discovery rung.
- No claim about *why* a given arm occurred — that is the next story's
  evidence, which this one exists to select.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/ethernet.rs`),
written Red first; then the Tier 1 transcription.

## Implementation location

- `os/src/hal-arm64/src/ethernet.rs` — the `ParkState` verdict, the
  extended `heartbeat_line`/`emit_heartbeat`, the park-loop wiring
  (wedge/resolve distinction, stopped persistence, re-probe reset).

## Reports

To be filed with the boxed boot.
