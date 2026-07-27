# TEST-P1-01-04-A — A Timing Gate Whose Verdict Is About the Code, Not About the Runner

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P1-01-04`](../stories/STORY-P1-01-04.md)
Tier: Host unit tests (ratio arithmetic, scale-invariance, baseline parsing, fail-closed cases — no QEMU dependency) **plus** Tier 0 QEMU runs of the real measurement fixture, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D02`, `D04`, `D05`, `D07`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-15`, `BND-16`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-08`
Code admission gates: `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

`STORY-P1-01-02` built a gate that fails closed on every malformed shape and has been seen to fail on a real injected regression. Everything it established stays true and none of it is re-litigated here. What it did **not** establish is that a green verdict means anything, and five sessions of red CI have now shown that it does not.

The evidence, from this project's own CI (Handover 10, runs `30274004446` and `30274317558`):

| Metric | run A p50 | run B p50 | ratio |
|---|---|---|---|
| `D07/pool_u64x64_alloc_free_round_trip` | 12 | 26 | 2.17x |
| `D07/pool_u64x4_alloc_denied_exhausted` | 22 | 26 | 1.18x |
| `D04/context_switch_yield_roundtrip_2switches` | 168 | 312 | 1.86x |
| `D05/dispatch_select_highest_priority_ready` | 90 | **182** | **2.02x** |
| `D05/dispatch_run_once_cooperative_round` | 200 | 364 | 1.82x |

**Run B's commit changed one documentation file and not one byte of code.** The binaries were identical. Every metric moved together by 1.8–2.2x, `D05/dispatch_select` crossed its limit, and the gate reported `REGRESSED` about code that had not changed. That is `LE-16` (the gate detects only ~1.6x-or-worse) meeting `LE-18` (it is host-condition-sensitive) on a metric whose CI value simply happens to sit on the boundary.

The finding that shapes this Story's design is in the last column: **the noise is global.** Nothing about `D05/dispatch_select` is unstable; it is the metric with the least headroom, so it trips first. A quantity measured *between two metrics in the same run* is therefore far more stable than either metric's absolute value — and a genuine regression in one operation moves that quantity, while a slow runner does not.

This project has already solved this problem once and the precedent is the answer. `kernel::fixture_idt_apic_timer` gates on `MAX_INTERVAL_RATIO`, *"a self-consistency bound rather than a fixed microsecond figure, since QEMU's own APIC-timer-to-wall-clock relationship under software emulation is not itself a stable absolute number this fixture should depend on."* That reasoning applies verbatim to this gate, which never got the memo.

**The hazard this document exists to guard against.** A gate made noise-tolerant is worthless if it has also been made blind. Every clause below that loosens something is paired with a clause that pins what must still be caught, and clause 7 is the whole-path demonstration: the real injected regression, built and booted, must still fail the gate after this change.

## Specification

### 1. A reference metric that no gated code path can move

**Given** the measurement fixture,
**then** it measures one additional metric whose workload is a **fixed integer computation** touching no scheduler, no pool, no context switch, no fault path and no allocation — nothing this project's kernel code can change.

**And** it is measured through exactly the same path as every gated metric: the same `Stopwatch`, the same `Calibration`, the same `Samples` buffer, the same `summarize`, the same envelope. A reference measured differently from the metrics it normalises would import its own systematic error into every ratio.

**And** its cost sits within the range spanned by the gated metrics rather than orders of magnitude away from any of them, so no ratio is dominated by the quantisation of a value close to zero.

**And** the reason it must never be edited is stated at its definition, in the source, not only here.

### 2. The gated quantity is a same-run ratio, not an absolute cycle count

**Given** a run measuring reference value `r` and metric value `m` for the same statistic (`min` against `min`, `p50` against `p50`),
**then** the quantity compared against the baseline is `m / r`, carried as an integer in **parts per million** (`m * 1_000_000 / r`) — this workspace has no floating point in a gate path and gains nothing by acquiring one.

**And** the ratio is formed **per run and then medianed across runs**, never as a ratio of two medians. The per-run ratio is the quantity from which the noise cancels; medianing first would re-admit the noise the design exists to remove.

**And** the reference metric is **not itself ratio-gated** — its ratio to itself is 1,000,000 by construction and gating it would be a tautology dressed as a check. Clause 5 states what guards it instead.

### 3. Scale-invariance, stated as arithmetic and pinned by test

**Given** a set of runs, and a second set formed by multiplying **every** metric in every run, including the reference, by a common factor `k`,
**then** every ratio comparison the gate produces is **identical** between the two sets — same observed ppm, same limit, same verdict.

This is the design's central claim and it is exactly testable without QEMU. A uniformly slower runner is precisely a common factor `k`, so a gate with this property cannot report a regression because CI was busy.

### 4. Tolerance derived from measurement, with a pre-committed acceptance bound

**Given** a set of at least five release-profile Tier 0 runs of identical binaries, deliberately spanning at least a **1.5x range in the reference metric** (obtained by loading the host, which is what CI's variability actually is),
**then** the spread of each metric's **ratio** across those runs must be at least **3x tighter** than the spread of that metric's **absolute** value across the same runs, measured as (max − min) / median for each.

**This bound is committed before the measurement is taken and before any tolerance constant is chosen.** If it does not hold, the design is wrong and no constant rescues it: normalising against a reference that does not actually track the runner would be a more elaborate way of measuring nothing. The measured figures are recorded in this Story's Report.

**And** the committed ratio tolerance is then set to clear the measured ratio spread with margin, in the same relative-percent-plus-absolute-floor shape `STORY-P1-01-02` already established, with the floor denominated in ppm. Its derivation is stated at the constant, as `TIER0_TOLERANCE`'s already is.

**And** the resulting sensitivity — the smallest regression this gate can now catch — is stated plainly in the gate's own documentation and in this Story's Report, whatever it turns out to be. `LE-16` is not closed by being made less visible.

### 5. The reference is guarded structurally, not by a tight threshold

**Given** the reference metric,
**then** it is gated on its **absolute** value against a band deliberately wider than the full run-to-run swing this project has recorded — at least **3x** the baseline, against a recorded swing of 2.2x.

**And** the purpose of that band is stated as what it is: a tripwire for *the reference having stopped being the reference* — someone editing the loop, the optimiser deleting it, a toolchain change — and explicitly **not** a regression detector. A band that tight would be `LE-18` all over again, on the one metric every other verdict now depends on.

**And** a reference metric that is absent from a run, or that measures zero, is a **harness error** and never a pass. Every ratio in the run divides by it.

### 6. Absolutes are still reported, and still labelled

**Given** the gate's output,
**then** every metric's absolute `min` and `p50` are printed alongside the ratio verdict, explicitly labelled as reported-and-not-gated — the same discipline the tails already have, for the same reason: a reader must never mistake an ungated number for a passing one.

**And** the reference metric's own absolute value is printed on every run, because it is now the direct measure of how fast the runner was, and reading it is how anyone diagnoses this gate in future.

**And** everything `STORY-P1-01-02` established about fail-closed parsing continues to hold unchanged: wrong header, wrong field count, non-numeric column, `min > p50`, `runs = 0`, empty field, duplicate key, no rows, missing file, provenance mismatch, a baselined metric not measured, a measured metric not baselined, fewer than three runs. The two new ratio columns join that set — non-numeric or absent is an error, never a default.

### 7. The gate has still been *seen* to fail, after the change

**Given** the deliberately-introduced regression that already exists (`--inject-regression`, seven extra selections inside the D05 timed region, ~8x),
**when** the gate runs against the new ratio baseline,
**then** it exits 1 and names `D05/dispatch_select_highest_priority_ready`.

**And** the demonstration is a real build-boot-measure-compare, not a doctored baseline file — `STORY-P1-01-02` clause 7's discipline, unchanged.

**And** additionally: a **synthetic** regression at the new tolerance's own boundary is pinned by host test, so the sensitivity claimed in clause 4 is a tested number rather than an asserted one.

### 8. The committed baseline is re-recorded, and why that is not baseline-laundering

**Given** that the existing baselines are, by Handover 10's Finding 2, **not mutually consistent** — two metrics report `improved (is the baseline stale?)` by 3–6x in the same run in which `D05/dispatch_select`'s baseline is tighter than anything the runner achieves —
**then** the baseline is re-recorded in a single coherent run set as part of this Story.

**And** this is admissible **only** because the methodology is fixed first and the inconsistency is independently evidenced. Handover 05's rule stands: re-recording a baseline to make a failing gate green destroys the signal in exchange for suppressing the symptom. Re-recording without clause 2 would produce a gate that is green today and exactly as uninformative as it is now. The order is not negotiable and this document is the record that it was followed.

**And** `LE-19(b)` is discharged as a prerequisite or explicitly carried: `--update-baseline` rewrites every measured row, so it cannot refresh one metric without silently re-recording the rest.

### 9. What this test explicitly does **not** establish

- **No hardware tier.** Every number here is Tier 0 QEMU/TCG. `LE-09` stays open, and a ratio gate is not a microsecond guardrail — it closes no `PERF-D*` budget.
- **No defence against a uniform slowdown of everything.** A change that slowed the reference loop and every gated path by the same factor would pass. The reference is a fixed integer computation that no kernel change can touch, so the realistic shape of that hazard is a toolchain or codegen-flag change, and clause 5's band is what watches for it. This limitation is deliberate, is the price of clause 3, and is stated rather than hidden.
- **No claim that ratios are budgets.** A committed ratio records the shape of this code under emulation today. It is not a WCET and not a contract with a caller.
- **`LE-16` is not closed.** The gate's sensitivity is restated in the new units, not eliminated. What changes is that the number now means something.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/xtask/src/gate.rs`) for ratio arithmetic, scale-invariance under a common factor, the boundary-sensitivity case, reference-absence and reference-zero, and every fail-closed case extended to the new columns; plus Tier 0 QEMU runs of `fixture-measure` through `cargo run -p xtask -- check-timing-regression`, idle and under deliberate host load, and the injected-regression demonstration.

## Implementation location

- `os/src/kernel/src/fixture_measure.rs` — the reference phase.
- `os/src/xtask/src/gate.rs` — the ratio model, the new baseline columns, the tolerance constants and their derivation.
- `os/src/xtask/src/main.rs` — the gate's reporting.
- `goals/performance/baselines/tier0-x86_64.tsv` — the re-recorded baseline.
- `.github/workflows/ci.yml` — the step's comment, which currently describes the tolerance model this Story replaces.

## Reports

- [`REPORT-2026-07-28-06`](../reports/REPORT-2026-07-28-06.md) — the Red run, the calibration measurement against clause 4's pre-committed bound, the tolerance derivation, the re-recorded baseline, and the injected regression still caught.
