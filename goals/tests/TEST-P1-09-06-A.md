# TEST-P1-09-06-A — The Wire Decides When, the Board Only Decides Whether

Status: **In progress — host clauses Green 2026-08-03; clause 4 awaits the board**
Story: [`STORY-P1-09-06`](../stories/STORY-P1-09-06.md)
Tier: Host unit tests (late link-up, wedge fail-safe, never-up honesty) **plus** a Tier 1 board run (NIC link watch and beacon capture)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-02`, `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by a link watch. This Test raises no
timing, measurement or qualification claim; in particular it deliberately
pins the *absence* of any negotiation-time constant.

## What this test is for

The ground-truth session showed this bench's PHY takes ~4 s to negotiate
while `discover` reads the link once, ~15 ms after reset release — an honest
report of a wire that was seconds from training, followed by a permanent
skip. The owner's constraint is binding: different hardware has different
character, so the fix must contain no number derived from this bench. These
clauses pin the shape that satisfies that — a once-per-second watch inside
the park loop that lets the wire pick the moment.

## Specification

### 1. A late link-up starts the beacon, for any N (`BND-06`, `PD-10`)

**Given** the park loop over a scripted management port whose link reads
answer down for the first N one-second polls and up-with-rate on poll N+1,
**then** the first beacon frame is staged and transmitted on that same tick
and the heartbeat line flips to `state=beaconing` — proven for more than one
N, with the decision containing no timing constant of its own.

### 2. The watch is bounded per poll and stops on a wedge (`SEC-20`, `PD-07`)

**Given** a management port that wedges (bounded-poll timeout) partway
through the watch,
**then** the watch ends permanently — no further link reads on any later
tick — while the heartbeat and the splash animation continue untouched, and
nothing is retried against the wedged port.

### 3. A link that never trains stays honestly parked (`BND-07`, `SEC-19`)

**Given** a link that answers down on every poll forever,
**then** no frame is ever staged, no transmit is ever attempted, and every
heartbeat reports `state=parked` — the watch can wait indefinitely but can
never invent a beacon.

### 4. Board: the beacon arrives on the wire's schedule

**Given** this image, the `STORY-P1-09-04` release, the proven cable and the
laptop NIC watch,
**then** the NIC records link training and subsequently captures
`TOS64-PRESENT/1` frames, however long this PHY and this partner take to
negotiate. Together with the release this closes `LE-68`'s observation half.

### 5. What this test explicitly does **not** establish

- No claim about how long negotiation takes on any hardware — that is the
  point.
- No link-loss downgrade: a link that later drops is handled by the existing
  transmit-error stop, not by the watch (named debt in the Story).
- No receive path: the watch reads PHY status registers only; receive stays
  disabled.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red
first; then the Tier 1 board run.

## Implementation location

- `os/src/hal-arm64/src/ethernet.rs` — the park-loop watch, its fail-safe,
  and the beacon start.
- `os/src/hal-arm64/src/gem.rs` — the latched-twice link read the watch
  re-uses unchanged.

## Reports

To be filed with the board run.
