# Handover 04 — `STORY-P1-01-03`: The ARM64 Cycle Source, and the Seam That Held

Follows: [`03-le-09-arm64-pi5-slice-proposal.md`](03-le-09-arm64-pi5-slice-proposal.md), whose recorded decision — **Option B with the carve-out** — this session executed. Evidence: [`REPORT-2026-07-27-03`](../../goals/reports/REPORT-2026-07-27-03.md).

## What this session did

Delivered the carve-out: piece 3 of the `LE-09` Pi 5 slice, the only piece that needs no board. New Story `STORY-P1-01-03`, under the spine's own rules (contract row and Test document first) and strict TDD.

- **Test document first** ([`TEST-P1-01-03-A`](../../goals/tests/TEST-P1-01-03-A.md)), eight clauses, written before any implementation; contract row added to [`story-contracts.tsv`](../../goals/assurance/story-contracts.tsv) before the Story entered progress.
- **Red recorded, then Green.** 12 new tests against stubbed bodies: `0 passed; 12 failed`. Implementing the bodies turned all 12 green with no test edited.
- **The backend**: `hal_arm64::timer` — `Cntvct<R>` (`CycleSource` implementor #2), `GenericTimerTimebase` + `plausible_cycles_per_us` (`CNTFRQ_EL0` → cycles/µs, rounded to nearest), and `SystemRegisters` (the real `isb; mrs CNTVCT_EL0` / `mrs CNTFRQ_EL0`, the crate's only `unsafe` and its only `cfg(target_arch = "aarch64")` item).
- **The register reads sit behind two one-method traits** (`VirtualCounter`, `CounterFrequency`), so the two `mrs` instructions are not merely the only untestable code — they are the only *untested* code. Everything else runs on the x86_64 dev machine.

## The result worth reading: the seam held, and the proof is a diff

`STORY-P1-01-01` claimed `hal::time::CycleSource` was a real architectural seam. That claim could not be checked, because a trait with one implementor is a guess. It can be checked now:

**No line of implementation changed in `os/src/hal/`, `os/src/kernel/` or `os/src/xtask/`.** A host test drives `Calibration`, `Samples`, `Stopwatch` and `Report` with the ARM64 source and emits a well-formed `TINYOS-MEAS/1` envelope (`arch=aarch64 cycle_source=cntvct_el0 cycles_per_us=54`); the unmodified `xtask::timing::parse_stream` reads it. Outside the new crate and the assurance spine, the whole diff is one added `#[cfg(test)]` parser case and one workspace-member line.

Two speculative design decisions from last session turned out to be load-bearing, neither needing a change: `Timebase`'s `Option<u32>` return (added for QEMU/TCG's absent timebase) also covers firmware that never programmed `CNTFRQ_EL0` — a different reason for the same honest `None`; and the envelope's `arch`/`cycle_source` fields are what let one parser read both architectures with no version bump.

## The finding: a 54 MHz ruler is a coarse ruler

`CNTVCT_EL0` is a fixed-frequency **system** counter, not a CPU cycle counter. On a Pi 5 it runs at 54 MHz — one tick is ~18.5 ns, against ~0.43 ns for a TSC tick at the 2,302–2,307 cycles/µs `REPORT-2026-07-27-02` measured. A ~100 ns context switch is about **five ticks** on the target board.

This changes how hardware evidence must be read, and it is better known now than after the board arrives:

1. A **calibrated overhead of zero is an ordinary reading** there, not a failure. The harness already behaved correctly; a clause now pins it, so nobody later adds an innocent "overhead must be positive" assertion.
2. The smallest metrics will be **quantization-limited, not noise-limited** — a different statistical problem from Tier 0's 39–61% p99 variation. `STORY-P1-01-02` must not design its gate assuming the hardware tier is a quieter Tier 0; the two tiers are noisy for unrelated reasons and a tolerance derived from one does not transfer.
3. `PMCCNTR_EL0` (the PMU cycle counter) would be finer, but is not architecturally guaranteed accessible, must be enabled at EL1, and responds to frequency scaling. New loose end **`LE-15`** — a decision for when a board exists, deliberately not made on paper today.

## What was verified about the two `mrs` reads, and what was not

Not executed — no AArch64 target spec, boot path or fixture exists (pieces 1, 2, 5, sequenced after `FEAT-P1-02`). Beyond "it compiles": `cargo check -p hal-arm64 --target aarch64-unknown-none` is clean, and the emitted assembly from a throwaway out-of-tree `staticlib` that calls both reads shows `isb; mrs x0, CNTVCT_EL0` and `mrs x8, CNTFRQ_EL0`. So the assembly assembles, names the intended registers, and keeps the ordering barrier ahead of the count read. That the registers then hold what the ARM ARM says they hold is asserted, not verified.

## Naming note

The crate is `hal-arm64`, not `hal-aarch64`: [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md#crate-map)'s crate map already reserved that name for the ARM64 backend, and inventing a second name for the same planned crate would have created drift. That table's "Created in" cell now records the early, deliberately narrow creation — this crate holds one module and must not accumulate boot, MMU, GIC or device-tree code here; that remains `EPIC-P7`.

## Verification

`cargo test -p hal-arm64` 12/12 · `cargo test --workspace --lib` 209 · `cargo test -p xtask` 52 · `cargo fmt --all -- --check` clean · scoped `clippy -D warnings` clean · `cargo check -p hal-arm64 --target aarch64-unknown-none` clean · `check-assurance-spine`: 14 Features, 36 Stories, 26 Tests, 32 Reports · `check-crate-sizes`: `hal-arm64` 234 lines.

Clippy is scoped rather than `--workspace --all-targets` for the pre-existing reason `REPORT-2026-07-26-09` recorded (`kernel`/`exec` bin targets carry ELF-specific `global_asm!` that a Windows host cannot assemble). `hal-arm64` itself has no bin target and builds everywhere; Linux CI remains the authoritative `--all-targets` gate.

## Loose-ends register (canonical as of this handover)

Carried forward from [Handover 02](02-story-p1-01-01-measurement-harness.md#loose-ends-register-canonical-as-of-this-handover) with per-item status; one new item, none closed.

| ID | Loose end | Origin | Owner / fix path | Status |
|---|---|---|---|---|
| LE-01 | Priority-inheritance behavioral half never verified (host-only bookkeeping proof) | `STORY-P0-02-03` | `STORY-P1-04-01` acceptance criterion 2 | Open — owned |
| LE-02 | WCET enforcement has no timer and no watchdog behind it | `STORY-P0-02-04` | `STORY-P1-04-02` | Open — owned |
| LE-03 | No real `#PF`/`#GP`/`#UD` handling; every fault is terminal diverge-and-report | Handover 32 | `FEAT-P1-02` (`STORY-P1-02-01`); also route `#XF` | Open — owned |
| LE-04 | No TSS/IST; a fault during fault handling triple-faults | Handover 32 | `STORY-P1-02-02` | Open — owned |
| LE-05 | `exec::AddressSpace` built but never installed; system runs all-RWX identity-mapped | `STORY-P0-05-02` | `FEAT-P1-03` | Open — owned |
| LE-06 | `pool-bench` was a divergent sibling harness | Handover 35 | `STORY-P1-01-01` | **Closed 2026-07-27** |
| LE-07 | CI has never been observed running any of this work | Standing since Handover 07 | Phase-independent | **Closed 2026-07-27** |
| LE-08 | I/O APIC device-IRQ routing deferred (local APIC only) | `STORY-P0-04-02`/`-03` | Whichever Story first routes a device IRQ | Open — deferred with trigger |
| LE-09 | Pi 5 (ARM64) is the short-term hardware, but no ARM64 bring-up slice or deploy path exists | Handover 37 directive 1 | Decided: Option B with the carve-out ([Handover 03](03-le-09-arm64-pi5-slice-proposal.md)) | Open — **carve-out delivered 2026-07-27** (`STORY-P1-01-03`: cycle source + timebase, host-tested, no board). Piece 4 (UART-borne pass/fail) still belongs to `STORY-P1-01-02`; pieces 1, 2 and 5 (boot + target spec, PL011 UART, SD-card/serial run path) wait for `FEAT-P1-02`. **The item leaves this register only when a Pi 5 has produced a parsed measurement** — a host-tested backend is not hardware evidence, and no board has been confirmed purchased |
| LE-10 | ECAM/MCFG config access and PCI bridge traversal deferred | `STORY-P0-04-03` | First Story needing extended config space | Open — deferred with trigger |
| LE-11 | `Context::new` seeds task `rflags` with `IF` set, so switching into a task enables interrupts with no IDT installed — fail-open | `STORY-P1-01-01` | `FEAT-P1-02` | Open — owned |
| LE-12 | CI's clippy never lints target-only fixture code | `STORY-P1-01-01` | Add per-fixture target clippy to the CI lint job | Open — unowned, needs a Story |
| LE-13 | Measurement runs **dev-profile** (unoptimized) binaries | `STORY-P1-01-01` | `STORY-P1-01-02` | Open — owned |
| LE-14 | `context::switch` saves no SSE/x87 state | `STORY-P1-01-01` / ADR 0003 | `FEAT-P1-04` | Open — owned |
| **LE-15** | The AArch64 generic timer is a 54 MHz fixed-frequency system counter (~18.5 ns/tick), so the smallest hardware metrics will be quantization-limited; `PMCCNTR_EL0` is finer but is not architecturally guaranteed accessible, needs EL1 enablement, and tracks frequency scaling | `STORY-P1-01-03` (this handover) | Decide when a board exists — either accept the resolution and report it, or add a `PMCCNTR_EL0` `CycleSource` behind the same trait (which the seam now demonstrably supports). Also an input to `STORY-P1-01-02`'s gate design | Open — owned |

## Next session — start here

1. **`STORY-P1-01-02`** — committed baselines and the `check-timing-regression` CI gate, demonstrated to fail on a deliberately-introduced regression. Three inputs it must fold in: gate `min`/`p50` over medians of repeated runs (never Tier 0 tails, per `REPORT-2026-07-27-02`); release-profile measurement (`LE-13`); and the UART-borne pass/fail bit (`LE-09` piece 4) that any hardware tier needs, since a gate reading only a QEMU exit code can never gate a board. `LE-15` says its hardware tolerances cannot be extrapolated from Tier 0 noise.
2. Then **`FEAT-P1-02`** (real exception handling), per the Epic's ordering — carrying `LE-03` (route `#XF`), `LE-04` and `LE-11`. Its exit is also what unblocks `LE-09`'s remaining pieces.
3. **When `FEAT-P1-02` exits, the board question becomes live**: a Raspberry Pi 5, SD card and USB-TTL serial cable. Nothing in this repository records one as purchased. That is the real critical path for hardware evidence, and it is now the only thing between the harness and a Tier 1 number.

## What this handover does not do

No hardware ran anything. No boot path, no UART, no target spec, no deploy path. No `PERF-D04`/`D05`/`D07` guardrail is closed, no Story moved to `verified`, and `LE-09` stays open and release-blocking. What changed is that the harness's arch-neutrality is now a checked fact instead of a claim — and that the next architecture's backend is known to cost a single module.
