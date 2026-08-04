# STORY-P1-09-14 — The Park Loop Speaks: Three Silences Named on the Night the Wire First Trained

Status: **In progress — every criterion Green on silicon 2026-08-04, criterion 4 answered first boot: the spoken verdict read `STATE=STOPPED REASON=TIMEOUT` — one word cleared the watch and the MDIO path (a `Stopped` is only reachable through a resolved watch) and convicted the transmit, whose DMA inbound path is the next story. Not Verified.**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: the 2026-08-04 first-link boot — `RP1=PRESENT`, the laptop's linkwatch logged
the wire training to gigabit at 01:27:03, and the beat line said `STATE=PARKED` and nothing else

## Description

The first boot behind the claimed window ran the whole chain: identity
`0x0109`, PHY `0x600D/0x84A2`, and a wire that physically trained — the
laptop logged it. And the beat line said `STATE=PARKED`, forever, which
is not a diagnosis: reading the park loop shows **three distinct states
that all print the same word.** A wedged management port kills the watch
permanently and silently. A watch that polls forever without resolving
says nothing about why. Worst, a watch that *resolves* and starts the
beacon whose very first transmit refuses gets `beaconing = false`
fail-safe — and the refusal, with its error and status word, is recorded
nowhere. The discovery ladder confesses every rung; the park loop — the
place the board now actually lives — was mute.

So the beat line learns the park verdict, on the channels that already
exist (the serial line and the canvas status line — the screen is still
bootstrap, and the link this would move diagnosis onto is the very thing
being diagnosed): `state=beaconing` as today; `state=parked` now carries
its watch — `watch=alive` (polling), `watch=dead` (the wedge, terminal),
`watch=none` (nothing to watch); and a transmit refusal becomes
`state=stopped reason=timeout` or `reason=mac detail=0x…` — permanent,
spoken every period, never silently re-labelled "parked". The wedge and
the resolve — both of which take the watch to `None` — are finally
distinguished at the call site, and a settled re-probe resets the
verdict fresh.

## Depends on

- `STORY-P1-09-05` — the heartbeat line this extends
  (`TEST-P1-09-05-A` is amended: the pinned `state=parked` byte-shape
  gains its watch field).
- `STORY-P1-09-06` — the watch whose death was silent.
- `STORY-P1-09-03` — the transmit whose refusal was silent.

## Acceptance criteria

1. **Every park state prints a distinct line.** `beaconing`,
   `parked watch=alive`, `parked watch=dead`, `parked watch=none`,
   `stopped reason=timeout`, `stopped reason=mac detail=0x…` — exact
   bytes pinned, every field driven, no two states sharing a line.
2. **The wedge is distinguished from the resolve.** Both take the watch
   to `None`; the loop records which happened, and a wedge reads
   `watch=dead` on every subsequent beat — terminal, like the watch
   itself.
3. **A stopped beacon stays spoken.** A refused transmit carries its
   `TxError` into every subsequent beat line; it is never re-labelled
   `parked`; a settled re-probe pass resets the verdict along with every
   other channel.
4. **Board: the silence names its arm.** The next boxed boot's beat line
   reads one of the distinct states, the session log records it, and the
   next fix is chosen on that word rather than on three overlapping
   conjectures.

## Named debt this Story leaves open

- The park verdict is spoken, not confessed: no lamp code — the lamp
  stays the discovery ladder's channel, and the beat line is the park
  loop's. If a future boot needs the park verdict with no monitor and no
  serial, that promotion is its own story.

## Progress, 2026-08-04

| Criterion | State |
|---|---|
| 1 — distinct lines | **Green** (host): all six shapes pinned byte-exact. |
| 2 — wedge vs resolve | **Green** (host): the call-site distinction pinned; dead is terminal. |
| 3 — stopped stays spoken | **Green** (host): the error persists across beats; reset only by a settled re-probe. |
| 4 — board | **Green (2026-08-04 ~01:41).** The beat read `STATE=STOPPED REASON=TIMEOUT` while the laptop logged the wire training (01:40:58, second boot running) — the watch resolved, the beacon started, and the first transmit hung. One word replaced three conjectures: the watch and MDIO paths are cleared, the transmit's DMA inbound path is convicted, and the next story is chosen on it. |

## Tests

[`TEST-P1-09-14-A`](../tests/TEST-P1-09-14-A.md) — written before
implementation, per the TDD mandate.
