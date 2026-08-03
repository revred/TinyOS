# STORY-P1-09-12 — The Current: the Ethernet Clocks Are Switched On Before the Identity Is Asked

Status: **In progress — host half Green 2026-08-03 (pre-flight gate, enable-by-readback, bounded run poll, pipeline splice, codes 16–18 all pinned); criterion 5 awaits the board. Not Verified.**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: the 2026-08-03 first-light session — the canvas held at `ID-MODULE 0xDEAD` and the
Pi OS capture proved the same register reads `0x00070109` once two clock-enable bits are set

## Description

The monitor spelled the identity rung's refusal: module `0xDEAD`, RP1's
fabric poison for a block whose clock is not running. The same evening,
the same silicon, the same bus path under Pi OS read `GEM_MID =
0x00070109` — so the bus is proven, the window is proven, and the delta
is not a mystery: it is two registers in RP1's clocks block
(`goals/reports/pios-ground-truth-2026-08-03.txt`, live capture). The
GEM's register-bus clock (`clk_sys`) is critical-always-on; the two
gateable consumers are `clk_eth` (tx, 125 MHz from `pll_sys_sec`) and
`clk_eth_tsu` (50 MHz from `xosc`), and under a working Linux both read
`CTRL = 0x10000800`: `ENABLE` (bit 11) held, running-status (bit 28)
answering, `AUXSRC = 0`, `DIV_INT = 1`. The PLL tree beneath them is the
firmware's work and already locked before any kernel runs.

So the pipeline gains one rung, strictly between enumeration and the
identity read: **switch the current on, then ask who is there.** The
clocks block is itself read before it is believed — a poisoned or
floating pre-flight readback is a refusal, not a surprise — then each of
the two clocks is enabled by a pinned write, the enable is believed only
from its readback, and the running status is polled under a bounded
budget. Every refusal is a new arm of the confession with its own code
and its named sixteen decisive bits; fabric poison spells itself as
`57005` — the decimal name of `0xDEAD` — on the lamp and the canvas.

No PLL is programmed, no rate is chosen, no divider is tuned: the values
written are the architectural defaults transcribed from the driver
source and confirmed live, and everything else is readback-validated
exactly as the window was (`STORY-P1-09-09`'s posture: a written
register is a hope, only its readback is a fact).

## Depends on

- `STORY-P1-09-10` — enumeration must have passed; the clocks block is
  behind the same programmed window.
- `STORY-P1-09-11` — the sentence carries the new codes unchanged.

## Acceptance criteria

1. **The block is read before it is believed.** A pre-flight read of
   `CLK_SYS_SEL` that is all-zeros, all-ones, or fabric poison refuses
   with its own code and the readback's decisive half — the clocks block
   is never written blind.
2. **Enable is believed only from readback.** Each gateable clock
   (`clk_eth`, `clk_eth_tsu`) is enabled by a pinned write; a readback
   without the enable bit is a distinct refusal carrying the readback.
   The write preserves the architectural source and divider fields and is
   performed exactly once per clock per pass.
3. **Running is polled, bounded, and honest.** The running status is
   polled under a fixed attempt budget with no time constant anywhere; a
   clock that never runs refuses with the status half of its last
   readback. A clock already enabled and running is left untouched —
   the rung is idempotent across re-probe passes.
4. **The rung sits in the pipeline and the confession speaks it.** The
   rung runs strictly after enumeration and strictly before the identity
   read; each new refusal earns a distinct blink code (16, 17, 18) and
   its `TOS64-LINK/1` name, health is untouched, and every existing
   pinned line is byte-identical.
5. **Board: the identity answers.** The next boxed boot moves the canvas
   report past `ID-MODULE`: the identity rung reads `0x0007` where the
   poison was, or the confession names the clock rung's actual readback
   and the next fix is chosen on that number.

## Named debt this Story leaves open

- The rung enables exactly the two clocks the GEM consumes and nothing
  else; a general RP1 clock service (rates, parents, other consumers) is
  `EPIC-P3` territory with the device-service work.
- `pll_sys` lock is verified as a gate (`LOCK` bit read) but never
  programmed; a boot where the firmware left the PLL unlocked parks with
  the pre-flight refusal and stays `LE-26`-adjacent driver territory.

## Tests

[`TEST-P1-09-12-A`](../tests/TEST-P1-09-12-A.md) — written before
implementation, per the TDD mandate.
