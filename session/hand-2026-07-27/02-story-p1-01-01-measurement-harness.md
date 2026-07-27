# Handover 02 — `STORY-P1-01-01`: The Measurement Harness, and Two Bugs Only Measuring on Target Could Find

Follows: [`01-story-p0-03-01-assurance-evidence.md`](01-story-p0-03-01-assurance-evidence.md) (a concurrent session's `PERF-D07` evidence work) and, for this session's mandate, [`Handover 37`](../hand-2026-07-26/37-epic-p1-mandate-hardware-direction-and-loose-ends.md). Accompanied by [`Handover 03`](03-le-09-arm64-pi5-slice-proposal.md), the `LE-09` proposal mandate item 3 asked for.

## What this session did

Delivered `STORY-P1-01-01` — `EPIC-P1`'s measurement harness, the ruler every other Feature in the Epic states its exit criteria in — under strict TDD, and closed `LE-06`. Full evidence: [`REPORT-2026-07-27-02`](../../goals/reports/REPORT-2026-07-27-02.md).

- **Test document first** ([`TEST-P1-01-01-A`](../../goals/tests/TEST-P1-01-01-A.md)), nine specification clauses, written before any implementation.
- **Red recorded, then Green.** Test suites written against stubbed bodies: 4 failures in `hal`, 16 in `kernel`, 29 in `xtask` (48 harness tests plus the assurance-spine count test). Implementing the bodies turned all 48 green with no test edited; the workspace now runs 197 tests.
- **The harness**: `hal::time::{CycleSource, Timebase}` plus a shared conformance suite (the Liskov requirement for a 2+-implementor trait, written as `no_std` runtime checks so the *same* suite runs in host tests, in the Tier 0 fixture, and later on ARM64 hardware where no test harness exists); `kernel::measure::{Samples, Calibration, Summary, Stopwatch, Report}` — fixed-capacity non-allocating buffers with explicit drop accounting, integer nearest-rank percentiles, a documented perturbation bound, and the versioned `TINYOS-MEAS/1` envelope; `hal_x86_64::tsc::{Tsc, calibrate_cycles_per_us}` as the x86_64 backend.
- **Fail-closed host tooling**: `xtask::timing` parses the envelope and treats *every* specified deviation as a harness error (exit 2) — unknown version, missing/repeated `BEGIN`/`END`, `END` count mismatch, missing/unknown/duplicate key, non-numeric value, duplicate metric, non-monotonic percentiles, `n=0`, unknown unit, no metrics, and truncation. One host test per case. Plus the new `xtask measure [--runs=N] [--fixture=measure|pool-bench] [--out=DIR]`, which boots the fixture with COM1 captured, parses each run, and reports per-metric run-to-run variance.
- **Tier 0 evidence for D04/D05/D07**: new `fixture_measure` (five metrics), three consecutive runs, all parsed, `dropped=0` throughout.
- **`pool-bench` refactored onto the harness**: its private `rdtsc`, calibration, percentile and prose-reporting code deleted; its numbers are now reachable through `xtask` instead of only a hand-written QEMU invocation.
- **Wired into CI the same day it landed**: `.github/workflows/ci.yml`'s QEMU job now runs `xtask measure --runs=1` for both measurement fixtures. It asserts that the fixtures boot, measure, and emit a stream the parser accepts — and deliberately asserts nothing about the numbers, because Tier 0 tail variance would make that noise. `LE-07`'s lesson (a gate nobody observes is not a gate) applied prospectively rather than after 30 handovers.
- **A guest-side microsecond timebase**: measured against a 10 ms PIT channel-2 one-shot, 2302–2307 cycles/µs across three runs, reported as `unknown` rather than guessed when implausible. This closes `REPORT-2026-07-27-01`'s first named next step — cycle-denominated evidence can now be scored against microsecond-denominated guardrails.

## The two bugs, because they matter more than the harness

**1. `kernel::sched::Scheduler::highest_priority_ready` could not execute on the real target binary.** It raised `#UD` → `#GP` → `#DF` → triple fault → silent QEMU shutdown. Cause: LLVM emits `movups`/`movaps` for ordinary 16-byte moves in entirely float-free code, SSE raises `#UD` while `CR4.OSFXSR` is clear, and nothing in this kernel's boot path had ever set it. The function passed all its host tests and no `EPIC-P0` QEMU fixture had ever called it, so nothing caught this until `EPIC-P1` started measuring on target. Fixed by enabling SSE in `hal_x86_64::boot`'s long-mode entry; the rejected alternative (compiling the kernel soft-float, as upstream `x86_64-unknown-none` does) and the consequences are recorded in [`ADR 0003`](../../docs/adr/0003-enable-sse-in-the-boot-path.md). Note that ADR 0003 also corrects ADR 0001's inaccurate "no SIMD in kernel context" rationale — the custom target spec never disabled SIMD.

**2. A fixture that switches into a task enables interrupts against an IDT that was never installed.** `Context::new` seeds a task's initial `rflags` with `IF` set; a measurement fixture deliberately installs no IDT (arming the APIC timer would inject ticks into the measured regions); a legacy IRQ0 accumulated during PIT calibration fired the instant `IF` went high. Mitigated in the fixture by retiring the legacy PIC first (`interrupts::remap_and_mask_pic`, made `pub` for exactly this — quiescing without arming). The underlying fail-open seam is now `LE-11`.

Both were found the same way: by running existing, host-verified code somewhere it had never run. That is the argument for `LE-09` in one sentence, and [Handover 03](03-le-09-arm64-pi5-slice-proposal.md) makes it at length.

## The result that shapes the next Story

Three consecutive Tier 0 runs give a run-to-run **p99 coefficient of variation of 39%–61%** for the small-operation metrics (pool alloc/free 39.11%, pool denial 47.85%, context switch 60.81%) against 1.2%–2.0% for the two larger dispatch metrics. The catalogue's own jitter guardrails want ≤5%.

**A timing gate that thresholds p99 or max on a single Tier 0 run would be noise** — failing green code, passing real regressions, and earning its own disablement. `STORY-P1-01-02` should gate on `min`/`p50` over medians of repeated runs, with Tier 0 tolerances derived from this measured noise rather than from the catalogue's hardware budgets, while still reporting the tails and saying plainly that they are not gated. Two inputs it also needs: measurement currently runs **dev-profile** binaries (`LE-13`), and no hardware tier exists (`LE-09`).

Nothing closes a guardrail. `REPORT-2026-07-27-02` scores D04/D05/D07 explicitly against `catalogue.tsv` — the closest is D04's context-switch round trip, which sits inside the single-switch p50 cycle budget in two of three runs even when the whole two-switch round trip is charged to one switch, and still does not close. `story-contracts.tsv`'s `STORY-P1-01-01` row moved `specified` → `baseline-debt`, never `verified`.

## Files touched

- New: `os/src/hal/src/time.rs`, `os/src/hal-x86_64/src/tsc.rs`, `os/src/kernel/src/measure.rs`, `os/src/kernel/src/fixture_measure.rs`, `os/src/xtask/src/timing.rs`.
- Changed: `.github/workflows/ci.yml` (two measurement steps in the QEMU job), `os/src/hal/src/lib.rs`, `os/src/hal-x86_64/src/lib.rs`, `os/src/hal-x86_64/src/boot.rs` (SSE enablement), `os/src/hal-x86_64/src/interrupts.rs` (`remap_and_mask_pic` made `pub`), `os/src/kernel/src/lib.rs`, `os/src/kernel/src/main.rs` (+ `fixture-measure` wiring), `os/src/kernel/Cargo.toml`, `os/src/kernel/src/fixture_pool_bench.rs` (refactored onto the harness), `os/src/xtask/src/main.rs` (`measure` command, optional serial capture, count assertions rebased), `os/src/xtask/src/assurance.rs` (Test 24→25, Report 30→31).
- New docs: `goals/tests/TEST-P1-01-01-A.md`, `goals/reports/REPORT-2026-07-27-02.md`, `goals/reports/_measure-p1-01-01/` (six raw captures), `docs/adr/0003-enable-sse-in-the-boot-path.md`, this file and [`03`](03-le-09-arm64-pi5-slice-proposal.md).
- Changed docs: `goals/stories/STORY-P1-01-01.md`, `goals/features/FEAT-P1-01.md`, `goals/assurance/story-contracts.tsv`, `goals/index.html`, `session/hand-2026-07-27/index.html`.

## Gates re-verified after every change

`cargo fmt --all -- --check` clean · `cargo test --workspace` 197 passed / 0 failed · clippy `-D warnings` clean on every host-buildable target *and* on the target-side `fixture-measure`/`fixture-pool-bench` builds · `check-assurance-spine`: 14 Features / 35 Stories / **25** Tests / **31** Reports / 1,500 Story-performance contracts / 6,575 application-performance contracts · `check-performance-catalogue` 625/625 · `check-crate-sizes` all far under 20,000 (largest: `kernel` 3,437) · `check-image-size` 18,024 bytes against the 8 MiB ceiling · full 12-fixture QEMU sweep after the `boot.rs` change with every documented exit code unchanged (`broken-boot` and `idt-apic-unrouted` still fail as their own tests require) · `xtask measure --runs=3` and `--runs=3 --fixture=pool-bench` both exit 0.

Two pre-existing local limitations, both CI-covered and neither introduced here: the `kernel` **bin** and `exec`'s fixture bins cannot be clippy'd or documented on a Windows host (`boot`/`qemu_exit`/`interrupts`/`pci` are `not(target_os = "windows")`-gated), and CI's own clippy never lints target-only fixture code at all — the latter is now `LE-12`.

## Loose-ends register (canonical as of this handover)

Per Handover 37's rule, carried forward with per-item status; four new items, one closed.

| ID | Loose end | Origin | Owner / fix path | Status |
|---|---|---|---|---|
| LE-01 | Priority-inheritance behavioral half never verified (host-only bookkeeping proof) | `STORY-P0-02-03` | `STORY-P1-04-01` acceptance criterion 2 | Open — owned |
| LE-02 | WCET enforcement has no timer and no watchdog behind it | `STORY-P0-02-04` | `STORY-P1-04-02` | Open — owned |
| LE-03 | No real `#PF`/`#GP`/`#UD` handling; every fault is terminal diverge-and-report | Handover 32 | `FEAT-P1-02` (`STORY-P1-02-01`) | Open — owned. **Sharpened today**: a `#UD` from missing SSE enablement cost this session a silent triple fault with no diagnostic whatsoever; `FEAT-P1-02` should also route `#XF` (vector 19) |
| LE-04 | No TSS/IST; a fault during fault handling triple-faults | Handover 32 | `STORY-P1-02-02` | Open — owned |
| LE-05 | `exec::AddressSpace` built but never installed; system runs all-RWX identity-mapped | `STORY-P0-05-02` | `FEAT-P1-03` | Open — owned |
| LE-06 | `pool-bench` was a divergent sibling harness exiting harness-error 2 | Handover 35 | `STORY-P1-01-01` | **Closed 2026-07-27** — the exit-2 symptom no longer reproduced (verified before any change this session; the concurrent session's stack/`memcmp` fixes had cleared it, and the entry was stale), and the divergent-sibling substance is gone: `fixture_pool_bench` now runs on `kernel::measure` with its private measurement code deleted, reporting through `xtask measure --fixture=pool-bench` |
| LE-07 | CI has never been observed running any of this work | Standing since Handover 07 | Phase-independent | **Closed 2026-07-27** (run `30226663769` on `f1d7c90` fully green) |
| LE-08 | I/O APIC device-IRQ routing deferred (local APIC only) | `STORY-P0-04-02`/`-03` | Whichever Story first routes a device IRQ | Open — deferred with trigger |
| LE-09 | Pi 5 (ARM64) is the short-term hardware, but no ARM64 bring-up slice or deploy path exists | Handover 37 directive 1 | Scoping + sequencing decision | Open — **decision needed; proposal now filed** ([Handover 03](03-le-09-arm64-pi5-slice-proposal.md)). Today's evidence strengthens the case: Tier 0 p99 noise of 39–61% means no tail claim can be settled under emulation at all |
| LE-10 | ECAM/MCFG config access and PCI bridge traversal deferred | `STORY-P0-04-03` | First Story needing extended config space | Open — deferred with trigger |
| **LE-11** | `Context::new` seeds task `rflags` with `IF` set, so switching into a task enables interrupts even on a boot path with no IDT installed — fail-open, and observed as a triple fault today | `STORY-P1-01-01` (this handover) | `FEAT-P1-02`: once real fault handling exists, either seed `IF` clear until a task is admitted or require an IDT before the first switch. Mitigated in fixtures today by masking the legacy PIC | Open — owned |
| **LE-12** | CI's clippy never lints target-only fixture code (`--all-targets` on the host builds no fixture feature), so fixture lint errors are invisible until someone runs target clippy by hand — `fixture-context-switch` currently fails `clippy::deref_addrof` that way | `STORY-P1-01-01` (this handover) | Add per-fixture target clippy to the CI lint job; cheap, and it makes the existing failure visible | Open — unowned, needs a Story |
| **LE-13** | Measurement runs **dev-profile** (unoptimized) binaries; no release-profile measurement path exists, so every absolute cycle count is inflated by missing optimization as well as by emulation | `STORY-P1-01-01` (this handover) | `STORY-P1-01-02` — baselines are worthless if they bake in the dev profile | Open — owned |
| **LE-14** | `context::switch` saves no SSE/x87 state, which became load-bearing the moment SSE was enabled: safe today (no task holds vector state across a yield, and compiler `xmm` use is intra-function) but unsound under preemption | `STORY-P1-01-01` / ADR 0003 | `FEAT-P1-04`, where a preempted task's register state is no longer the compiler's to reason about | Open — owned |

## Next session — start here

1. **`LE-09` decision** ([Handover 03](03-le-09-arm64-pi5-slice-proposal.md)) — A, B, or B-with-carve-out. This is a user decision, not a technical one, and the recommendation plus its reasoning is in that document.
2. **`STORY-P1-01-02`** — committed baselines and the `check-timing-regression` CI gate, demonstrated to fail on a deliberately-introduced regression. Design it against today's measured noise (gate `min`/`p50` over medians of repeated runs; do not gate Tier 0 tails), and fold in `LE-13` (release-profile measurement) plus the UART-borne pass/fail bit that any future hardware tier needs.
3. Then `FEAT-P1-02` (real exception handling), per the Epic's ordering — carrying `LE-03`'s sharpened note (route `#XF`) and `LE-11`.

## What this handover does not do

No gate exists yet — nothing fails a build on a timing regression. No guardrail is closed, no Story is `verified`, and no hardware has run any of this. `FEAT-P1-02`/`-03`/`-04`/`-05`/`-06` remain untouched, and their nine Stories remain `specified`.
