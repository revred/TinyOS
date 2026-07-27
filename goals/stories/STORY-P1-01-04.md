# STORY-P1-01-04 — A Timing Gate Whose Verdict Is About the Code

Status: **Functionally Verified (Tier 0 + Host), 2026-07-28** — assurance state `baseline-debt`; no `PERF-D*` guardrail closed, hardware-tier timing evidence still outstanding (`LE-09`)
Feature: [`FEAT-P1-01`](../features/FEAT-P1-01.md)
Introduced in: [`session/hand-2026-07-28/10-next-session-mandate.md`](../../session/hand-2026-07-28/10-next-session-mandate.md)
Implemented in: [`session/hand-2026-07-28/15-story-p1-01-04-ratio-timing-gate.md`](../../session/hand-2026-07-28/15-story-p1-01-04-ratio-timing-gate.md)

## Description

`STORY-P1-01-02` built a gate that fails closed on every malformed shape and has been demonstrated to fail on a real injected regression. What it never established is that a **green** verdict means anything, and five sessions of red CI on `main` showed that it did not: between two runs of *identical binaries* — the second commit changed one markdown file — every gated metric moved together by 1.8–2.2x, and the metric with the least headroom crossed its limit and reported `REGRESSED` about code that had not changed.

This Story replaces the gated quantity. Instead of absolute cycle counts it compares each metric's **same-run ratio to a fixed reference workload**, formed per run and then medianed, so a slower runner cancels out of the verdict and a real regression does not. `LE-16` and `LE-18` are the register entries this addresses; neither is closed, and §"The finding" says why.

## Depends on

`STORY-P1-01-01`, `STORY-P1-01-02`.

## Acceptance criteria (final)

1. **The gated quantity is a same-run ratio, and a uniformly slower runner cannot change a verdict.** **Met**: `kernel::fixture_measure::phase_reference_loop` measures a fixed integer computation touching no scheduler, pool, context switch, fault path or allocation, through the same `Stopwatch`/`Calibration`/`Samples`/`summarize` path as every gated metric. `gate::check_against_baseline` compares `metric / reference` in ppm, formed **per run and then medianed** — not a ratio of two medians, which the tests pin as a distinguishable case. Scale-invariance is pinned by test at factors 2, 7 and 23: every ratio comparison is byte-identical.

2. **Demonstrated on the real fixture, in all four quadrants.** **Met**, and this is the Story's evidence:

   | Host | Code | Reference p50 | Verdict |
   |---|---|---|---|
   | quiet | unchanged | 532 (baseline 572) | exit 0 |
   | **loaded (2.24x slower)** | unchanged | **1284** | **exit 0** |
   | quiet | `--inject-regression` | 678 | exit 1, names `D05/dispatch_select` |
   | **loaded (2.15x slower)** | `--inject-regression` | **1230** | **exit 1, names `D05/dispatch_select`** |

   Row 2 is the failure that has been red on `main` since `91c95c1`. Row 4 is the one that proves the gate was not merely made blind: the observed ratio under load (2,205,314 ppm) is within 1% of the ratio measured quiet (2,223,255 ppm), against a limit of 395,522 — the ratio recovered the same signal under both conditions.

3. **The tolerance is derived from measurement, and the pre-committed acceptance bound is reported whether or not it held.** **Partly met, and the shortfall is recorded rather than smoothed over.** `TEST-P1-01-04-A` clause 4 committed, before any measurement, that ratio spread must be **≥3x tighter** than absolute spread. Measured across six simulated `--runs=3` invocations spanning a 2.02x reference swing, the improvement is **1.41x–2.28x** — the bound **failed**. What the same data showed is that the practical excursion a quiet-recorded baseline must absorb on a loaded host is **+62% worst case**, against absolute swings of up to 4.18x, so a 100% tolerance contains every measured excursion with 38 points of margin. See [`REPORT-2026-07-28-06`](../reports/REPORT-2026-07-28-06.md) for both tables.

4. **A metric the measurement cannot support carries no verdict, and says so.** **Met**: `gate::UNGATED_AT_TIER0` holds `D07/pool_u64x64_alloc_free_round_trip`, which medians to **0 cycles** — the operation costs less than the calibrated `rdtsc` overhead subtracted from it, so its value is quantisation. It is still measured, still baselined, still printed with its ratio, and still subject to every fail-closed check; only the conclusion is withheld. The set lives in source with a stated reason per entry and is pinned by test, deliberately *not* as a column in the baseline TSV — moving a metric out of the gate should require editing code with a reason attached, not a one-character change to a file nobody diffs.

5. **Everything `STORY-P1-01-02` established still holds.** **Met**: the fail-closed set is re-stated against the twelve-column baseline, and the two new ratio columns join it — non-numeric, zero, or a reference row whose ratio is not unity are each errors. A baseline carrying no reference row is refused outright. The ten-column header `STORY-P1-01-02` committed is now rejected by exact match rather than read with the new columns defaulted.

## Beyond the original scope

- **`LE-19(b)` is unchanged and still open.** `--update-baseline` still rewrites every measured row. This Story re-recorded the whole file deliberately (clause 8's ordering), so the limitation was not on the critical path, but it remains the reason a single metric cannot be refreshed in isolation.
- **The baseline is now portable in a way it was not.** Absolute baselines could never transfer between machines; the committed ratios are a property of the code rather than of the recording host. That claim is untested across hosts — this baseline was recorded on a Windows dev box and CI runs Linux — and its first CI run is the test. See §"Named debt".

## The finding this Story produced

**The pre-committed bound failed, and the reason it failed is more useful than the bound would have been.** The improvement a ratio buys is not uniform across metrics — it sorts by *what the metric is made of*:

- metrics made of hundreds of cycles of straight-line emulated guest code (`D05/dispatch_run_once`, `D04/context_switch`) track the reference closely, needing +26% and +16% headroom respectively;
- `D02/fault_ud2` goes through QEMU's exception delivery rather than TCG straight-line execution, and needs +9% — it tracks *better*, not worse, which was not predicted;
- the two small pool metrics are dominated by measurement quantisation, and one of them measures **0 cycles**. No denominator rescues a numerator that is noise.

The first analysis of this data was wrong in a way worth recording: it measured spread across *individual runs*, and the gate never compares an individual run. Re-computed over the median across a 3-run invocation — the statistic the gate actually uses — the same data supports gating five of six metrics rather than three. **Analysing a gate with a statistic the gate does not use produces a conclusion about nothing**, which is the same error, one level up, as the one this Story exists to fix.

## Tests

[`TEST-P1-01-04-A`](../tests/TEST-P1-01-04-A.md) — written before implementation, Red run recorded (72 compile errors against an API that did not exist), then Green.

## Reports

- [`REPORT-2026-07-28-06`](../reports/REPORT-2026-07-28-06.md) — the Red run, the calibration against clause 4's pre-committed bound including its failure, the tolerance derivation, the re-recorded baseline, and the four-quadrant demonstration.

## Goals verified

G-RT-3 (regression-suite half), G-DX-7. Neither is *closed*: both await hardware-tier evidence.

## Named debt this Story leaves open

- **`LE-16` restated, not closed.** The gate now catches ratio regressions of roughly **2x or worse** at Tier 0 — nominally looser than `STORY-P1-01-02`'s 60%, and strictly better, because that 60% applied to a quantity measured swinging +318% on unchanged code. No Tier 0 work improves this; it needs `LE-09`'s board.
- **`LE-18` addressed for the gated set, and newly bounded rather than eliminated.** A uniform slowdown of *everything including the reference* passes by construction. The reference is a fixed integer computation no kernel change can move, so the realistic shape of that hazard is a toolchain or codegen-flag change, which `REFERENCE_TOLERANCE`'s 4x structural band watches for.
- **New: `LE-23`** — the committed ratios were recorded on a Windows host and have never been compared against a Linux CI run. Ratios *should* transfer where absolutes could not, and that is this Story's central untested claim.
- **New: `LE-24`** — `D07/pool_u64x64_alloc_free_round_trip` measures below the harness's own calibrated overhead and is therefore not measurable at Tier 0 by this harness at all. Ungating it is containment, not a fix.
