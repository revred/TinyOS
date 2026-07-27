# TEST-P1-07-03-A — Caches Are Actually On, and the Proof Is a Difference

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P1-07-03`](../stories/STORY-P1-07-03.md)
Tier: Host unit tests (descriptor construction, `MAIR_EL1`/`TCR_EL1` field encoding, table walk arithmetic) **plus** a Tier 1 hardware run on a Raspberry Pi 5 with a before-and-after measured loop, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D08`
Security controls: `SEC-03`, `SEC-19`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: `PERF-D08-G01`, `PERF-D08-G04`, `PERF-D08-G10` — translation latency, its bound, and paging working memory. None is closed here; this Story establishes translation, not address spaces.

## What this test is for

With `SCTLR_EL1.M == 0`, AArch64 treats every data access as **Device-nGnRnE** regardless of what the memory actually is: uncached, unbuffered, no speculation, caches architecturally not consulted. A dispatch path measured in that state produces a number dominated by DRAM round-trips. It is not slow-but-proportional. It is meaningless.

This is the single most likely way for this whole Feature to produce **confidently wrong numbers** — numbers that parse, that look plausible, that get quoted, and that describe nothing. A flat identity map with Normal cacheable attributes is therefore a prerequisite of measurement, and this test is written around the one clause that can tell the difference between doing it and appearing to.

## Specification

### 1. The tables are built by pure, host-tested code (`SEC-19`)

**Given** a description of RAM and the UART MMIO region,
**then** the page-table descriptors, the `MAIR_EL1` attribute indices, and the `TCR_EL1` granule/address-size/shareability fields are produced by pure functions with host unit tests, and the only `unsafe` on the board is the system-register writes themselves.

**And** descriptor construction is arithmetic. Arithmetic belongs on the dev host where it can be tested exhaustively, not on a board whose only feedback channel is a serial line.

### 2. Attributes are per-region and explicit

**Given** the identity map,
**then** RAM is Normal Write-Back Cacheable, Inner-Shareable, and the UART MMIO region is Device-nGnRnE, each mapped **explicitly** rather than left to a blanket attribute.

**And** the map covers RAM and the UART MMIO and nothing else. An over-broad map is not more convenient here; it is the thing that makes a wrong attribute invisible.

### 3. The transition is ordered

**Given** the switch,
**then** `TTBR0_EL1`, `MAIR_EL1` and `TCR_EL1` are written, the TLB is invalidated, the required `dsb`/`isb` barriers are issued, and only then are `SCTLR_EL1.M`, `.C` and `.I` set.

**And** a missing barrier here does not fail loudly — it fails intermittently, later, in someone else's Story. The ordering is asserted by review and stated here because it is unobservable from any output this board produces.

### 4. **Acceptance requires evidence that caches are actually on**

**Given** the same measured loop,
**when** it runs before the MMU is enabled and again after,
**then** the two captures show the expected order-of-magnitude difference, and **both are quoted verbatim in this document**.

**And this clause is the Story.** A write to `SCTLR_EL1` that is silently ignored, a `MAIR_EL1` index that points at the wrong attribute, and a fully correct configuration are indistinguishable in every other respect — same boot, same UART, same absence of faults. The only signal that separates them is that the cached case is dramatically faster. Without this clause the Story cannot distinguish success from a silently-ignored write, and every number the Feature later produces inherits that ambiguity.

**And** the loop is chosen to be memory-bound, so that the difference it reports is about the cache and not about the pipeline.

### 5. The UART survives the switch

**Given** the moment the MMU is enabled,
**then** the UART continues to work, demonstrated by output emitted after the switch.

**And** if the UART goes silent exactly at the switch, the device-region attributes are wrong. That is a *diagnosable* outcome, and it is the reason the MMIO region is mapped explicitly: a silent board with no hypothesis is the failure this Feature's ordering exists to prevent.

### 6. A deliberate translation fault closes the loop with `-02`

**Given** an access to an unmapped address,
**then** `STORY-P1-07-02`'s handler reports it with a decoded `ESR_EL1` naming the data-abort exception class and a `FAR_EL1` matching the address accessed.

**And** this is the proof that the fault path survived the memory-system change that most easily breaks it — a vector table that worked with the MMU off and stopped working with it on is a real and common bring-up failure, and nothing else in this Feature would notice.

### 7. What this test explicitly does **not** establish

- **No per-task address spaces, no W^X, no teardown, no generation-safe reuse.** `SEC-03` is selected because this Story establishes translation, and its scope stops there. **Nothing here may be cited as isolation evidence** — that is `FEAT-P1-03`'s port, a follow-on Feature with its own adversarial obligations.
- **No `EL0`, no `TTBR1_EL1`, no per-task `TTBR0`.**
- **No measurement.** Clause 4's loop is a cache detector, not a benchmark, and its numbers are not baselines and must not be recorded as any.
- **`LE-09` stays open.**

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`) plus a Tier 1 hardware run with paired captures.

## Implementation location

- `os/src/hal-arm64/` — descriptor construction, `MAIR_EL1`/`TCR_EL1`/`TTBR0_EL1` programming, the `SCTLR_EL1` enable sequence.
- `os/src/kernel/` — the before/after cache-evidence fixture.

## Reports

To be filed when the Story goes Green.
