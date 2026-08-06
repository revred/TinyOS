# STORY-P1-09-13 — The Address Nobody Wrote: the Endpoint's BARs Are Sized, Assigned, and Believed

Status: **Verified (functional) 2026-08-05 — every criterion Green on silicon 2026-08-04: the boxed boot answered criterion 5 in its success arm — the canvas read `RP1=PRESENT ID=0x0109 PHY=0x600D84A2` where a week of `0xDEAD` poison had been, and the laptop's linkwatch logged the wire training to 1000 Mbps at 01:27:03 under TinyOS. This is the Story that cleared the window-wide poison and unblocked the rest of `FEAT-P1-09`: everything downstream — the PHY scan, the link watch, the beacon, and eventually the byte-identical capture that closed the Feature's exit criterion — became reachable on this boot. Advanced under [`06A`](../../session/hand-2026-08-05/06A-nothing-is-verified-and-the-reason-is-not-velocity.md) §4.1. **Assurance state remains `specified` and this Story is NOT release-assured**: 0 qualified platforms.**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: [`session/hand-2026-08-03/09A-window-poisoned-inbound-path-indicted.md`](../../session/hand-2026-08-03/09A-window-poisoned-inbound-path-indicted.md) — the same-night conviction capture

## Description

The board spelled code 16, detail 57005: the clocks block reads the same
fabric poison as the GEM, so the poison covers the whole window and the
per-peripheral clock theory died on silicon. The conviction capture ran
the same night, and it names the fault exactly: every register in the
working inbound chain — bus numbers, bridge decode, both command words,
all five outbound window registers — is byte-identical between the
working system and what `pcie::establish` already writes, **except the
endpoint's three BARs, which TinyOS never programs.** The recorded
assumption is in `pcie.rs`'s own `WindowPci` doc comment — "where the
firmware assigns RP1's peripheral BAR" — and the Pi OS dmesg refutes it:
Linux found BAR1 unassigned at probe and assigned it itself. Enumeration
proved who answers; it never told the device *where to listen*. Every
memory TLP goes to bus `0x0 + offset`, nothing claims it, and the read
returns `0xDEADDEAD`.

So the introduction finishes its own sentence: strictly after the
endpoint vendor gate and strictly before the memory-enable, each BAR is
**sized** by the architectural all-ones probe (the masks are pinned from
silicon: dmesg's own probe read `0xffc00000` for BAR1's 4 MiB), then
**assigned** the capture's bus addresses — `BAR0 = 0x0041_0000`,
`BAR1 = 0x0000_0000`, `BAR2 = 0x0040_0000` — and each step is believed
only from its readback. BAR1's happy readback is zero, so its belief
rests on the size mask *and* the readback together: a BAR that answered
the probe exists, and a readback of the assigned value seals it. A BAR
already holding its pinned address is left untouched — the sizing probe
is destructive to a live window, and the re-probe loop must never blink
it.

## Depends on

- `STORY-P1-09-10` — the vendor gates and the `EXT_CFG_INDEX` config
  path this rung reuses; the bridge decode it completes.
- `STORY-P1-09-12` — the clock rung immediately downstream, whose
  pre-flight becomes the first beneficiary of a claimed window.

## Acceptance criteria

1. **Sizing before assignment, masks pinned.** Each BAR not already
   holding its address is probed all-ones exactly once; the readback must
   equal the silicon-pinned mask (`0xFFFF_C000` / `0xFFC0_0000` /
   `0xFFFF_0000`, flag bits masked); a zero, all-ones, or otherwise wrong
   mask refuses with its own code and the readback — no assignment is
   attempted against a BAR that did not answer the probe.
2. **Assignment believed from readback.** Each sized BAR is written its
   pinned bus address exactly once; a readback disagreeing with the
   assignment (flag bits masked) is a distinct refusal carrying the
   readback. BAR1's zero readback is believed only after clause 1's mask.
3. **Order and idempotence.** The BAR work runs strictly after the
   endpoint vendor gate and strictly before the `EP_COMMAND`
   memory-enable write; a pass that finds every BAR already holding its
   pinned address performs zero BAR writes.
4. **The confession speaks the new rungs.** Codes 19 (BAR mask refused)
   and 20 (assignment not held) with the readback's decisive high half;
   `TOS64-LINK/1` names them (`bar-silent`, `bar-held`); the exhaustive
   match forces the wiring; every previously pinned line is
   byte-identical.
5. **Board: the window is claimed.** The next boxed boot walks past
   `CLK-SILENT`: the clock rung's pre-flight reads a credible one-hot
   `CLK_SYS_SEL`, the enables land, and the identity rung answers
   `0x0007` — or the confession names this rung's actual readback and the
   ladder continues on that number.

## Named debt this Story leaves open

- Bring-up size only: three BARs, fixed addresses from one capture. A
  real resource allocator is `EPIC-P3`'s root-complex driver.
- The `0xDEADDEAD` origin (which component poisons an unclaimed read on
  this root complex) is recorded as a curiosity in 09A, not owed.

## Progress, 2026-08-04

| Criterion | State |
|---|---|
| 1 — sizing, masks pinned | **Green** (host): probe-first order pinned; wrong/floating masks refuse as code 19 with the readback. |
| 2 — assignment by readback | **Green** (host): held readback required; a dropped write refuses as code 20. |
| 3 — order + idempotence | **Green** (host): write-list pinned — vendor gate, three size/assign pairs, then memory-enable; already-assigned pass writes nothing to the BARs. |
| 4 — confession wiring | **Green** (host): codes 19/20 distinct and exhaustive; `bar-silent`/`bar-held` lines pinned. |
| 5 — board | **Green, success arm (2026-08-04 ~01:27).** The window is claimed: identity answered module `0x0007` rev `0x0109`, the PHY scan identified `0x600D/0x84A2`, and the wire autonegotiated to gigabit (laptop linkwatch, 01:27:03) — the first Ethernet link TinyOS has ever trained. The report line's boot-time `LINK=DOWN` is the expected snapshot; the park-loop watch owns the transition, and its on-silicon beacon flip is the next observation. |

## Tests

[`TEST-P1-09-13-A`](../tests/TEST-P1-09-13-A.md) — written before
implementation, per the TDD mandate.
