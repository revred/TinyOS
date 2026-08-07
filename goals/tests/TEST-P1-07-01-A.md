# TEST-P1-07-01-A — The Board Says Which Exception Level It Woke Up At

Status: **Verified (Host + Tier 1 wire evidence), 2026-08-07** — clauses 2, 3, 4 and 5 Green, clauses 3 and 4 on the **substituted Ethernet channel** per the dated amendment at the end of this document; clause 1 is retired with the channel it tested; clause 6 Green for the reporting with its hardware half not claimed. **The specification below is unchanged since it was written before implementation** — the amendment is appended, never edited in, per this document's own 2026-07-28 precedent.
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

## Amendment, 2026-08-07 — the channel is the Ethernet wire, and the PL011 is retired from this test's evidence path

Per the precedent `TEST-P1-07-03-A` set (clauses re-read against their spirit,
each with its reason recorded, the original specification untouched above), and
executed on the owner's instruction to decide (`hand-2026-08-07/09A`). The
facts that force the substitution, all of record:

- The PL011 has produced **nothing, ever** — five consecutive zero-byte
  captures across every image, and the owner ruled the clause-1 loopback test
  infeasible on this bench (`LE-47`). Serial was demoted as an instrument on
  2026-08-03 (`hand-2026-08-03/08A`).
- The owner's standing direction since 2026-08-03 is that diagnostics ride
  **TOS64 envelopes over Ethernet** once the link trains. It trained on
  2026-08-04, and since 2026-08-07 a passive capture parses to its own verdict
  (`xtask parse-meas` exit 0).
- `STORY-P1-07-01`'s own status text named this exact fork — "a working
  adapter or an amendment substituting the channel with its reason" — and the
  adapter has not appeared in ten days of sessions.

| Clause | Re-read | Evidence |
|---|---|---|
| 1 — adapter proven before the board is blamed | **Retired with the channel.** Its purpose — never blame the board for a dead instrument — is carried on the wire path by instruments that demonstrably return both answers: the netboot serve log confirms each transfer digest before a byte executes, `ti64dink --until` exits 0 on sighting and 1 on timeout (both observed live), and `parse-meas` refuses a capture without a verdict (`LE-110`). | serve log discipline (`LE-87`), `wire-qual-2026-08-07-verdict.txt` |
| 3 — `CurrentEL` read, not assumed | **Green, with the ordering clause re-read as *captured at entry, reported when a channel exists*.** A channel that trains seconds after boot cannot carry entry-time bytes, and the retired channel never carried any; what the ordering existed to buy — knowing the firmware's entry level even when everything after it goes wrong — is bought by the raw register being saved at entry and quoted beside its decode, so a wrong decode is diagnosable from the capture alone. The conditional drop executed on silicon: `now_at=EL1`. Corroborated independently by `REPORT-2026-08-07-01`'s Q2 determination (TF-A BL31 hands off at NS-EL2). | `TOS64-QUAL/1 boot_entry current_el=EL2 raw=0x0000000000000008 now_at=EL1 firmware_cntvoff=0x0000000000000000` — verbatim, both boots, two owner power cycles |
| 4 — a known byte sequence reaches the host | **Green on the substituted channel, and stronger than written:** not merely a known sequence in order, but a machine-parsed envelope whose framing host tests pin, whose transfer digest was confirmed before execution, and which since boot 2 carries its own verdict. | `wire-qual-2026-08-07.txt` (boot 1), `wire-qual-2026-08-07-verdict.txt` (boot 2: `TOS64-RESULT/1 fixture=measure ok=true`, `parse-meas` exit 0) |

**What this amendment does not do.** It does not weaken clause 6 (still Green
for reporting only; no handoff line rides the wire and none is claimed), does
not close `SEC-01`/`BND-01` (stated debt, unchanged), does not revive any
timing claim (the qualification figures live in `REPORT-2026-08-07-01`, which
carries its own refusals), and does not assert the PL011 works — it asserts the
evidence this Story needed no longer waits on it.

## Reports

[`REPORT-2026-08-07-01`](../reports/REPORT-2026-08-07-01.md) — names this
test's clause 3 in its own header ("the entry exception level, read on the
board rather than assumed") and carries both captures. `LE-09` closed earlier
on `STORY-P1-07-06`'s Report (`REPORT-2026-08-04-01`), not on this Story, and
no `PERF-*` guardrail closes here.
