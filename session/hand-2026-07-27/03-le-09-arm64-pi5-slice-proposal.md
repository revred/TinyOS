# Handover 03 — `LE-09` Proposal: Minimal ARM64 / Raspberry Pi 5 Bring-Up Slice (decision needed)

Follows: [`02-story-p1-01-01-measurement-harness.md`](02-story-p1-01-01-measurement-harness.md). This document exists to be **decided on, not implemented from** — Handover 37's directive 1 made the Pi 5 the only viable short-term hardware, and mandate item 3 asked this session to scope the minimal slice and present two sequencing options. Nothing below is started.

## Why this is needed at all, in one paragraph

Every timing claim `EPIC-P1` makes is currently Tier 0 QEMU/TCG, and [`REPORT-2026-07-27-02`](../../goals/reports/REPORT-2026-07-27-02.md) just quantified how far that is from evidence: run-to-run p99 variation of 39–61% on small operations, absolute cycle counts inflated by an unoptimized dev profile and an emulated timestamp counter, and no way to distinguish "the code is slow" from "the emulator is noisy". The measurement harness is now built to survive the move — its cycle source is behind `hal::time::CycleSource`, and the ARM64 backend is a single trait implementor — so the remaining cost is bring-up, not redesign.

## What "minimal" means — the slice, in five pieces

Deliberately **not** a HAL port. The goal is one board that boots far enough to run the existing measurement fixtures and print an envelope; everything else stays in `EPIC-P7`.

| # | Piece | Scope | Est. |
|---|---|---|---|
| 1 | **Boot + target spec** | `os/targets/aarch64-tinyos.json`, linker script, `_start` in AArch64 assembly: land at EL1 (drop from EL2 if the firmware left us there), set up SP, zero `.bss`, no MMU, no cache configuration beyond what the firmware left, jump to `kernel_main`. Pi 5 boots via `config.txt` + `kernel8.img` from the SD card's boot partition, so no bootloader work is needed. | 2–3 sessions |
| 2 | **Serial (UART)** | The Pi 5's debug UART is a PL011 at a fixed MMIO base, reachable over the 3-pin debug header with a USB-TTL cable. Polled `write_byte`, mirroring `hal_x86_64::serial`'s shape exactly — the fixtures write through `core::fmt::Write` and do not care which UART. | 1 session |
| 3 | **Cycle source + timebase** | `CNTVCT_EL0` for cycles and `CNTFRQ_EL0` for the frequency. This is *easier* than x86: the counter frequency is architecturally readable, so `Timebase::cycles_per_us` needs no PIT-style calibration at all — it is one register read divided by 1,000,000. The whole ARM64 side of the harness is one `CycleSource` impl plus one `Timebase` impl. | <1 session |
| 4 | **Exit/result reporting** | There is no `isa-debug-exit` on hardware. The harness's pass/fail bit has to travel over the UART instead — one sentinel line the host-side tool reads (the envelope already carries `overall_ok`-style chatter, so this is a small, honest extension of the existing protocol, not a new one). | ~1 session, and it also benefits Tier 0 |
| 5 | **Host-side run path** | `xtask measure --tier=T1`: build the ARM64 fixture, produce `kernel8.img`, and read the UART capture (initially a human copying the image to an SD card and `xtask` reading a serial port; a network/deploy path is `EPIC-P1_5`'s job, not this slice's). | 1–2 sessions |

**Explicitly out of scope for the slice:** MMU/page tables, GIC/interrupt controller, device-tree parsing, timer interrupts, multi-core (`PSCI`), USB, networking, and any `hal::topology` ARM64 backend. `FEAT-P1-02`/`-03`/`-04` (faults, address spaces, preemption) stay x86_64-only until the board proves itself on measurement alone.

**Physical prerequisites the user owns:** a Raspberry Pi 5, an SD card, a USB-TTL serial cable, and a host machine that can read that serial port. No board has been purchased yet as far as this repository records — if that is still true, it is the real critical path, and option B below is the honest choice.

## The two sequencing options

### Option A — start the slice in parallel, now

Begin piece 1–3 alongside `STORY-P1-01-02` (baselines + the CI timing gate), so the first hardware numbers arrive while `EPIC-P1`'s x86_64 Features are still landing.

- **For:** hardware evidence stops being a lump of debt at the end of the Epic and becomes a stream. The harness's arch-neutrality gets validated *now*, while it is cheap to change — a trait seam nobody has ever crossed is a guess, not a design. Two of the five pieces (4 and 5) improve the Tier 0 path too. And the noise result above means Tier 0 alone cannot tell us whether any determinism claim is true; the sooner one real board exists, the sooner every subsequent Feature's Report can carry a number that means something.
- **Against:** it splits attention across two architectures during the Epic's most safety-relevant work (fault handling, address spaces), and if no board is in hand, "parallel" becomes "blocked while looking parallel".

### Option B — after `FEAT-P1-02`, sequentially

Finish the timing gate and real fault handling on x86_64 first; start the slice once `FEAT-P1-02` exits.

- **For:** one architecture at a time, and fault handling is the thing that makes *any* hardware bring-up debuggable — on a Pi 5 with no `isa-debug-exit`, a fault today is a silent hang with no output at all, whereas after `FEAT-P1-02` it is a reportable event over the UART. It also matches the current backlog (`EPIC-P7`) rather than pulling work forward, and it gives time to acquire the board without pretending to be blocked on scheduling.
- **Against:** every Report filed before then carries hardware-tier debt, and the arch-neutral seam stays unvalidated for longer — this session already found *two* bugs that only appeared when code first ran somewhere new, which is precisely the risk of deferring.

## Recommendation

**Option B, with one carve-out: do piece 3 (the `CNTVCT_EL0` cycle source and `CNTFRQ_EL0` timebase) now, as host-testable code, and piece 4 (UART-borne pass/fail) as part of `STORY-P1-01-02`.**

The reasoning is specific rather than a split-the-difference: piece 3 is under a session's work, needs no board to write, and is the *only* piece that tests the claim this Story just made — that a second `CycleSource`/`Timebase` implementor drops in without touching the harness. Writing it against the shared conformance suite either validates the seam or exposes it while it is still free to fix. Piece 4 belongs to the gate Story anyway (a gate that can only read a QEMU exit code cannot ever gate hardware). Everything requiring the physical board waits for `FEAT-P1-02`, by which time a fault on that board produces a message instead of a hang — and by which time the board can be bought without the schedule pretending it already exists.

**Decision requested:** A, B, or B-with-carve-out. If A, the first question back is whether a Pi 5 is in hand, because that answer changes the first session's work entirely.

## Loose-ends register — carried forward

Unchanged from [Handover 02's copy](02-story-p1-01-01-measurement-harness.md#loose-ends-register-canonical-as-of-this-handover); this document adds no new items and closes none. `LE-09` remains **open — decision needed**, and this proposal is the input to that decision.
