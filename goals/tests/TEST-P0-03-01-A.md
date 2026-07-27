# TEST-P0-03-01-A — Pool Alloc/Free Round-Trip and Invalid-Handle Rejection

Status: **Functionally Verified — passing locally, 2026-07-26.** `PERF-D07` release-guardrail evidence attached 2026-07-27 (`REPORT-2026-07-27-01`): 1 of 23 non-Claim guardrails (`G01`-`G23`) closed with passing Host+T0 evidence (`G11`), 2 correctly `N/A-debt` (`G08`, `G19`), `G22` (72-hour soak) still pending — see Reports below for the full per-guardrail scorecard. Assurance state remains `baseline-debt`, not `verified`.
Story: [`STORY-P0-03-01`](../stories/STORY-P0-03-01.md)
Tier: Host unit test (no QEMU/hardware dependency — pure allocator logic), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D07`
Security controls: `SEC-03`, `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`, `C3`, `C4`
Boundary tests: `BND-04`, `BND-15`, `BND-20`
Protection Domain contracts: `PD-01`, `PD-06`, `PD-08`, `PD-13`
Code admission gates: `RCG-09`, `RCG-10`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

Per the TDD mandate, this specification is written before the allocator code it verifies.

**Given** a `Pool<u32, 4>`,
**when** a value is `alloc`'d,
**then** the returned handle can be `free`'d to recover the exact value that was stored, the slot becomes reusable by a subsequent `alloc`, and:
- freeing the same handle twice returns `Err(PoolError::InvalidHandle)` on the second call, not a panic or a stale/aliased value,
- an out-of-range handle (constructed from an index the pool never issued) is rejected the same way.

## Test type

Host unit test (`#[cfg(test)]` in `os/src/kernel/src/mem.rs`, run via `cargo test -p kernel --lib`) — no QEMU dependency because the allocator's correctness doesn't depend on the target's boot environment, only on `core` semantics common to host and `no_std` builds alike.

## Implementation location

`os/src/kernel/src/mem.rs` (`Pool`, `PoolHandle`, `PoolError`).

## Reports

- [`REPORT-2026-07-26-04`](../reports/REPORT-2026-07-26-04.md) — Pass (local, host unit test).
- [`REPORT-2026-07-27-01`](../reports/REPORT-2026-07-27-01.md) — `PERF-D07-G01`..`G23` release-guardrail evidence pass (Host + Tier 0 QEMU), plus mapped `SEC-03`/`SEC-19`/`SEC-20` and `BND-04`/`BND-15`/`BND-20` boundary evidence. Real Host diagnostics added to `os/src/kernel/src/mem.rs`'s test module, plus a new Tier 0 fixture (`os/src/kernel/src/fixture_pool_bench.rs`, `--fixture=pool-bench`) and a new COM1 serial driver (`os/src/hal-x86_64/src/serial.rs`). Per-guardrail scorecard against `goals/performance/catalogue.tsv`'s actual numeric targets (not just ID-tagging): `G11` closed; `G08`/`G19` correctly `N/A-debt`; `G09`/`G15`/`G16`/`G17`/`G23` not attempted; `G01`-`G07`, `G10`, `G12`-`G14`, `G18`, `G20`, `G21` have real evidence gathered but do not close against their numeric thresholds (mostly: T0 cycle counts are 5-140x over budget with no documented TSC-frequency conversion to the specified us units, or the guardrail's own required WCET-margin/multi-boot/A-B-instrumentation work was not done); `G22` (72-hour soak) deliberately not run this session, tracked separately. An independent adversarial verification pass confirmed no fabricated numbers anywhere but found the three earlier sub-reports never checked their own data against `catalogue.tsv`'s thresholds — this Report and Test-doc update correct that.
