# STORY-P1-09-06 — The Watch: a Link Is Awaited, Never Concluded From One Read

Status: **In progress — host half Green 2026-08-03 (late link-up starts the beacon, wedge stops the watch permanently, cadence and fail-safe pinned); criterion 4 awaits the board. Not Verified.**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: [`session/hand-2026-08-03/07A-instruments-proven-os-indicted.md`](../../session/hand-2026-08-03/07A-instruments-proven-os-indicted.md) — the ground-truth session that followed it: Pi OS's own dmesg shows ~4 s between PHY bring-up and link-up on this bench, and the owner's direction is that no one bench's number becomes the design

## Description

`discover` reads the link exactly once, milliseconds after the reset release.
Tonight's ground truth (`goals/reports/pios-ground-truth-2026-08-03.txt`)
shows what that misses: on this bench, under Pi OS, the same PHY takes ~4
seconds of autonegotiation between driver attach and link-up. A single early
read would have honestly reported `link=down` on a wire that was seconds from
training — and the beacon would have skipped forever on a healthy cable.

The owner's constraint shapes the fix: **different hardware has different
character**, so no constant tuned to this bench (4 s, or any other) may
appear. The robust shape is not a longer one-shot window but a watch: the
park loop already ticks forever, so while the link is down it re-reads the
PHY once per second, and the beacon starts on whatever tick the wire comes
up — after four seconds of autoneg, after forty, or when a cable is plugged
in a week after boot. "The cable is the signal" finally means plug-in *at any
time*.

The one-shot `TOS64-LINK/1` report is unchanged and stays honest — it says
what the link was when discovery looked. The watch's evidence channel is the
heartbeat that already exists: `TOS64-BEAT/1 state=` flips from `parked` to
`beaconing` on the tick the link arrives, and the splash block's color says
the same thing to the monitor. Fail-safe is unchanged in kind: each poll is
one bounded MDIO transaction, and a management-port timeout ends the watch
permanently — a wedged port is never retried against, matching every other
channel's one-refusal-stops rule.

## Depends on

- `STORY-P1-09-02` — the latched-twice link read and MDIO bounds the watch
  re-uses unchanged.
- `STORY-P1-09-04` — the reset release that makes the PHY answer at all.
- `STORY-P1-09-05` — the park loop and heartbeat the watch lives inside and
  reports through.

## Acceptance criteria

1. **A late link-up starts the beacon.** With a scripted PHY that answers
   `link=down` for the first N polls and `up` on poll N+1, the park loop
   begins transmitting beacon frames on that tick, and the heartbeat line
   flips to `state=beaconing` — for N of any size, with no timing constant
   anywhere in the decision.
2. **The watch is bounded per poll and fail-safe overall.** Each poll is the
   existing latched-twice read over the bounded management port; a scripted
   port wedge ends the watch permanently (no re-poll, no retry), while every
   other channel continues untouched.
3. **A link that never comes stays honestly parked.** With a PHY that
   answers down forever, the loop ticks on: heartbeat `state=parked`, no
   frame ever staged, no transmit attempted — pinned so the watch cannot be
   the source of a phantom beacon.
4. **Board: the beacon arrives on the wire's schedule.** With this image and
   the release from `STORY-P1-09-04`, the laptop NIC trains and then captures
   `TOS64-PRESENT/1` frames — regardless of how many seconds this PHY and
   this partner take to negotiate. Closes `LE-68`'s observation half if the
   release alone has not already done so.

## Named debt this Story leaves open

- The watch only upgrades toward beaconing; a link that later *drops* stops
  transmission through the existing transmit-error path rather than through
  link re-reads. Downgrade-on-link-loss is deliberately out of scope.

## Progress, 2026-08-03

| Criterion | State |
|---|---|
| 1 — late link-up starts the beacon | **Green.** Scripted-PHY park test: down for N polls, up on N+1, first frame staged that tick, heartbeat flips. |
| 2 — bounded and fail-safe | **Green.** Wedged-port script ends the watch permanently; heartbeat and animation continue. |
| 3 — never-up stays parked | **Green.** Forever-down script: no staging, no transmit, `state=parked` throughout. |
| 4 — board | **Blocked on the next power-on.** |

## Tests

[`TEST-P1-09-06-A`](../tests/TEST-P1-09-06-A.md) — written before
implementation, per the TDD mandate.
