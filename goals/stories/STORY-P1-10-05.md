# STORY-P1-10-05 — The Machine Says How Hot It Is

Status: **In progress — criteria 1 to 6 met; **criterion 7's missing thing is the paired Pi OS reading: the raw word is on the wire and it moves, but nothing has calibrated it.** Implemented 2026-08-05 after `LE-75`; host-Green. **First board evidence 2026-08-05** (`BOARD VERDICT 14`, netbooted image `f8133b0958d3`): `Thermal Kernel Observe Ok rung=ThermalSample` records are on the wire **once per park beat**, carrying raw AVS words `0x000106D4`–`0x000106DD` — the data field moving across 724..733 within a single 25-second window, which is criterion 7's first half exactly ("a capture shows `Thermal Kernel Observe` records whose raw word moves"). Criterion 2 is discharged with it: the board sent the 32-bit word **unmasked and unscaled** and every conversion happened on the host, where the decoder labels its own output `(unverified)`. Criteria 4 and 5 rode along — `Category::Thermal`/`Action::Observe` decoded against the kernel's closed vocabulary with 0 refused across the whole capture, and the sample costs one load on the park path with nothing that can block. **Criterion 7's second half is not met and no temperature may be quoted until it is.** The criterion is explicit that "the calibration is confirmed against a paired Pi OS reading **before any temperature is quoted as fact**", and no such pairing has been taken: the `bcm2711_thermal` formula the host applies (`-487 × data + 410040` millicelsius) is carried in Ti64Dink as an unverified hypothesis and prints ~53–57 °C, a range that is *plausible for a Pi 5 under load on an open bench* — which is precisely what makes it dangerous, because a plausible wrong number is the one that gets quoted. Criterion 3 anticipated this and is why the raw word travels: the reading is falsifiable *because* nothing was converted on the board. Closing it needs one paired capture — TinyOS's raw word and Pi OS's `/sys/class/thermal` value read minutes apart on the same board at the same thermal state. **Not Verified.**
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
