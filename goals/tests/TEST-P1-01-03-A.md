# TEST-P1-01-03-A — AArch64 Generic-Timer Cycle Source and Timebase: Conformance, Honest Absence, and Harness Drop-In

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P1-01-03`](../stories/STORY-P1-01-03.md)
Tier: Host unit tests only (`cargo test --workspace` on the x86_64 dev machine and in CI). Deliberately **no** Tier 0 and **no** Tier 1 clause — this Story owns no board, no boot path and no UART; those are `LE-09`'s pieces 1, 2 and 5, which the 2026-07-27 decision sequenced after `FEAT-P1-02`.
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D04`, `D05`, `D07`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-15`, `BND-16`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-08`
Code admission gates: `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

`STORY-P1-01-01` made a claim it could not check: that `hal::time::CycleSource` and `hal::time::Timebase` are a real architectural seam, so a second implementor drops into `kernel::measure` without the harness, the envelope format or any fixture changing. A trait seam with exactly one implementor is a guess. Every clause below exists to turn that guess into a checked fact **before** a Raspberry Pi 5 exists to check it on — which is why every clause is host-runnable and none needs hardware.

The two `mrs` instructions that read `CNTVCT_EL0` and `CNTFRQ_EL0` are the only part that cannot be host-tested. Clause 1 exists so that they are also the *only* untested part: everything else — the `CycleSource` implementation, the frequency arithmetic, the plausibility policy and the drop-in — is driven through a substitutable register-read seam and tested on the dev machine.

## Specification

### 1. The register reads are behind their own segregated seam

**Given** the AArch64 backend,
**then** reading `CNTVCT_EL0` and reading `CNTFRQ_EL0` are two separate one-method traits, not one two-method trait, mirroring the `CycleSource`/`Timebase` split they respectively serve (`agent/CODING_STANDARDS.md#i--interface-segregation`): counting code that never converts to microseconds must not depend on a frequency register it never reads.

**And** the concrete implementor that actually executes `mrs` is compiled only under `cfg(target_arch = "aarch64")`; every other item in the module — the cycle source wrapper, the frequency arithmetic, the plausibility policy — is unconditional and therefore compiled, linted and tested on the x86_64 host.

**And** `unsafe` appears in exactly one place: the two `mrs` reads, each with its own `// SAFETY:` comment (`agent/CODING_STANDARDS.md#unsafe-code-policy`).

### 2. The AArch64 cycle source passes the *shared* conformance suite

**Given** the AArch64 cycle source driven by a host double that advances monotonically,
**when** `hal::time::conformance::check` is run against it,
**then** it passes and reports the observed span — the identical suite, with the identical expectations, that `hal_x86_64::tsc::Tsc` passes. This is the Liskov requirement in `agent/CODING_STANDARDS.md#l--liskov-substitution` discharged for the trait's second implementor, and it is the whole reason the suite was written as ordinary runtime checks rather than `#[cfg(test)]` helpers.

**And** when the same wrapper is driven by a stuck double it fails with `NoForwardProgress`, and by a backwards-going double it fails with `WentBackwards` — the wrapper must not launder a non-conforming counter into an apparently-conforming source by clamping, latching or caching.

**And** the wrapper adds no state and no arithmetic of its own: the value it returns is the value the register seam returned, unmodified.

### 3. Cycles-per-microsecond is derived, not calibrated

**Given** a counter frequency in Hz read from `CNTFRQ_EL0`,
**then** the timebase is that frequency converted to cycles per microsecond directly, with **no** PIT-style measured calibration of any kind — the AArch64 generic timer reports its own frequency architecturally, which is the one respect in which this backend is strictly simpler than x86_64's.

**And** the conversion rounds to nearest rather than truncating: 54,000,000 Hz yields 54; 19,200,000 Hz yields 19; 1,000,000 Hz yields 1; 24,576,000 Hz yields 25, not 24.

**And** the conversion is a pure `const fn` over `u64`, with no floating point anywhere (`no_std`, no `libm`), so it is fully host-testable independently of any register.

### 4. An untrustworthy frequency register yields no timebase, never a guess

**Given** a `CNTFRQ_EL0` value that cannot be trusted,
**then** `cycles_per_us` returns `None` and every downstream artifact reports cycles only — never a plausible-looking default, per the `Timebase` trait's documented contract.

Specifically **`None`** for:

- **zero** — firmware that never programmed `CNTFRQ_EL0` is a real, documented condition on ARM boards, not a hypothetical, and it is exactly the input a truncating divide would silently turn into a timebase of 0;
- a frequency **below 1 MHz**, which cannot produce a nonzero cycles-per-microsecond figure at all;
- a frequency **above 100 GHz**, which no generic timer implements and which therefore indicates a bad read rather than a fast board.

**And** the plausibility floor is deliberately *not* the x86_64 backend's: `hal_x86_64::tsc` rejects anything under 10 cycles/µs because a timestamp counter that slow would be a broken TSC, whereas a generic timer at 1–2 MHz is an ordinary, conforming implementation. A backend that copied the other backend's bounds would reject valid hardware, so the bounds are stated and tested per architecture.

### 5. The harness accepts the new source with no change to the harness

**Given** `kernel::measure` exactly as `STORY-P1-01-01` left it,
**when** a host test drives the full measurement path — `Calibration::measure`, `Samples`, `Stopwatch`, `Report::begin`/`metric`/`end` — with the AArch64 cycle source in place of `Tsc`,
**then** it compiles and produces a well-formed `TOS64-MEAS/1` envelope whose `arch=aarch64` and `cycle_source=cntvct_el0`, and **no implementation code under `os/src/kernel/`, `os/src/hal/` or `os/src/xtask/` is modified by this Story**. A diff touching the harness itself would falsify the claim this Story exists to test.

**And** that exact envelope text parses cleanly through the existing `xtask::timing::parse_stream`, with its `arch` and `cycle_source` fields carried through — the host-side tool is arch-neutral too, or the drop-in is only half real. This is checked by one added case in `xtask`'s existing `#[cfg(test)]` module: a new test is not a modification to the implementation, and the parser's own code stays untouched.

### 6. A coarse counter is a supported case, not an error

The generic timer is a fixed-frequency system counter, not a CPU cycle counter: on a Raspberry Pi 5 it runs at 54 MHz, so one tick is ~18.5 ns and a paired-read calibration can legitimately measure an overhead of **zero** ticks.

**Given** a calibration that observes zero overhead,
**then** the harness records it as zero and `correct` is the identity — no division, no "must be positive" assertion, no rejection. A backend whose calibrated overhead rounds to zero is a coarse counter reporting honestly, and treating it as a failure would make the harness unusable on exactly the board this slice targets.

### 7. Adversarial: the seam cannot be bypassed

**Given** the whole `hal-arm64` crate,
**then** no item in it names `rdtsc`, `x86_64`, or any x86 register, and `kernel::measure` continues to name no architecture at all. Two backends that both compile is not the property under test; two backends that both satisfy one unmodified consumer is.

### 8. What this test explicitly does **not** establish

- **No hardware evidence whatsoever.** No Pi 5 has booted, and none is required to run any clause here. `LE-09` stays open: a sequencing decision plus a host-tested backend is not a measurement.
- **The two `mrs` reads are unexecuted.** They are compiled only for `aarch64`, and nothing in this Story builds an `aarch64` target — piece 1 (boot + target spec) is what makes that possible, and it waits for `FEAT-P1-02`. Every clause above tests the code *around* those two instructions; that the instructions themselves read the registers the manual says they do is asserted, not verified, and is named as such in this Story's Report.
- **No microsecond-denominated `PERF-D04`/`D05`/`D07` guardrail closes.** A correct conversion factor for a board that has never run converts nothing.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/timer.rs`), run via `cargo test --workspace`. No QEMU dependency, no hardware dependency, no new CI job.

## Implementation location

- `os/src/hal-arm64/src/lib.rs` — the new backend crate (workspace member).
- `os/src/hal-arm64/src/timer.rs` — the register-read seam, the `CNTVCT_EL0` cycle source, the `CNTFRQ_EL0`-derived timebase, and its plausibility policy.
- `os/Cargo.toml` — the workspace member entry.
- `os/src/xtask/src/timing.rs` — **test module only**: one added case parsing an `arch=aarch64` envelope.

Deliberately **unmodified**: every line of implementation in `os/src/hal/src/time.rs`, `os/src/kernel/src/measure.rs` and `os/src/xtask/src/timing.rs`. Clause 5 is a claim about the diff, not only about the test output.

## Reports

- [`REPORT-2026-07-27-03`](../reports/REPORT-2026-07-27-03.md) — Red run recorded, then Green, with the drop-in diff evidence and the counter-resolution finding.
