# TEST-P0-01-06-A — What Actually Blocks `D09`, and the Gates That Turn Out Not To Be Waiting on Hardware

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P0-01-06`](../stories/STORY-P0-01-06.md)
Tier: Host reasoning against the machine-readable catalogues, **plus** a Tier 0 QEMU measurement fixture for the `D09` work unit, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`, `BND-18`
Protection Domain contracts: `PD-02`, `PD-12`, `PD-13`, `PD-14`
Code admission gates: `RCG-05`, `RCG-06`, `RCG-07`, `RCG-12`, `RCG-14`
Assurance state: `baseline-debt`

Applicable guardrails: this Story records evidence for **other** Stories' guardrails and closes none of its own, exactly as [`STORY-P0-01-05`](../stories/STORY-P0-01-05.md) does. `D01` is selected because the deliverable is tooling and a register entry, not a measured subsystem.

## What this test is for

`LE-31` says the project-wide `0 / 57 Stories assurance-verified` is attributed to `LE-09` — no hardware tier — and that the attribution is wrong for nine Stories. [Handover 28](../../session/hand-2026-07-28/28-analysis-response-and-le-33.md)'s first pass narrowed the field to one candidate needing no hardware purchase at all: **`STORY-P0-05-01`, which selects `D09` alone, and whose 25 release gates are every one of them `Host+T0`.**

"`Host+T0`" is the catalogue asserting these gates need no hardware this project lacks. That assertion has never been checked against what the tree and the dev loop can actually produce. This test checks it, gate by gate, and it is written to be **falsifiable in the unflattering direction**: the expected outcome is that some gates close, most do not, and the *reasons* they do not are more specific than "no hardware".

The reason that matters more than the count: a wrong belief about what blocks `verified` compounds in every later session that plans around it.

## Specification

### 1. Every one of `D09`'s 25 gates is dispositioned, with a named blocker

**Given** the 25 `PERF-D09-*` rows in [`goals/performance/catalogue.tsv`](../performance/catalogue.tsv),
**then** each one is assigned exactly one disposition — `closeable-now`, `blocked-on-tooling`, `blocked-on-environment`, `blocked-on-hardware`, or `blocked-on-subsystem` — and every non-`closeable-now` disposition names *what specifically is missing*.

**And** "no hardware tier" is not an acceptable blocker for any `Host+T0` gate unless the gate's own metric demands a device this project does not have. A blocker that restates `LE-09` for a gate whose tier column says otherwise is the exact error this Story exists to correct.

### 2. The `D09` work unit is measured on the tier its catalogue row names

**Given** `exec::pe::parse` — the PE64 load-and-import-validation path `D09` is stated against,
**when** it runs inside the real `x86_64-tinyos` target binary under QEMU,
**then** a `TOS64-MEAS/1` envelope reports its cycle distribution, through the **shared** [`kernel::measure`] harness rather than a second copy of one.

**And** both the accept path and the **denial** path are measured. `D09`'s `G20` is stated against malformed input, and a domain measured only on well-formed input has not been measured on the input that matters — the same reason `fixture_measure` measures `pool_alloc_denied_exhausted` separately from the happy path.

### 3. Measuring `D09` does not perturb the gated baseline (`LE-23`, `LE-28`)

**Given** the timing gate's rule that a measured-but-unbaselined metric is `MetricNotBaselined` rather than a silent skip,
**then** this Story's measurement runs in a fixture **outside** the gated `measure` envelope — the `fixture-pool-bench` precedent — and the committed baseline in [`goals/performance/baselines/`](../performance/baselines/) is **not** rewritten.

**And** this is not a convenience. Re-recording the baseline on the dev host would bake in the confirmed 23–53% Windows-versus-Linux offset (`LE-23`) and is one command from turning that offset into a false green (`LE-28`). A Story that produced `D09` evidence by corrupting every *other* domain's baseline would be a net loss.

### 4. A gate is recorded as evidenced only when the evidence satisfies the whole gate

**Given** a gate whose target names several conditions,
**then** a row is filed in [`goals/assurance/guardrail-evidence.tsv`](../assurance/guardrail-evidence.tsv) **only if every condition is met**, and a gate whose evidence covers part of its target is left absent with its remainder named in the disposition table.

**And** this follows `STORY-P0-01-05`'s own rule directly: the register is a count of evidence, never a score, and a gate absent from it is unevidenced rather than failed. Filing a half-met gate would be the "cheapest lie available" that Story explicitly refused.

### 5. The run-to-run stability the catalogue asks for is checked, not assumed

**Given** `PERF-D09-G05`'s requirement of run-to-run p99 CV ≤ 5%,
**then** the observed CV for this domain's own metrics is computed across at least three runs and **reported whether or not it passes**.

**And** if it does not pass, that is recorded as an environment finding rather than a measurement failure: the existing `measure` fixture's own metrics have been observed between 1.48% and 81.38% p99 CV on the dev host, so a gate demanding ≤ 5% is making a claim about the *measurement environment* before it makes one about the code.

### 6. What this test explicitly does **not** establish

- **No `STORY-P0-05-01` state change is promised.** If the audit shows the Story cannot reach assurance `verified`, it does not, and the count stays at `0 / 57`. The deliverable is a correct disposition, not a moved number.
- **No hardware claim.** Every number here is Tier 0 QEMU/TCG, and `LE-09` stays open regardless of outcome.
- **No new gate, no relaxed target.** The 25 `D09` rows are not this Story's to edit.
- **No other domain audited.** `LE-31` names nine Stories; this is the one Handover 28 identified as the sole no-hardware candidate.

## Test type

Host reasoning against the catalogues (recorded as a disposition table), plus a Tier 0 QEMU measurement fixture and host unit tests for anything it computes.

## Implementation location

- `os/src/kernel/src/fixture_pe_bench.rs` — the `D09` work-unit fixture.
- `os/src/xtask/` — fixture registration.
- `goals/assurance/guardrail-evidence.tsv` — the rows this Story files.

## What was and was not run, 2026-07-28

**No clause was edited to fit what happened.** The specification above is the one written before implementation.

### The measurement

`cargo run -p xtask -- measure --fixture=pe-measure`, three runs, `x86_64-tinyos` under QEMU, against the real `blue-sharc.txe` (8,269,824 bytes) with its real import table. `cycles_per_us=2310`.

| Metric | p50 | p99 | max |
|---|---|---|---|
| `pe_parse_blue_sharc_accept` | 4,510,818 cy — **1,952.7 µs** | 8,796,646 cy — 3,808.1 µs | 12,784,433 cy — 5,534.4 µs |
| `pe_parse_denied_truncated` | 19,792 cy — 8.6 µs | 28,776 cy — 12.5 µs | 104,298 cy — **45.2 µs** |

Run-to-run p99 CV: **22.13%** (accept), **71.22%** (denial).

### The disposition (clause 1): 25 gates, 25 named blockers

| Gate | Target | Disposition | Blocker |
|---|---|---|---|
| `G01` | p50 ≤ 50 µs | not closeable | **blocked-on-environment** — measured 1,952.7 µs, **39.1× over**; see the finding below |
| `G02` | p99 ≤ 100 µs | not closeable | blocked-on-environment — measured 3,808.1 µs |
| `G03` | p99.9 ≤ 200 µs | not closeable | blocked-on-environment — measured 3,877.2 µs |
| `G04` | max ≤ 500 µs, ≥20% margin | not closeable | blocked-on-environment — measured 5,534.4 µs |
| `G05` | p99−p50 ≤ 50 µs **and** run-to-run p99 CV ≤ 5% | **fails outright** | blocked-on-environment — CV is 22.13%/71.22%; see clause 5 |
| `G06` | p50 ≤ 150,000 cycles | not closeable | blocked-on-environment — measured 4,510,818, **30.1× over** |
| `G07` | p99 ≤ 500,000 cycles | not closeable | blocked-on-environment — measured 8,796,646, **17.6× over** |
| `G08` | retired instructions, branch/L1D miss rates | not closeable | **blocked-on-hardware** — QEMU/TCG exposes no PMU; needs real silicon, which is `LE-09`'s only legitimate appearance in this table |
| `G09` | feature code+rodata delta ≤ 128 KiB | closeable, not done | **blocked-on-tooling** — no per-feature section-size delta tool exists |
| `G10` | peak OS-owned working memory ≤ 256 KiB | closeable, not done | blocked-on-tooling — same missing footprint tool |
| `G11` | heap allocations per work unit = 0 | **already evidenced** | closed by `STORY-P0-01-05`, structurally |
| `G12` | setup/pool allocation max ≤ 10 µs, bounded reclamation | **closeable now** | none — `pe::parse` performs no allocation of any kind; this is the cheapest remaining gate and is left unfiled only because clause 4 forbids filing what this Story did not rigorously verify |
| `G13` | enqueue-to-service p99 ≤ 100 µs | not closeable | **blocked-on-subsystem** — the load path has no queue; absence must be *tested*, not inferred |
| `G14` | service-start-to-completion max ≤ 250 µs | not closeable | blocked-on-subsystem — same |
| `G15` | ≥ 2000 work units/s | not closeable | blocked-on-environment — observed ~512/s at Tier 0, and that figure inherits `G01`'s problem |
| `G16` | 2× throughput floor for 10 s, bounded queue | not closeable | blocked-on-subsystem — no queue, no burst harness |
| `G17` | cold start ≤ 10 ms | not closeable | blocked-on-tooling — "cold start" is undefined for a parser invoked as a library call |
| `G18` | warm restart ≤ 500 µs | not closeable | blocked-on-tooling — same |
| `G19` | p99 degradation ≤ 5% under load | not closeable | blocked-on-tooling — no competing-load harness exists; this is the `G19` theme Handover 29 trap 4 names |
| `G20` | denied work ≤ 125 µs; state changes = 0; allocations = 0 | **CLOSES** | none — all three conditions met; see below |
| `G21` | fault decision and containment ≤ 500 µs | not closeable | blocked-on-subsystem — the parse path emits no fault; this is `D02`-shaped work |
| `G22` | 72 h soak | not closeable | blocked-on-environment — no soak infrastructure |
| `G23` | spoor overhead ≤ 2% | not closeable | blocked-on-subsystem — the parse path emits no spoor |
| `G24` | ≤ 0.5× Linux PREEMPT_RT p99 | not closeable | blocked-on-tooling — needs external baselines; cadence is `claim`, not `release` |
| `G25` | ≥ 10× vs 3 RTOS baselines | not closeable | blocked-on-tooling — same |

**One gate closes.** `LE-09` — no hardware tier — is the correct blocker for **exactly one** of the twenty-five (`G08`). That is the finding `LE-31` predicted and this table now evidences: the project-wide attribution of the assurance-verified zero to `LE-09` is wrong for `D09` in twenty-four cases out of twenty-five.

### `G20` closes on all three of its conditions (clause 4)

| Condition | Evidence |
|---|---|
| denied work ≤ 125 µs | max over 600 samples across three runs: 104,298 cycles = **45.2 µs**, a 2.8× margin |
| state changes = 0 | `pe::parse` takes `&[u8]` and returns `Result` by value, mutating nothing reachable by a caller; the fixture asserts the *same* `PeError` on all 200 iterations, so the denial is deterministic rather than merely repeated |
| allocations = 0 | no heap exists in any shipped crate — the compiler-enforced property `STORY-P0-01-05` recorded for `G11`, which this path inherits |

Filed as one row in [`guardrail-evidence.tsv`](../assurance/guardrail-evidence.tsv). No other row was filed, per clause 4.

### The finding this Story did not go looking for

**The accept path is 17.6–39.1× over every latency and cycle budget `D09` states**, on the only measurement that has ever been taken of it.

What that does **not** establish, stated plainly so nobody promotes it:

- These are Tier 0 QEMU/TCG figures. TCG does not model cache, and its timing is not proportional to hardware, so the *magnitude* is not transferable.
- The run-to-run p99 CV of 22–71% means **no stable verdict exists in this environment at all** — which is `G05` failing, and `G05` failing is the reason the other six cannot be evaluated rather than a separate problem.

What it **does** establish: a 30× overshoot is far outside the range that measurement noise plausibly explains, and nobody had looked. Registered as `LE-42` rather than left in this document, per Handover 29's own closing point that a finding which stays in prose stops being read.

### Clause 3 held: the gated baseline was not touched

The fixture is `pe-measure`, registered in `MEASURABLE_FIXTURES` but outside the gated `measure` envelope, following `fixture-pool-bench`. `goals/performance/baselines/tier0-x86_64.tsv` is unmodified, and `--update-baseline` was never run (`LE-28`).

This was not a stylistic choice. The gate refuses a measured-but-unbaselined metric with `GateError::MetricNotBaselined`, so putting `D09` in the gated envelope forces a same-commit baseline re-record — taken on a Windows host, that bakes in the confirmed 23–53% cross-host offset (`LE-23`). **`D09`'s measured evidence was blocked by baseline provenance, not by hardware**, which is itself an `LE-31` finding worth more than the numbers.

## Reports

[`REPORT-2026-07-28-09`](../reports/REPORT-2026-07-28-09.md) — the run, the disposition, and what it does not license.
