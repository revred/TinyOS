# STORY-P0-03-01 — Bounded-Capacity Pool Allocator Type

Status: **Verified** (locally; CI run pending)
Feature: [`FEAT-P0-03`](../features/FEAT-P0-03.md)
Introduced in: [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)

## Description

A `Pool<T, const N: usize>` type in `os/src/kernel/src/mem.rs`: fixed-capacity, statically-sized storage for up to `N` live values of `T`, with no heap allocation anywhere in its implementation — the RT-path primitive `agent/CODING_STANDARDS.md`'s "no heap allocation in any scheduler, IPC, or interrupt-handling hot path" rule requires. This Story covers the allocator type and its ordinary alloc/free path; exhaustion behavior is specified separately in `STORY-P0-03-03` (fail-closed on a full pool) but implemented in the same type since the two are inseparable in practice.

## Acceptance criteria

1. `Pool::<T, N>::new()` is a `const fn` (no runtime initialization cost, usable in a `static`) and requires no heap allocation — backing storage is `[MaybeUninit<T>; N]` plus an `N`-bit-or-smaller occupancy bitmap, not a `Vec`/`Box`.
2. `alloc(&mut self, value: T) -> Result<PoolHandle, PoolError>` claims the first free slot, moves `value` into it without dropping the caller's copy twice, and returns a handle that identifies that slot. `&mut self` is deliberate at this stage — no concurrent-access story exists yet (that's `FEAT-P0-02`'s scheduler work), so this Story does not invent an interior-mutability/locking scheme speculatively; a `&self`, synchronized API is a follow-on Story once real concurrent callers exist.
3. `free(&mut self, handle: PoolHandle) -> Result<T, PoolError>` returns ownership of the stored value to the caller and marks the slot free again; freeing an already-free or out-of-range handle returns `Err(PoolError::InvalidHandle)` rather than panicking (`panic!` is not error handling on an RT path, per `agent/CODING_STANDARDS.md`).
4. No `unsafe` block lacks a `// SAFETY:` comment stating the invariant that makes it sound, per the Unsafe code policy.
5. `cargo test -p kernel --lib` passes on the host target with no `unsafe` beyond what's needed for the `MaybeUninit` slot storage itself.

## Tests

- [`TEST-P0-03-01-A`](../tests/TEST-P0-03-01-A.md) — pool alloc/free round-trip and double-free/invalid-handle rejection (host unit test).

## Goals verified

G-RT-2 (deterministic memory behavior — no unbounded heap fragmentation or allocation-time variance).

## 2026-07-27 — `PERF-D07` assurance-evidence session

Functional correctness (this Story's own acceptance criteria above) was never in question going into this session; the gap was `PERF-D07`'s 23 release guardrails (`G01`-`G23`) and the mapped `SEC-03`/`SEC-19`/`SEC-20`/`BND-04`/`BND-15`/`BND-20` boundary evidence, all of which were still completely unattempted. This session built real measurement infrastructure and produced real evidence, then had it independently adversarially checked against `goals/performance/catalogue.tsv`'s actual numeric targets rather than just labeling data with guardrail IDs.

**What closed:**

- `PERF-D07-G11` (steady-state allocations = 0) — genuinely closed. `grep -rn "global_allocator\|GlobalAlloc" os/src/kernel/src/` finds zero matches: the kernel has no `#[global_allocator]` at all, so heap allocation is categorically impossible by construction, not just empirically absent this run.
- `PERF-D07-G08` (microarchitectural counters) and `PERF-D07-G19` (isolation under competing load) — correctly reasoned `N/A-debt`, not silently dropped: `G08` needs a vPMU this QEMU/TCG-on-Windows environment cannot provide (HIL or a Linux/KVM host would); `G19` needs concurrent-load scheduler infrastructure that does not exist yet.

**What remains open (the honest majority):**

- 14 further guardrails (`G01`-`G07`, `G10`, `G12`-`G14`, `G18`, `G20`, `G21`) now have real Host-tier and Tier-0-QEMU cycle-count/latency data on record — a new Host diagnostic harness in `os/src/kernel/src/mem.rs`'s test module, and a new real Tier-0 fixture (`os/src/kernel/src/fixture_pool_bench.rs`, `--fixture=pool-bench`, backed by a new `os/src/hal-x86_64/src/serial.rs` COM1 driver) — but that data does not close the guardrails: T0 cycle counts run 5-140x over several guardrails' numeric budgets, several latency guardrails are specified in microseconds with no documented TSC-frequency assumption to convert QEMU's emulated `RDTSC` cycles into real time, `G04`'s required WCET-margin argument was never written, `G05`'s own computed run-to-run CV (~6.6%) exceeds its own ≤5% target, and `G18` is mislabeled onto the wrong measurement phase in the fixture.
- 5 guardrails (`G09`, `G15`, `G16`, `G17`, `G23`) were not attempted at all this session.
- `PERF-D07-G22` (72-hour soak) was deliberately **not** run this session — see the linked session handover for how a separate soak was kicked off outside this workflow, and why that is infrastructure debt (a session-scoped scheduled job), not a durable guarantee.
- `SEC-19`'s Miri/sanitizer/fuzz/property requirement was not run — a real, named gap, not substituted by the existing `cargo test`/clippy suite.

**Assurance state:** `story-contracts.tsv`'s `STORY-P0-03-01` row stays `baseline-debt` — confirmed unchanged this session. It does not become `verified`: `G22` alone would block that regardless of anything else, and in fact the large majority of the other 22 guardrails are not closed either. See [`REPORT-2026-07-27-01`](../reports/REPORT-2026-07-27-01.md) for the full per-guardrail scorecard and [`session/hand-2026-07-27/01-story-p0-03-01-assurance-evidence.md`](../../session/hand-2026-07-27/01-story-p0-03-01-assurance-evidence.md) for the session handover.
