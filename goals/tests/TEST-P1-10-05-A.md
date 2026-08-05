# TEST-P1-10-05-A — A Temperature Nobody Verified Is Not a Measurement

Status: **In progress — host-Green 2026-08-05. Clause 5's board half (the raw word moving with the die) and clause 6 (the calibration confirmed against a paired reading) have no silicon evidence yet.**
Story: [`STORY-P1-10-05`](../stories/STORY-P1-10-05.md)
Tier: Host unit tests (`hal_arm64::thermal`, `hal_arm64::board`, `kernel::spoor`, `kernel::spoor_stream`) **plus** a Tier 1 hardware run, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D07`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

## What this test is for

The temptation here is a single line of code that returns a plausible number. `LE-69` is what
that looks like when it goes wrong: a constant assumed rather than discovered, which made the
code refuse a conforming device and cost a board session to find. A temperature is worse than
a priority mask in one respect — **a wrong one still reads like a temperature**, so nothing
about the value itself announces the error.

Every clause below exists to keep the unverified parts *visibly* unverified.

## Clauses

**Clause 1 — the address is transcribed and corroborated.** `AVS_MONITOR_BASE` equals the
device tree's `7d542000` plus the `soc` node's parent base `0x10_0000_0000`, and that sum
equals the name Linux independently resolved for the platform device
(`107d542000.avs-monitor`). Two derivations from one capture, not one derivation and a
memory.

**Clause 2 — the read stays inside the page the map covers.** The offset plus four bytes is
within `AVS_MONITOR_SIZE`, and the AVS monitor shares its Device gigabyte with `GICD_BASE`.
The second half is the one that would otherwise fault only on hardware while every host test
passed — the `LE-71` shape.

**Clause 3 — the board does not interpret.** The raw 32-bit word reaches the spoor's cost
field unmasked and unscaled. Verified by the absence of any arithmetic in the read path, and
by the host decoder being the only place a celsius figure is produced.

**Clause 4 — the vocabulary widened rather than stretched.** `Category::Thermal` and
`Action::Observe` decode, their discriminants are pinned (10 and 14), and the next values up
(11 and 15) are still refused. **`Action` now has one verb of headroom**, and the test says so
where someone will read it: the addition after next is a wire-format change to every spoor
ever stored.

**Clause 5 — a host build cannot invent a temperature.** `read_raw` returns zero off the
board. A host stand-in that returned a plausible sample would let a test pass against a
machine that has no sensor at all.

**Clause 6 — Tier 1: the word moves like a die temperature.** A capture shows
`Thermal Kernel Observe` records whose raw word changes as the board warms and settles. A word
that never moves, or whose validity bits never set, refutes the offset hypothesis — and that
refutation is the point of shipping the raw value.

**Clause 7 — Tier 1: the calibration is confirmed before it is quoted.** The raw datum is
paired against a Pi OS `thermal_zone0` reading at two distinct temperatures and the slope and
offset are derived from *this* board. Until then Ti64Dink marks the figure `unverified` and no
document quotes a TinyOS temperature as a measurement. **A stated "unverified" passes this
clause; a confident number does not.**

**Clause 8 — nothing acts on the reading.** No fan control, no throttle, no policy anywhere on
this path. `LE-75` stays open until an actuation Story exists, and the operational rule holds:
power for a run, power down after.

## What this test does not cover

- **Actuation of any kind.** Driving the fan is a separate Story with its own hazard argument;
  a duty cycle derived from an unvalidated sensor is a physical error waiting to happen.
- **Whether firmware is protecting the SoC.** `throttled=0x0` says the firmware was not
  throttling at the moment of one capture. It does not establish what firmware would do under
  sustained load, and nothing here should be read as evidence that it would.
- **RP1's own temperature sensor**, which the ground truth also shows (`rp1_adc`) and this
  Story does not read.
