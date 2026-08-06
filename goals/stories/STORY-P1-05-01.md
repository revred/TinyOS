# STORY-P1-05-01 — Hostile-Load Campaign: Saturation, RT Reserves, Bounded Recovery

Status: **Specified, not yet started**
Feature: [`FEAT-P1-05`](../features/FEAT-P1-05.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

A flooding fixture playing a compromised C2 component saturates, in turn and in combination: pool allocation, task-slot creation, spoor-journal writes, IPC channels/grants, and ready-queue churn — while a declared RT task runs its deadline workload on the same core. The claim under test is `G-SEC-12`'s, verbatim: every bound holds, RT reserves are priority-safe, denial is bounded and attributable, recovery after the flood is bounded and complete. Property-based tests (new this phase) state the invariant over interleavings, not examples: no schedule of hostile allocations may starve an RT reserve.

## Depends on

`STORY-P1-04-01`/`-02` (real preemption and deadline enforcement are what "RT reserves survive" means); `STORY-P1-01-02` (degradation measured against committed baselines).

## Scope, settled 2026-08-06 before the Story starts

The scope question `09A` §9 raised, answered here rather than discovered mid-campaign. **The
saturation vectors this Story covers are pools, task slots, the ready queue and the spoor
journal — `D05`, `D07`, `D11`.** The contract row gained `D11` on 2026-08-06: the description
above names spoor-journal saturation and the row did not select the journal's own domain. `D11`
is `prototype` readiness, so the addition costs no `open-debt.tsv` row.

**IPC channel and grant saturation are deliberately out of scope.** They are `D12` and `D13`,
both `specified` readiness — the subsystems do not exist. Selecting them would force open-debt
rows that can never be closed, in both directions, and `check-assurance-spine` enforces exactly
that. They split into a second Story that lands when those subsystems do; the sentence in the
Description above is the record of what that Story owes.

**Three of the four things the criteria below assert must be true have no mechanism in the
kernel to be true of**, which is why this Story has not started and why starting it is a build
job rather than a test-writing one. In cost order:

1. **There is no RT reserve and no per-class budget.** `Tcb` carries `base_priority`,
   `inherited_priority`, `wcet_budget`, `overrun_policy`, `entry`, `ticks_consumed` and `state`
   — and no containment class. The pool is one flat capacity with no class tag and no
   reservation floor, and a repository-wide search for a scheduling or allocation reserve finds
   none. So `BND-15` and `RCG-08` have nothing behind them. **This is Feature-sized design work
   and probably its own Feature rather than a Story of this one.**

   **Acted on 2026-08-06: it is now [`FEAT-P1-12`](../features/FEAT-P1-12.md).** This item is
   no longer a prerequisite recorded inside this Story, and the reason for moving it is the
   effect it was having here. The standing do-not-start rule names *"`FEAT-P1-05`'s RT
   reserve"* and always did — but a rule naming one item inside a Story that holds four gets
   applied to the Story, and four consecutive handovers carried it that way while
   `FEAT-P1-06`'s half 3 waited behind it. **Items 2, 3 and 4 below were never blocked and are
   startable now.** Nothing about the scope changed in the move; `FEAT-P1-12` restates the
   audit above and adds no design surface to it.
2. **Denial is not attributable to an offender.** `Actor` is a three-value nibble — `Kernel`,
   `Exec`, `Session`. Criterion 2 wants every denial charged to the offender and
   spoor-attributed; today a spoor can say the kernel denied something, not *which task, of
   which class* caused it. Extending the encoding is a wire-format change touching
   `spoor_stream.rs`, `spoor_wire.rs` and the ARM64 parity vocabulary, and it interacts with
   `LE-82`. Do them together: both are "the spoor vocabulary cannot say the thing the contract
   requires".
3. **No property-testing infrastructure.** Criterion 4 needs the interleaving invariant in CI
   with a recorded seed policy. There is no `proptest`, `quickcheck` or `arbitrary` dependency
   anywhere under `os/`, and `kernel` is `no_std`. This needs a host-tier dev-dependency, a
   decision recorded against `RCG-07`'s minimal-dependency-surface stance, and the seed policy
   written down *before* the first test.
4. **No campaign harness.** `xtask` parses single measurement envelopes; this needs concurrent
   flood-plus-RT orchestration, idle-versus-flood distributions side by side, and a measured
   recovery-time bound after the flood stops.

Steps 1 and 2 are the honest answer to "what will it take". Steps 3 and 4 are the part this
document describes, and they are the smaller half.

**Re-read 2026-08-06, after step 1 moved to `FEAT-P1-12`.** "The smaller half" undersold what
is reachable. Steps 3 and 4 together are what `FEAT-P1-06`'s half 3 needs — `PERF-D05-G19`
(`loaded_degradation`) and `PERF-D05-G21` (`fault_completion`) ask for *measurements under
load*, not for a guarantee, so a flooding fixture plus a campaign harness produces both
numbers with no reserve in the tree. Step 2 pairs with `LE-82`, already open: both are "the
spoor vocabulary cannot say the thing the contract requires", and this Story already says to
do them together. **Three session-sized items, none blocked** — and if `G19` fails without a
reserve, that failing row is what turns `FEAT-P1-12` from architecture prose into a number,
which is the order this repository holds every other claim to.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. Under each saturation vector and their combination, the RT task's measured deadline-hit rate and latency distribution stay within its declared bound — raw distributions in the Report, idle-vs-flood side by side.
2. Every denial is `Err`, bounded in cost, charged to the offender, and spoor-attributed; no denial path allocates, amplifies, or blocks unboundedly.
3. When the flood stops, recovery to baseline is bounded and complete (no leaked slots, no stuck queues, no residual degradation) — measured, with the recovery-time bound recorded.
4. The property-based invariant runs in CI at host tier with a recorded seed policy; Tier 0 carries the behavioral campaign.

## Tests

Not yet written — deferred until this Story starts. Requires host property tests plus a Tier 0 campaign fixture.

## Goals verified

G-SEC-12; G-SEC-14 (attributable denial); first `baseline-debt` → `verified` conversion candidate for `SEC-20`.
