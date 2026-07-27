# STORY-P1-07-01 — AArch64 Target Spec, Boot Stub, `EL2 → EL1`, and the First Byte on the Wire

Status: **Specified — not started; needs the board, a serial adapter, and `TEST-P1-07-01-A` Red first**
Feature: [`FEAT-P1-07`](../features/FEAT-P1-07.md)
Introduced in: [`session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md`](../../session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md) §5, piece 1 and piece 2 of the `LE-09` slice

## Description

Piece 1 and piece 2 of the minimal Pi 5 slice, joined into one Story because neither is observable without the other: a target spec and boot stub with no UART produces silence, and a UART driver with no boot path is unreachable code.

The deliverable is the smallest thing that proves the board is running TinyOS's own instructions: a named AArch64 target that builds `hal-arm64`, a stub that establishes a stack, zeroes `.bss`, drops to `EL1` if the firmware entered at `EL2`, and writes a known byte sequence to the PL011 debug UART.

**Pi 4 material is actively misleading here.** The Pi 5 is a larger departure than the version number suggests, and the debug UART is a dedicated 3-pin connector distinct from the GPIO header. Every register address and the expected baud must be verified against current BCM2712 documentation and Raspberry Pi firmware notes, not inherited from a tutorial.

## Depends on

`FEAT-P1-02` (the recorded gate — a fault on this board is a silent hang until `STORY-P1-07-02` lands, which is why that Story is next and not later). Nothing in `hal-x86_64`.

## Acceptance criteria

1. **A named target spec builds `hal-arm64` for the board.** The target is committed, reproducible from a clean checkout, and named in the run path `STORY-P1-07-05` later automates. Building is necessary and not sufficient — a target spec that builds and produces an image the firmware silently rejects is the failure mode this criterion cannot detect on its own, which is why criterion 3 is the real one.
2. **The stub establishes a stack, zeroes `.bss`, and drops to `EL1` if entered at `EL2`.** The exception-level transition is conditional on `CurrentEL`, never assumed: the firmware's entry level is an input, not a constant.
3. **`CurrentEL` is printed before anything else.** The first thing on the wire says which exception level the firmware handed over at. This is the Story's cheapest and most valuable output: it converts the plan's second risk row ("entered at `EL2`, code assumes `EL1`") from a two-hour bisect into a line of text.
4. **A known byte sequence reaches the host over PL011.** Evidence is a serial capture, quoted verbatim in `TEST-P1-07-01-A`.
5. **Everything that can be host-tested is host-tested.** The PL011 register writes sit behind a one-method MMIO seam the way `STORY-P1-01-03` put `mrs` behind `VirtualCounter` — the concrete volatile-write implementor is the only `cfg(target_arch = "aarch64")` item and the only `unsafe`; the flag-polling, framing and character encoding are pure and tested on the dev host. A bring-up Story is where this discipline is most tempting to skip and most expensive to have skipped.

## Named debt this Story leaves open

- **`SEC-01` is selected and cannot be closed.** The Pi 5 firmware chain gives TinyOS no measured-boot evidence, so `BND-01` ("only authentic measured boot state reaches C1") is stated debt for the whole Feature. Naming it here is the point; a boot Story that omitted the control would read as though the question had not come up.
- **`LE-09` stays open.** A byte on a serial line is not a measurement.
- The firmware handoff register state and the device-tree blob pointer are read and reported, never retained as authority (`PD-14`, `BND-02`). No DT parsing (`BND-03`).

## Tests

[`TEST-P1-07-01-A`](../tests/TEST-P1-07-01-A.md) — written before implementation, per the TDD mandate.
