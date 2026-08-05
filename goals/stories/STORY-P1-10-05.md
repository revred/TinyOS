# STORY-P1-10-05 — The Machine Says How Hot It Is

Status: **In progress — implemented 2026-08-05 after `LE-75`; host-Green, no board evidence yet. The sensing half only; nothing acts on the reading.**
Feature: [`FEAT-P1-10`](../features/FEAT-P1-10.md)
Architecture: [`docs/spoor-transport-architecture.md`](../../docs/spoor-transport-architecture.md) §1, §2
Introduced in: `session/hand-2026-08-04/06A` session, from the owner's observation that the fan never spins under TinyOS

## Description

`LE-75` began as a fan that would not turn. The capture that followed found something larger:
**TinyOS drives this SoC with no thermal awareness at all.** It could not read the die
temperature, could not throttle, and did not know whether firmware was managing heat on its
behalf. That is rule 1 territory — safety before security before correctness before
performance — and it was an absence no Story owned.

It matters for a real-time OS more than for most systems: **a throttled core invalidates every
timing figure measured on it.** Thermal state is not an environmental footnote here, it is a
precondition of the evidence `EPIC-P1` exists to produce.

**The ground truth settled the hard question.** Captured from Pi OS over SSH on 2026-08-05:
the thermal zone is backed by `brcm,bcm2711-avs-monitor` at device-tree `7d542000` under a
`soc` node whose ranges map child `0x0` to parent `0x10_0000_0000` — resolving to
`0x10_7D54_2000`, exactly what Linux named the platform device (`107d542000.avs-monitor`).
So it is a **mappable register, not a VideoCore mailbox call**, and it lives in the same
Device gigabyte the identity map already covers for `GICD_BASE`. No new transport, no mapping
change.

**It also corrected the original alarm.** At 38 °C the fan is off under Pi OS too — `pwm1 = 0`,
`cooling_device0 cur=0/4`, first active trip point 50 °C, `throttled=0x0`. TinyOS idling with a
still fan is the same behaviour Linux exhibits at the same temperature. The gap is not that
TinyOS runs uncooled where Linux would cool; it is that TinyOS **cannot see** the temperature
and would not know if it ever climbed.

### Why the board does not convert

The raw-to-millicelsius slope and offset are **not verified on this hardware** — reading the
register from Pi OS needed root the ground-truth session did not have. Compiling in an
unverified constant and emitting a number that *looks* like a temperature is precisely the
shape of `LE-69`, where an assumed constant made the code refuse a conforming device.

So the raw 32-bit word travels unaltered and Ti64Dink converts, clearly labelled unverified.
Two consequences, both wanted: a wrong register offset shows up as a word that does not drift
the way a die temperature drifts, rather than as a plausible number nobody questions; and the
calibration can be corrected with a host edit instead of a card swap and a power cycle.

### Sensing before actuation

Nothing in this Story drives the fan, caps a clock, or acts on the reading in any way. An
actuator fed by a sensor nobody has validated converts a measurement error into a physical
one. Reading and acting are separate rungs, and today only the first exists.

## Acceptance criteria

1. **The register is transcribed, not recalled.** The AVS monitor address is derived from this board's own device tree and corroborated against the name Linux resolved for the platform device.
2. **The board reads and does not interpret.** The raw 32-bit word reaches the wire unmasked and unscaled; every conversion happens on the host.
3. **A wrong offset is falsifiable.** Because the raw word travels, a reader can see that the value does not behave like a temperature. The offset is held as a hypothesis the board will confirm or refute, and the code says so.
4. **The vocabulary is widened honestly.** `Category::Thermal` and `Action::Observe` are added test-first with append-only discriminants, rather than folding a die temperature into `Boot` because the boot crate stamps it.
5. **The sample costs one load.** No allocation, no branch, nothing that can block — cheap enough to stamp every park beat without arguing for itself.
6. **Nothing acts on the reading.** No fan control, no throttle, no policy. Asserted by the absence of any such path, and stated in every document until an actuation Story exists.
7. **Board evidence.** A capture shows `Thermal Kernel Observe` records whose raw word moves as the board warms, and the calibration is confirmed against a paired Pi OS reading before any temperature is quoted as fact.

## Named debt this Story leaves open

- **The calibration is unverified** — criterion 7's second half. Until a paired capture exists, no document may quote a TinyOS temperature as a measurement.
- **The register offset (`0x200`) is the one value not corroborated by the ground-truth capture.** It is the `bcm2711_thermal` driver's offset, held as a hypothesis.
- **No actuation and no throttle.** `LE-75` stays open until the board can respond to what it reads, and the operational rule remains: power for a run, power down after.
- **RP1's own sensor** (`rp1_adc`, `temp1_input`) is a second reading this Story does not take.

## Tests

[`TEST-P1-10-05-A`](../tests/TEST-P1-10-05-A.md) — written with the implementation, per the TDD mandate.
