# Handover 23 — BCM2712 / Raspberry Pi 5 Divergence Record: Where Pi 4 Material Is Wrong, and How Each One Fails

Follows: [`21-next-session-mandate.md`](21-next-session-mandate.md). Companion to
[`24-story-p1-07-01-host-half.md`](24-story-p1-07-01-host-half.md), which is the session record.

`TEST-P1-07-01-A` clause 7 asks for the divergences from Pi 4 sources to be written down, on the
grounds that it is the most reusable output the first Pi 5 session produces and nobody else will
write it. This is that document. It is separated from the session handover deliberately: the
session is one day's work, and this is a reference the next four Stories will keep opening.

**Everything here is a transcription, not an observation.** No Raspberry Pi 5 has executed anything
from this repository. A transcription that is wrong fails as *silence* on a board with no fault
reporting, which is why every row below names how it fails as well as what it is.

## Sources, with revisions

| Source | What was taken from it |
|---|---|
| Raspberry Pi Linux `rpi-6.12.y`, `arch/arm64/boot/dts/broadcom/bcm2712.dtsi`, retrieved 2026-07-28 | `uart10` node (`reg`, `compatible`, `arm,primecell-periphid`), `clk_uart` frequency, the `soc` node's `ranges` |
| Raspberry Pi firmware `earlycon` parameter as documented for the Pi 5 debug connector | The base address, cross-checking the device tree; the `115200n8` line setting |
| Raspberry Pi documentation and community guidance on bare-metal loading on Pi 5 | `os_check=0`, and the `0x80000` versus `0x200000` load address |
| ARM PrimeCell UART (PL011) TRM | Register offsets, the 16.6 divisor, the `LCR_H` latching rule |

**Note the shape of that table.** Three of the four are Raspberry Pi sources and one is ARM's. That
is the divergence in miniature: the *peripheral* is standard and a Pi 4 tutorial describes it
correctly; everything about *where it is and what feeds it* changed.

## The divergences

### 1. The debug UART is `uart10`, at a 35-bit physical address — and truncation is silent

| | Pi 4 (BCM2711) | Pi 5 (BCM2712) |
|---|---|---|
| Debug PL011 | `UART0`, `0xFE20_1000` | `uart10`, `0x10_7D00_1000` |
| Reachable via | GPIO header pins 8/10 (GPIO14/15) | a dedicated 3-pin debug connector |

The device tree expresses the Pi 5 address as `serial@7d001000` inside a `soc` node whose `ranges`
is `<0x00000000 0x10 0x00000000 0x80000000>` — child address `0x7d00_1000` at CPU-physical
`0x10_0000_0000 + 0x7d00_1000`.

**How it fails.** The Pi 4 address fits in 32 bits and the Pi 5 address does not. Code carrying a
`u32`, a 32-bit `usize` habit, or a hand-typed `0x7D001000` does not fault: `0x107D001000`
truncated to 32 bits is `0x7D00_1000`, which is **ordinary DRAM on every Pi 5 SKU including the
2 GB one**. So the driver writes UART configuration words into memory, reports success, and the
line stays silent. There is no abort to catch and — until `STORY-P1-07-02` — nothing that could
report one anyway.

This is pinned as a test rather than a comment:
`board::tests::the_debug_uart_address_does_not_fit_in_32_bits_and_truncation_lands_in_ram`.

### 2. The UART reference clock is 9.216 MHz, fixed — not 48 MHz, and not configurable

| | Pi 4 | Pi 5 |
|---|---|---|
| Reference clock | 48 MHz default, via `init_uart_clock` in `config.txt` | 9,216,000 Hz, `clk_uart`, a fixed child of the 54 MHz oscillator |
| Derived from | the peripheral output of PLLD | the crystal oscillator directly |
| Divisors at 115200 | `IBRD = 26`, `FBRD = 3` | `IBRD = 5`, `FBRD = 0` |

9.216 MHz divides *exactly* at 115200: `9_216_000 / (16 × 115_200) = 5.0`. That is almost certainly
why the frequency was chosen, and it means the fractional divisor is zero on this board.

**How it fails.** A Pi 4 divisor programmed against a Pi 5 clock runs the line about **5.2× too
fast**. The far end frames nothing; the capture is garbage bytes rather than silence, which at
least is diagnosable — this is one of the few divergences here that announces itself.

**The second-order trap.** Because `FBRD` is 0 on this board, an implementation that computes the
fractional divisor *wrongly* still works on a Pi 5 and breaks on anything else. The classic PL011
defect — truncating `26.0417 × 64 = 1666.67` to `FBRD = 2` instead of rounding to 3 — is
untestable against this board's own numbers. `BaudDivisors::compute` therefore rounds to nearest
(matching the precedent `timer::plausible_cycles_per_us` set) and is tested against **both**
clocks.

**A related finding worth stating because it removed code.** With six fractional bits the rounded
divisor is never worse than `0.5/64` relative, i.e. under 0.8%, and only at the fastest expressible
rate. Async 8N1 framing tolerates roughly 2–3% combined. So a "requested rate not achievable within
tolerance" error variant would have been unreachable code defending against a state the hardware
cannot enter. It was written, found unreachable, and replaced by a test that asserts the bound
across a matrix of clocks and rates.

### 3. `os_check=0`, or the image is loaded at the wrong address

Pi 4 bare-metal material says `kernel8.img` loads at `0x80000` and stops there. On a Pi 5 the
firmware inspects the image, concludes it is a Linux kernel, and loads it at `0x200000` instead
unless `config.txt` contains `os_check=0`.

**How it fails.** Total silence. The linker script places `_start` at `0x80000`; the firmware
enters at `0x200000`; execution begins in the middle of the image at whatever instruction happens
to be there.

`targets/aarch64-tinyos.ld` and `board::KERNEL_LOAD_ADDRESS` both carry `0x80000` and a test pins
them together — but **neither can check `config.txt`**, which is on an SD card. That is the one
constant in this record with no test behind it, and it is the first thing to check when the board
says nothing.

### 4. The GPIO header UART has no Pi 5 equivalent at a fixed address

On a Pi 5, USB, Ethernet and GPIO sit behind the **RP1 southbridge, reached over PCIe**. There is no
poking a GPIO-header UART at a fixed physical address the way Pi 4 code does — reaching one is PCIe
bring-up plus an RP1 driver.

**How it fails.** As a project-planning error rather than a runtime one; it is already recorded as
`LE-26` (the `EPIC-P1_5` Ethernet transport decision) and as `FEAT-P1-07` §6's first non-goal. It
belongs here because it is the reason the 3-pin connector is not merely *convenient* — for Stories
`-01` through `-03` it is the **only** channel by which the board can say anything at all.

The debug connector's muxing (UART versus A76 SWD) is handled by the always-on GPIO block at
`0x10_7D51_7C00`. This slice does not touch it and assumes the firmware's default. If the connector
turns out to be muxed to SWD, the symptom is silence with everything else correct — check this
before suspecting anything in §1–§3.

### 5. Unaligned access is not merely slow, it faults

Not a Pi 4 divergence but an MMU-off one, and it lands on the same session. With `SCTLR_EL1.M`
clear every access is Device-nGnRnE, and unaligned accesses to Device memory take an **alignment
fault** — on a board that, until `STORY-P1-07-02`, has no vector table to report it.

Consequences already applied:

- the target spec keeps `+strict-align` and uses the **softfloat** ABI (`-neon`). FP/SIMD also traps
  until `CPACR_EL1.FPEN` is set, and Rust's default `aarch64-unknown-none` emits NEON for things as
  ordinary as a memcpy — a trap with no handler, from code nobody wrote;
- `__bss_start` and `__bss_end` are 16-byte aligned in the linker script, so the `stp`-based zeroing
  loop can neither overrun nor straddle.

### 6. What a Pi 4 source gets *right*

The PL011 itself. `arm,primecell-periphid = <0x0034_1011>` — it is ARM's PrimeCell, the register
offsets are the architected ones, and `LCR_H` still latches `IBRD`/`FBRD` on write. Saying so is
part of the record: the divergences are about address, clock, connector and load address, not about
the peripheral programming model, and treating *everything* Pi 4 as suspect would waste the one
part that transfers.

## Build recipe, and one thing `-Z build-std=core` alone gets wrong

```
cargo build -p hal-arm64 \
  --target targets/aarch64-tinyos.json \
  -Z build-std=core,compiler_builtins \
  -Z build-std-features=compiler-builtins-mem \
  -Z json-target-spec
```

- `-Z json-target-spec` is newly **required** by the pinned toolchain for `.json` target specs; the
  older invocation fails outright, with a clear message.
- `-Z build-std=core` **alone leaves `memcpy` and `memcmp` undefined at link time.** It compiles
  fine and fails only when something links, which — because this workspace links no AArch64 binary
  yet — is a failure the next session would have met first. `compiler_builtins` with the
  `compiler-builtins-mem` feature is the fix.

The build (not the link) now runs in CI on every push, so the target spec, the boot stub's inline
assembly and the `VolatileMmio` seam cannot rot silently between hardware sessions — which is
exactly the state `timer::SystemRegisters` sat in for three Features.

## Layout check on the linker script

Linked outside this workspace, against a throwaway `no_main` binary, because producing a real
`kernel8.img` needs an AArch64 binary crate and that is `STORY-P1-07-05`:

```
Idx Name           Size     VMA
  1 .text.boot     0000004c 0000000000080000 TEXT
  2 .text          0000c16c 0000000000080050 TEXT
  3 .rodata        00003884 000000000008c1c0 DATA
  4 .bss           00000000 000000000008fa50 BSS
  5 .stack         00010000 000000000008fa50 BSS

0000000000080000 <_start>:
   80000: d53800a4     mrs  x4, MPIDR_EL1
   ...
   80040: 9400005e     bl   0x801b8 <entry>
```

`objcopy -O binary` produced a 62 KiB flat image whose first bytes are `a4 00 38 d5` — the
`mrs x4, MPIDR_EL1` at `_start`. So the entry point is the first byte of the file, which is what the
firmware requires and what an `ENTRY()` directive alone would not have guaranteed.

**This is a layout check, not `TEST-P1-07-01-A` clause 2's evidence.** Clause 2 asks for the image
artifacts the boot firmware expects, and this workspace does not build one.

## What is still unverified in this document

Every row. In particular, the three things the first board session should confirm before trusting
anything above:

1. **The connector pinout and its ground.** Taken from product documentation, not from a board in
   hand. Loopback-test the adapter first (`TEST-P1-07-01-A` clause 1) — a suspected-dead board is
   usually a dead adapter, and this is the only clause in the Feature runnable before anything else
   exists.
2. **The entry exception level.** Assumed to be `EL2` by every Pi source, treated as an *input*
   everywhere in the code, and printed first precisely so it is never assumed again.
3. **That `uart10` is what the 3-pin connector is muxed to** at firmware default. If it is not,
   §4's last paragraph is where to look.
