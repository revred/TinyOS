# STORY-P1-07-01 — AArch64 Target Spec, Boot Stub, `EL2 → EL1`, and the First Byte on the Wire

Status: **Verified — criteria 1, 2 and 5 met since 2026-07-28; criteria 3 and 4 closed 2026-08-07 on the Ethernet wire, under `TEST-P1-07-01-A`'s dated channel amendment.** The fork this header carried — "a working adapter or an amendment substituting the channel with its reason" — was decided 2026-08-07 on the owner's instruction (`hand-2026-08-07/09A`): the amendment, per the `TEST-P1-07-03-A` precedent, because the PL011 has produced nothing ever (`LE-47`, loopback ruled infeasible; serial demoted 2026-08-03) while the owner's standing direction routes diagnostics over TOS64 envelopes on Ethernet, which since 2026-08-07 parse to their own verdict. Evidence is `REPORT-2026-08-07-01`'s two owner-power-cycled netboots: `TOS64-QUAL/1 boot_entry current_el=EL2 raw=0x0000000000000008 now_at=EL1` on both — the raw register captured at entry and quoted beside its decode (the ordering clause's re-read: a channel that trains after boot cannot carry entry-time bytes), with `now_at=EL1` proving the conditional drop executed, which is criterion 2's behaviour observed rather than inferred. Criterion 4's known byte sequence is the machine-parsed envelope: transfer digest confirmed before a byte executed, `xtask parse-meas` exit 0, `verdict fixture=measure ok=true` on boot 2 (`goals/reports/wire-qual-2026-08-07.txt`, `wire-qual-2026-08-07-verdict.txt`). **What this does not claim:** no PL011 byte has still ever been seen and none is asserted; `SEC-01`/`BND-01` remain stated debt; `LE-09` closed earlier on `-06`'s Report, not here.
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

## Progress, 2026-07-28

The Story was deliberately split along the line the board draws, the way
`STORY-P1-01-03` split the timer. **The half that needs no hardware is done and
Green on the x86_64 dev machine; the half that needs a board is untouched, and
this Story is not Verified.**

| Criterion | State |
|---|---|
| 1 — named target spec builds `hal-arm64` | **Green.** `os/targets/aarch64-tinyos.json` + `aarch64-tinyos.ld`. Built in CI on every push, so the spec cannot rot between hardware sessions. |
| 2 — stack, `.bss` zeroing, conditional `EL2 → EL1` drop | **Written, not executed.** `hal_arm64::boot`. The layout is confirmed by a link (`_start` at `0x80000`, first byte of the flat image); the *behaviour* is not. |
| 3 — `CurrentEL` printed before anything else | **Half.** The decode, the ordering and the wire text are host-tested against a double. The register read and the capture need the board. |
| 4 — a known byte sequence reaches the host | **Blocked.** Needs the board and the adapter. This is the Green. |
| 5 — everything host-testable is host-tested | **Green.** 64 host tests; `pl011::VolatileMmio` is the only `cfg(target_arch = "aarch64")` item and the only `unsafe` in the driver. |

**What is deliberately not here.** No `kernel8.img` is produced by this
workspace: that needs an AArch64 binary crate and an SD-image build, which is
`STORY-P1-07-05`, and pulling it forward is the scope creep `FEAT-P1-07` §6 and
Handover 17 §10's last risk row both name. The link that validated the linker
script was done outside the workspace and its recipe is recorded in
[`session/hand-2026-07-28/23-bcm2712-divergence-record.md`](../../session/hand-2026-07-28/23-bcm2712-divergence-record.md).

**No measurement was taken and none may be.** Until `STORY-P1-07-03` lands the
MMU every access on that board is Device-nGnRnE, and `TEST-P1-07-01-A` §7 says
what any number obtained here would be worth.

## Tests

[`TEST-P1-07-01-A`](../tests/TEST-P1-07-01-A.md) — written before implementation, per the TDD mandate.
