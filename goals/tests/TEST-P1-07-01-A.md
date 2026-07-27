# TEST-P1-07-01-A — The Board Says Which Exception Level It Woke Up At

Status: **Partially Verified (Host), 2026-07-28** — clause 5 Green, clause 2 Green for the build, clause 3 Green for the decode and the ordering; clauses 1 and 4 are untouched and need hardware. **Specification unchanged since it was written before implementation** — see "What was and was not run", below.
Story: [`STORY-P1-07-01`](../stories/STORY-P1-07-01.md)
Tier: Host unit tests (PL011 register encoding, flag polling, `CurrentEL` decoding, linker/target-spec shape) **plus** a Tier 1 hardware run on a Raspberry Pi 5, captured over the debug UART, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-01`, `SEC-19`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: `PERF-D01-G01`, `PERF-D01-G09`, `PERF-D01-G17` — boot latency, image footprint and cold start. None is *closed* by this Story; the run establishes that they can be measured on this board at all, which is a precondition none of them has ever had.

## What this test is for

This project has never executed one of its own instructions on hardware it owns. Everything in `EPIC-P1` — 39–61% tail variance included — is a statement about QEMU. The first byte on a serial line is worth more than every remaining Tier 0 Feature in this Epic, because it is the first thing this project has ever done that QEMU was not doing for it.

The test is written so that the *failure* modes are the interesting cases, because on a bring-up the failures are what actually happen.

## Specification

### 1. The adapter is proven before the board is blamed

**Given** a USB-serial adapter and no board,
**when** the adapter is loopback-tested against a known-good source,
**then** the capture matches what was sent.

This clause is first on purpose. A suspected-dead board is usually a dead adapter, and this is the only clause in the Feature that can be run before anything else exists. Buy two adapters. Confirm the expected baud from firmware documentation rather than assuming it.

### 2. The target spec builds, reproducibly

**Given** a clean checkout,
**when** the committed AArch64 target spec is used to build `hal-arm64`,
**then** the build succeeds and produces the image artifacts the boot firmware expects, with no host-local state.

**And** this clause is explicitly *not sufficient*: a target spec that builds and produces an image the firmware silently rejects passes clause 2 and fails clause 4. Clause 4 is the real test; clause 2 exists so that a failure at clause 4 is not ambiguous about which half broke.

### 3. `CurrentEL` is read, not assumed

**Given** entry from the Raspberry Pi firmware,
**then** the stub reads `CurrentEL` and prints it **before anything else**, and drops to `EL1` only if it finds itself at `EL2`.

**And** the drop is conditional. The firmware's entry level is an input, not a constant: hardcoding `EL1` and being wrong produces faults or silence with no way to tell which, and hardcoding `EL2` and being wrong does the same in the other direction. Printing it first converts the plan's second-highest risk into one line of text.

### 4. A known byte sequence reaches the host

**Given** a stack, a zeroed `.bss`, and an initialised PL011,
**when** the stub writes a known byte sequence,
**then** the host serial capture contains it, in order, at the expected baud.

**And** the capture is quoted verbatim in this document when the Story goes Green. A paraphrased serial capture is not evidence.

### 5. Everything except the volatile writes is host-tested (`SEC-19`)

**Given** the PL011 driver,
**then** its register offsets, its flag-polling loop, its baud-divisor arithmetic and its byte framing are pure functions with host unit tests, and the concrete volatile-write implementor is the **only** `cfg(target_arch = "aarch64")` item and the **only** `unsafe` — the seam `STORY-P1-01-03` established for `mrs` reads, applied to MMIO writes.

**And** the flag-polling loop is bounded. An unbounded wait on a transmit-FIFO-full flag is a hang indistinguishable from every other hang on this board, and it is the exact shape of hang this Feature's whole ordering exists to eliminate.

### 6. The firmware handoff confers nothing (`BND-02`, `PD-14`)

**Given** whatever register state and device-tree blob pointer the firmware leaves,
**then** it is read, reported over the UART, and **not retained as authority** by anything downstream.

**And** the device-tree blob is **not parsed** (`BND-03`). Reading the pointer is acceptable; walking the structure is a hostile-format parser in C1 and belongs behind the Security Charter's `C4` discipline, not in a bring-up Story. Addresses are hardcoded-and-verified against BCM2712 documentation, with the documentation revision recorded.

### 7. What this test explicitly does **not** establish

- **No verified boot.** `SEC-01` is selected and **cannot be closed by this Feature**: the Pi 5 firmware chain gives TinyOS no measured-boot evidence, so `BND-01` is stated debt. The control is named here rather than omitted so that no reader infers the question was considered and answered.
- **No fault reporting.** Until `STORY-P1-07-02`, a fault on this board is a silent hang. Every diagnostic in this Story is a `println`-shaped diagnostic and nothing more.
- **No caches.** The MMU is off, so every access on this board is Device-nGnRnE (`STORY-P1-07-03`). **No timing observation made during this Story means anything**, and none may be recorded as though it does.
- **No measurement, so `LE-09` stays open.**
- **Pi 4 material does not apply.** Every register address, the debug-UART connector and the expected baud are verified against current BCM2712 and firmware documentation. Where a Pi 4 source was consulted and found wrong, the divergence is recorded — it is the most reusable output this Story produces.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`) plus a Tier 1 hardware run captured over the debug UART.

## Implementation location

- `os/src/hal-arm64/` — target spec, boot stub, linker script, PL011 driver, `CurrentEL` read and the `EL2 → EL1` drop.

## What was and was not run, 2026-07-28

**No clause was edited to fit what happened.** The specification above is the
one written before implementation; this section is added below it, per the
precedent `TEST-P1-01-04-A` clause 4 set.

| Clause | State | Evidence |
|---|---|---|
| 1 — adapter proven before the board is blamed | **Not run.** No adapter has been loopback-tested. | — |
| 2 — the target spec builds, reproducibly | **Green for the build.** `cargo build -p hal-arm64 --target targets/aarch64-tinyos.json` succeeds from a clean checkout and now runs in CI. **Not Green for "the image artifacts the boot firmware expects"** — no `kernel8.img` is produced by this workspace; see below. | CI job `governance-gates`, step "AArch64 target spec builds hal-arm64" |
| 3 — `CurrentEL` is read, not assumed | **Green for the decode and the ordering.** `ExceptionLevel::decode` is total over the register, ignores `RES0`, and only `EL2` asks for a drop; a host test asserts the level is the first content on the wire. **The register read and the drop have never executed.** | `hal_arm64::exception_level`, `hal_arm64::boot` host tests |
| 4 — a known byte sequence reaches the host | **Not run.** This is the Green, and it needs the board. | — |
| 5 — everything except the volatile writes is host-tested | **Green.** 64 host tests across `board`, `exception_level`, `pl011` and `boot`. | `cargo test -p hal-arm64` |
| 6 — the firmware handoff confers nothing | **Green for the reporting; unexercised on hardware.** `x0`–`x3` and the DTB pointer are reported and dropped; the wire carries `parsed=no`. No device-tree parser exists. Addresses are hardcoded-and-verified with the documentation revision recorded in `hal_arm64::board`. | `hal_arm64::boot` host tests |

### Clause 5's "only `cfg(target_arch)` item, only `unsafe`" is read as scoped to the driver

Stated in the open rather than quietly satisfied. Clause 5 opens *"Given the
PL011 driver"*, and it is honoured exactly within that module:
`pl011::VolatileMmio` is the only `cfg(target_arch = "aarch64")` item and the
only `unsafe` in `pl011.rs`.

It is **not** true of the Story as a whole, and cannot be: clause 4 of this same
document requires "a stack, a zeroed `.bss`, and an initialised PL011", and
neither a stack nor `.bss` zeroing can be established without assembly.
`hal_arm64::boot` therefore contains a `global_asm!` reset vector, a `CurrentEL`
read and the `EL2 → EL1` drop, all `cfg(target_arch = "aarch64")` and all
`unsafe`. `agent/CODING_STANDARDS.md`'s language policy admits precisely this
case ("the earliest boot stub … may be written in a small amount of
hand-written assembly, wrapped by a Rust `extern "C"` boundary as thin as
possible").

The whole-Story reading is not merely inconvenient, it is unsatisfiable, so the
driver-scoped reading is the only coherent one. Recorded here so that a later
reader finds the interpretation rather than inferring that the clause was
ignored.

### What clause 2 does not cover, and why nothing was added to cover it

Clause 2 asks for "the image artifacts the boot firmware expects". This
workspace does not build one: there is no AArch64 binary crate, so nothing
links `hal-arm64` into an image. That is `STORY-P1-07-05` ("host-side run path:
SD image build"), and adding it here is exactly the growth `FEAT-P1-07` §6 and
Handover 17 §10's last risk row forbid.

The linker script was instead validated by linking a throwaway binary outside
the workspace. It produced `_start` at `0x80000` as the first byte of the flat
image, `.bss` and `.stack` 16-byte aligned, and a 62 KiB debug image. **That is
a layout check, not clause 2's evidence**, and the recipe — including the
`-Z build-std=core,compiler_builtins -Z build-std-features=compiler-builtins-mem`
that a `build-std=core` alone gets wrong — is recorded in
[`session/hand-2026-07-28/23-bcm2712-divergence-record.md`](../../session/hand-2026-07-28/23-bcm2712-divergence-record.md).

### One defect this specification's own tests found

A first implementation spelled its line endings `"\r\n"` inside
`Pl011::write_str`, which frames `\n` as `\r\n` — putting `\r\r\n` on the wire.
It is invisible on most terminals, so it would have survived review and landed
inside a *quoted serial capture* offered as evidence. The framer owns the CR;
the report supplies `\n` only, and
`no_reported_line_contains_a_carriage_return_the_framer_would_double` now pins
it. Recorded because the failure mode — a capture that is subtly not what the
source says — is the one this Feature's evidence rests on.

## Reports

To be filed when the Story goes Green. **Nothing here closes a `PERF-*`
guardrail and `LE-09` remains open**: a byte on a serial line is not a
measurement, and no byte has reached one yet.
