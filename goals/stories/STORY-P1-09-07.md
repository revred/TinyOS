# STORY-P1-09-07 — The Confession: Discovery's First Refusal, Counted Out on the Lamp

Status: **In progress — every criterion Green 2026-08-03: the count was taken through the case seam the same evening it was written, and it read 3 — `pcie-link-down`, the data-link-active gate, is the Ethernet chain's first refused rung on this board. The same observation exposed the lamp's true polarity (`STORY-P1-07-08`, amended). Not Verified pending the assurance pass.**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: the 2026-08-03 evening board session — the first boot with `STORY-P1-07-08`'s lamp: execution proven by a pulsing LED while the NIC stayed flat, leaving one question ("which gate?") that no working channel can answer

## Description

Tonight's boot split the world: the lamp pulses (our code runs, the park loop
ticks) and the wire stays flat (the Ethernet chain refuses somewhere between
the PCIe status gates and the PHY scan). `discover` already knows the answer
— every arm of [`Discovery`] names the first rung that failed — but the
report that carries it rides the dead serial line, and the screen is still
dark. The one channel proven on silicon is the LED.

So the LED learns to count. When discovery ends anywhere short of a known
PHY, the park loop replaces the plain 1 Hz pulse with a repeating pattern:
**N short blinks, then a long dark gap**, where N names the first refused
rung. A human counts blinks through a box seam in a dark room; the next
session fixes the rung by name instead of by conjecture. The pattern is a
pure function of tick index and code — no waits of its own, no new failure
modes, riding exactly the 10 Hz tick the loop already has.

A healthy discovery keeps the plain pulse: the confession exists only for
refusals, so the lamp's ordinary language ("alive") is never diluted. Like
the lamp itself this is an instrument, never evidence — the serial line
remains the protocol of record whenever it works.

## Depends on

- `STORY-P1-07-08` — the lamp and its park-loop tick.
- `STORY-P1-09-01`/`-02`/`-04` — the discovery outcomes being named.

## Acceptance criteria

1. **The mapping is total and pinned.** Every reachable discovery outcome
   short of a known PHY maps to a distinct nonzero code, healthy outcomes
   (known PHY, with or without a trained link) map to none, and the mapping
   is pinned case-by-case so a new `Discovery` arm cannot silently share a
   code.
2. **The pattern is pure and pinned tick-by-tick.** For a given code, the
   on/off value at every 100 ms tick is a pure function; the tests pin a full
   period (N blinks, inter-blink gaps, the long trailing gap) and its
   periodicity, and prove the pattern engine adds no wait and touches nothing
   but the lamp.
3. **The park loop speaks it only on refusal.** With a refused discovery the
   loop drives the pattern; with a known PHY it keeps the plain 1 Hz pulse —
   including while the link watch is still waiting — asserted at the
   composition level.
4. **Board: the blink count names the rung.** The next boxed boot yields a
   counted N, recorded in the session log as the chain's first on-silicon
   self-diagnosis; the following story fixes that rung by name.

## Named debt this Story leaves open

- The code is the *first* refusal only — one number per boot, by design; the
  full register readbacks stay the SD evidence recorder's future scope if
  a rung's fix needs more than its name.

## Progress, 2026-08-03

| Criterion | State |
|---|---|
| 1 — total, pinned mapping | **Green.** Every arm matched exhaustively; distinctness pinned by a collision test. |
| 2 — pure pattern, pinned ticks | **Green.** Full-period tick table pinned for two codes; periodicity proven. |
| 3 — refusal-only composition | **Green.** Composition test: refused discovery → pattern ticks; known PHY (watching or beaconing) → plain pulse ticks. |
| 4 — board | **Green.** Count 3 observed and confirmed over multiple periods, 2026-08-03 evening: `LinkAbsent::LinkDown` — RC mode and PCIe PHY up, `DL_ACTIVE` clear. The chain's first on-silicon self-diagnosis; `STORY-P1-09-08` fixes the rung by name. |

## Tests

[`TEST-P1-09-07-A`](../tests/TEST-P1-09-07-A.md) — written before
implementation, per the TDD mandate.
