# FEAT-P1-05 — Hostile-Load & Exhaustion-Containment Proof

Status: **Specified — no Story started**
Epic: [`EPIC-P1`](../epics/EPIC-P1.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

The first *adversarial-by-design* Feature (Goal **G-SEC-12**; `SeedMVP.md` §7's "hostile-load" test type, introduced this phase): prove — with dated raw evidence, not architecture prose — that saturating every bounded thing `EPIC-P0` built (pools, queues, spoor journal, IPC channels, task slots, the scheduler's ready queue) degrades the *attacker's* service and nobody else's. Exhaustion must be contained to the offender's declared budget: RT tasks keep their reserves and deadlines under a C2-class flood, denial responses are themselves bounded (no amplification), recovery after the flood stops is bounded and complete, and every denial is attributable via spoor. This Feature is also where property-based tests (SeedMVP §7, this phase) enter: "no interleaving of hostile allocations can starve an RT reserve" is a property, not an example.

## Crate(s) involved

`os/src/kernel/` (hostile-load fixtures, any budget-accounting gaps they expose), `os/src/xtask/` (campaign harness), potentially `os/src/exec/` (loader-facing exhaustion probes)

## Depends on

`FEAT-P1-04` (RT reserves under load are only meaningful with real preemption and deadline enforcement), `FEAT-P1-01` (degradation is measured against baselines, not eyeballed).

**[`FEAT-P1-12`](FEAT-P1-12.md) — split out of `STORY-P1-05-01` on 2026-08-06, and it gates *criterion 1* rather than the Feature.** The RT reserve and per-class budget do not exist: `Tcb` carries no containment class and the pool is one flat capacity with no reservation floor, so `BND-15` and `RCG-08` are selected by the contract row below against a mechanism that is not there. That is Feature-sized design work and is now its own Feature at this Story's own recommendation.

**What that changes here is which work is blocked, and the answer is less than four sessions assumed.** The standing do-not-start rule names *"`FEAT-P1-05`'s RT reserve"* and always did — but while the reserve sat as item 1 inside `STORY-P1-05-01`'s scope, the rule was read as covering the Story, and therefore this Feature. Items 2 (denial attribution, which pairs with `LE-82`), 3 (property-test infrastructure) and 4 (the campaign harness) were never blocked, are each session-sized, and **items 3 and 4 are what `FEAT-P1-06`'s half 3 actually needs** — `PERF-D05-G19` and `PERF-D05-G21` ask for measurements under load, not for the guarantee a reserve provides.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P1-05-01`](../stories/STORY-P1-05-01.md) | Hostile-load campaign: saturation, RT-reserve preservation, bounded recovery, attributable denial | Specified |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0/C1** · subject **C1/C2/C3/C4** · boundary tests **BND-15, -16, -20**.

The load generator plays a compromised C2 component and must prove single-compromise insufficiency (`BND-20`) for the exhaustion vector specifically: one flooding domain cannot consume another class's budget or any RT reserve (`BND-15`), cannot buy priority with class (`BND-16`), and cannot make the kernel's own denial path unbounded (`PD-08`/`PD-09` — denial work is charged to the caller).

## Exit criteria

The Story **Verified** at Tier 0 with a dated Report containing raw campaign evidence — the first Report in the repository whose primary content is adversarial-load data rather than functional pass/fail — and `SEC-20`'s state on this Story converting from `baseline-debt` to `verified` for the Tier 0 scope (hardware-tier debt stays named).

### Re-worded 2026-08-06 — "`verified` for the Tier 0 scope" is not a state the spine can represent

The paragraph above asks for something the assurance spine has no way to record, and a criterion
that cannot be represented is a criterion that will be declared met by prose. **A Story's
assurance state is all-or-nothing**: `baseline-debt` or `verified`, with no partial value and no
per-tier value. There is no "verified for the Tier 0 scope".

What the spine *can* represent, and therefore what this Feature actually exits on:

- **Functional `Verified`** on `STORY-P1-05-01`, from its `Status:` header, on the strength of the
  campaign — every saturation vector run, RT reserves held, denial bounded and attributable,
  recovery measured. This is the substance and it is fully reachable at Tier 0.
- **Guardrail evidence rows** in `guardrail-evidence.tsv` for the gates the campaign actually
  measures, in `D05`, `D07` and `D11`, each read against its `target` column **before** the
  measurement is taken rather than after. `cargo run -p xtask -- assurance-status` prints which
  of those gates are reachable; `G19` (isolation under competing load) and `G21` (exhaustion and
  fault containment) are this campaign's own gates and are currently counted as *unevidenced
  because unbuilt*, which is the honest reading.
- **`SEC-20` staying `baseline-debt`.** Not as a failure — as the accurate record. The conversion
  the original paragraph asks for is gated on hardware-tier obligations this Feature does not
  discharge, and `qualified-platforms.tsv` holds zero qualified platforms.
- **No `G04` row from any of it.** `PERF-D05-G04` and `PERF-D07-G04` are bound-class and are
  refused at `T0`, on `x86_64`, and from any platform absent from `qualified-platforms.tsv`. The
  refusal lands at the *end* of the work if nobody reads this first, which is the only reason
  this bullet exists.

So the honest exit is **functionally `Verified` with `baseline-debt` retained**, and the Feature
document says so here rather than leaving a session to discover it at a release gate.
