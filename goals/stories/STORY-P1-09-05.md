# STORY-P1-09-05 — The Heartbeat: a Parked Board Is a Talking, Moving Board

Status: **In progress — host half Green 2026-08-03 (heartbeat line pinned, fail-safe stop proven, bounce pure and in-bounds, verdict color pinned); criterion 4 awaits the board. Not Verified.**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: [`session/hand-2026-08-03/05A-ethernet-discovery-signal.md`](../../session/hand-2026-08-03/05A-ethernet-discovery-signal.md) — the owner's constraint ("solve with the constraints we have") after two silent board attempts

## Description

Every diagnostic channel this board has is dark, and the one instrument left —
a passive listener on the host's serial port — can only catch a boot transcript
if the wiring, the clock transcription, and the operator's timing are all right
at once. This Story removes the timing and the single-shot nature from that
conjunction: after the verdict, the splash, and the one `TOS64-LINK/1` line,
the park loop emits a **`TOS64-BEAT/1` heartbeat every period, forever** —
`seq=` a counter, `state=` what the park is doing (`beaconing` or `parked`).

What that buys under the constraints: the host can listen at leisure and sweep
candidate baud rates until bytes appear. A UART-clock transcription that is
wrong by any rational factor stops being *silence* and becomes *bytes at the
wrong baud* — distinguishable, decodable, fixable. And because the heartbeat
follows the `TOS64-LINK/1` line, the first successful serial contact of any
kind also delivers the Ethernet discovery verdict that four captures have
failed to carry.

**The screen heartbeats too** — an owner finding this Story exists to answer
("what stopped you from making a ball bounce?"): the splash surface, if the
firmware granted one, stops being a single static frame. Every park tick
repaints a small bouncing block over the splash, its color carrying the
discovery verdict (beaconing / parked-with-link-story). A monitor connected
*from power-on* then shows a **moving** picture whenever the board is alive —
kernel liveness and discovery state readable with no serial and no Ethernet —
and a screen that stays dark under those conditions finally becomes evidence
*against the mailbox framebuffer path itself* (`STORY-P1-07-07`'s open board
criterion) instead of evidence of nothing. The hot-plug limit is physics, not
policy: the firmware negotiates the framebuffer once at power-on, so a
monitor plugged in later shows nothing regardless.

The protocol discipline holds: the pinned boot lines are untouched, the LINK
line stays exactly one (`TEST-P1-09-01-A` clause 5), and the heartbeat is a
new envelope appended after everything it must never perturb. A UART write
that fails stops the heartbeat (fail-safe, like every other refusal on this
Feature) — the park itself is never disturbed.

## Depends on

- `STORY-P1-09-01`..`-04` — the heartbeat reports their outcome; it rides the
  same park loop as the beacon.

## Acceptance criteria

1. **The heartbeat line is exact bytes.** `TOS64-BEAT/1 seq=<dec>
   state=<beaconing|parked> fb=<granted|refused>\n`, built pure, pinned by
   host tests, sequence the only variance — `fb=` reporting whether the
   firmware granted the splash's framebuffer exchange (06A's Question-1
   discriminator).
2. **Placement and restraint.** Exactly one `TOS64-LINK/1` line still; the
   heartbeat begins only after it; the pinned protocol lines are byte-
   identical with the heartbeat code present; a failed UART write ends the
   heartbeat permanently while the park (and beacon, if running) continue.
3. **The bounce is pure, bounded, and in-bounds.** The animation state steps
   by a pure function (position, velocity, wall reflection) pinned by host
   tests; each tick erases the block's previous rectangle and paints the new
   one through the existing `Surface` seam with every write in-bounds on a
   mock surface; the block's color is a pure function of the discovery
   verdict. No full-screen repaint, no unbounded work per tick.
4. **Board: something reaches a human without being asked for twice.** Either
   the host listener receives repeating heartbeats (at the expected baud or a
   swept one), or a monitor connected from power-on shows the block moving —
   and a dark screen under those exact conditions is recorded as evidence
   against the mailbox path. This criterion is what turns four timed
   silences into an untimed experiment.

## Named debt this Story leaves open

- If the sweep still hears nothing at any rational baud with the board
  provably powered, the remaining branches are physical (adapter, cable,
  connector mux) — the heartbeat cannot rule those in software, only make
  their diagnosis untimed.

## Progress, 2026-08-03

| Criterion | State |
|---|---|
| 1 — heartbeat exact bytes | **Green.** `TOS64-BEAT/1 seq=<dec> state=<beaconing\|parked>` pinned, sequence the only variance. |
| 2 — placement and restraint | **Green (host).** One LINK line still (its pinned test unchanged); heartbeat emitted only in the park loop after it; a refused UART write stops heartbeating permanently (wedged-FIFO double). |
| 3 — bounce pure, bounded, in-bounds | **Green.** Wall reflection driven on all four walls; a too-small surface parks the block; per-tick work is two small rectangles through the `Surface` seam with zero out-of-bounds writes; verdict colors pinned. |
| 4 — board: something reaches a human | **Blocked on the next power-on.** The untimed experiment this Story exists for: heartbeats on a swept COM5, or motion on a from-power-on monitor — or a dark screen that finally indicts the mailbox path. |

## Tests

[`TEST-P1-09-05-A`](../tests/TEST-P1-09-05-A.md) — written before
implementation, per the TDD mandate.
