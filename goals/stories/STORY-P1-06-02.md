# STORY-P1-06-02 — The Actuation Path Reaches the Architecture That Is Supposed to Be Real-Time

Status: **In progress — host half Green (an `OutputLine` backend over RP1 bank-0 GPIO, driven by a recording MMIO window, with the claim sequence and the single-store command write asserted register by register); criterion 4 needs one board run**
Feature: [`FEAT-P1-06`](../features/FEAT-P1-06.md)
Introduced in: `session/hand-2026-08-06/10A` session

## Description

**`FEAT-P1-06` is the `G-PA-1` flagship path, and until 2026-08-06 every line of its evidence
was Tier 0 QEMU `x86_64`.** [`ADR 0004`](../../docs/adr/0004-arm64-is-the-real-time-tier.md)
designates ARM64 the real-time tier. `os/src/hal-arm64/src/` contained no `actuation` module and
no `OutputLine` implementation: the arch-neutral trait in
[`os/src/hal/src/actuation.rs`](../../os/src/hal/src/actuation.rs) was built explicitly so a
Pi 5 backend could slot in without the kernel path, the fixtures or the measurement harness
changing a line — and nothing had slotted in.

[`REPORT-2026-07-29-02`](../reports/REPORT-2026-07-29-02.md) is careful that *the boards carry
the product's numbers*. What was written nowhere is that **the mechanism itself had never run on
the architecture this project designates as real-time.** That is what this Story fixes, and it
was found by checking rather than by reading — handover `09A` §11, filed as `LE-85`.

It is worth stating what this does *not* change. `09A` §3 established that the board unblocks no
release gate: every release guardrail in every implemented in-play domain is `Host`/`T0` tier,
and `cargo run -p xtask -- assurance-status` derives that rather than asserting it. This Story is
the inversion of that finding — the board moves no *gate*, and it is nonetheless the only thing
that can carry `G-PA-1`'s mechanism onto the RT tier.

## The output boundary, and why these eight pins

**GPIO 20..27, bank 0, driven through `sys_rio0`'s XOR alias.**

- **Eight contiguous pins** because [`hal::actuation::OutputLine`] writes a byte, and the
  x86_64 stand-in is a byte-wide ISA port write. A backend that dropped seven bits would be
  measuring a narrower path than the one the Tier 0 numbers describe.
- **20..27 and not 0..7** because GPIO 0 and 1 are the HAT ID EEPROM's `ID_SD`/`ID_SC`. A
  stand-in that quietly drives a bus the board uses for something else is not a stand-in — the
  same argument `hal_x86_64::actuation` makes for choosing port `0x80` over the DMA page
  registers.
- **The XOR alias, not SET-then-CLEAR**, because a byte written as two stores presents a
  transient in which some bits have changed and others have not. On an actuator command bus that
  transient is a real intermediate command. XOR against a shadow of the last value updates
  exactly the changed bits, in exactly one store, and touches no other pin in the bank.

**One named limitation, stated rather than discovered.** Two identical consecutive commands
produce two bus stores and no change in pin state, so a downstream device cannot distinguish
them without a strobe line or an edge-encoded protocol. This backend does not coalesce, buffer
or replay — the trait's actual prohibitions — and every call performs exactly one unconditional
store. A real parallel actuator interface will need a strobe; inventing one now, with no
implementor that could honour it, is the speculative-consumer trap
`agent/CODING_STANDARDS.md` names.

## Depends on

`STORY-P1-06-01` (the kernel-side `ActuationPort` and the trait it is written against);
`STORY-P1-09-01` (the RP1 peripheral window, validated before any use);
`STORY-P1-09-04` (`rp1_gpio`, whose bank-1 register derivation this reuses for bank 0).

## Acceptance criteria

1. `hal_arm64::actuation` implements `hal::actuation::OutputLine` over RP1 bank-0 GPIO 20..27,
   and the type is generic over the same `Mmio` window `rp1_gpio` uses, so it is driven by a
   recording window in host tests and by the real peripheral window on the board. **Met**:
   `Rp1CommandLines`, `hal_arm64::actuation::tests`.
2. The claim sequence is glitch-ordered by construction — pad output-enable, then level and
   direction staged through RIO, then function select hands RIO the pins — so the first driven
   value on every one of the eight lines is a deliberate zero and never a float. Asserted as an
   **ordered** register trace, not as a set of writes. **Met**:
   `claim_stages_level_and_direction_before_handing_rio_the_pins`.
3. `write_command` performs **exactly one** bus store per call, unconditionally, touching only
   the eight owned pins; it allocates nothing, takes no lock, contains no loop and no wait.
   **Met**: `every_command_is_exactly_one_store`,
   `a_command_touches_no_pin_outside_the_owned_byte`, and the no-heap gate for the allocation
   half.
4. The path runs **on silicon**: an image carrying this backend boots on the Pi 5 and the
   actuation fixture's command sequence is observed leaving the board. **Not met — needs one
   power cycle.** Until then this Story is `In progress` and says so, per `06A` §4.1: a Story
   claiming every criterion is met may not also read `In progress`, and the converse is the
   rule this header follows.

## Tests

Host: `hal_arm64::actuation::tests` (driven by a recording MMIO window, the same harness shape
`rp1_gpio::tests` uses). Cross-target: `cargo run -p xtask -- check-boot-images` compiles this
for AArch64, which is the only local gate that does — `LE-72`.

## Goals verified

`G-PA-1` (the mechanism, now on the architecture `ADR 0004` designates real-time). No
performance guardrail is closed by this Story and none is claimed: `PERF-D03-G04` and
`PERF-D05-G04` are bound-class and refused from any platform absent from
`qualified-platforms.tsv`, where the count is zero, the Pi 5 included.
