# Handover 36 — EPIC-P0 Functionally Complete; EPIC-P1 (Determinism Proof) Created and Fully Decomposed

Follows: [`35-story-p0-04-03-pci-bus-enumeration.md`](35-story-p0-04-03-pci-bus-enumeration.md) (which closed the last EPIC-P0 Story). This is the **exclusive Epic-transition handover** the user asked for: it records EPIC-P0's functional closure and elaborates `EPIC-P1` end to end — Epic file, 6 Features, 10 Stories, machine-checked assurance contracts — without starting any implementation.

## EPIC-P0 closure (what is and is not claimed)

- **Claimed:** all 25 Stories functionally **Verified** at minimum Tier 0; all 24 Test documents passing locally; 29 Reports filed; every Feature/Story carries a valid assurance-contract row. [`EPIC-P0.md`](../../goals/epics/EPIC-P0.md)'s status line now says exactly this.
- **Not claimed:** release-readiness or assurance closure. Every EPIC-P0 Story remains `baseline-debt` — no latency-tail, WCET, hostile-load, isolation, or campaign evidence exists yet. Known open functional caveats are carried forward with named owners in EPIC-P1: `STORY-P0-02-03`'s priority-inheritance behavioral half (→ `STORY-P1-04-01`), `STORY-P0-02-04`'s "no timer, no watchdog" gap (→ `STORY-P1-04-02`), Handover 32's TSS/IST and real-fault-handler items (→ `FEAT-P1-02`), the dormant-not-active `AddressSpace` machinery (→ `FEAT-P1-03`), and CI has still never been observed running this work (unchanged, phase-independent).

## What EPIC-P1 is

**Phase 1 — Determinism proof** ([`goals/epics/EPIC-P1.md`](../../goals/epics/EPIC-P1.md)): turn the functionally-verified skeleton into a *demonstrated* deterministic system, and start converting `baseline-debt` to `verified` — the first Epic whose primary deliverable is evidence, not mechanism. Goals: G-RT-1, G-RT-3, G-PA-1; G-SEC-2, G-SEC-8 (partial — substrate only, deploy half lands in `EPIC-P1_5`), G-SEC-12; plus G-SEC-13–15 per `SeedMVP.md` §9's Phase 1 row (which supersedes the backlog table's shorter listing — noted in the Epic file). New test types this phase: timing regression/WCET benchmarks, property-based, hostile-load.

**Hardware honesty up front:** SeedMVP names "both MVP boards" for this phase; neither is purchased. Tier 0 (QEMU) proves every *mechanism*; every timing Report must carry hardware-tier evidence as named release-blocking debt until boards exist. No QEMU number may masquerade as a hardware WCET claim.

## The decomposition

| Feature | Stories | What it proves |
|---|---|---|
| [`FEAT-P1-01`](../../goals/features/FEAT-P1-01.md) Timing harness & CI regression gate | `-01-01` reusable cycle-calibrated harness; `-01-02` committed baselines + `check-timing-regression`, proven able to fail | The ruler. Generalizes the pool-bench pattern; D04/D05/D07 first. Timing regressions fail PRs like functional failures. |
| [`FEAT-P1-02`](../../goals/features/FEAT-P1-02.md) Real exception handling | `-02-01` `#PF`/`#GP`/`#UD` capture + terminate-vs-resume + spoor; `-02-02` TSS/IST double-fault survival | Faults contain to the faulting task; the kernel survives a fault in its fault path. Handover 33's priority 2. |
| [`FEAT-P1-03`](../../goals/features/FEAT-P1-03.md) Active address spaces, W^X, teardown | `-03-01` per-task CR3 in the context switch; `-03-02` W^X/NX mappings + generation-safe teardown | G-SEC-2 becomes an active runtime fact; closes three Security Charter runtime-evidence gaps (active spaces, sealing, teardown). **Hard-ordered after FEAT-P1-02**, per the standing Handover 32/33/35 reasoning. |
| [`FEAT-P1-04`](../../goals/features/FEAT-P1-04.md) Preemption, deadline monitor, WCET watchdog | `-04-01` timer-driven preemption + priority inheritance under real preemption; `-04-02` overrun → declared fault policy | The scheduler gets teeth: the armed timer finally has a consumer, `kernel::wcet` gets a real clock, overruns trip policy — never silent. |
| [`FEAT-P1-05`](../../goals/features/FEAT-P1-05.md) Hostile-load proof | `-05-01` saturation campaign: RT reserves held, bounded attributable denial, bounded recovery, property-based invariants | G-SEC-12 with raw adversarial data — the first Report whose primary content is attack evidence. First `SEC-20` `verified` conversion candidate. |
| [`FEAT-P1-06`](../../goals/features/FEAT-P1-06.md) Deterministic actuation (flagship) | `-06-01` decision-to-actuation bound: declared budget, enforced deadline, measured under idle + hostile load, overrun trip demonstrated | G-PA-1 verbatim: *enforced by the scheduler, not merely observed*. The primitive the G-PA-8 CNC milestone later builds on. |

**Ordering:** `-01` first (the ruler), `-02` strictly before `-03` (no live CR3 without real fault handlers), `-04` after `-02`, `-05`/`-06` consume everything and close the Epic.

## Machine-checked state (all gates green after this decomposition)

- `goals/assurance/feature-contracts.tsv`: +6 rows (FEAT-P1-01..06) with class/BND/PD/RCG selections per Feature doc.
- `goals/assurance/story-contracts.tsv`: +10 rows, all `state=specified`, classes ⊆ each Feature's, every control class-applicable.
- `xtask`'s `committed_assurance_spine_is_complete`: expectations updated to **14 Features / 35 Stories** (Tests 24 / Reports 29 unchanged — Tests are written when each Story starts, per TDD).
- `cargo run -p xtask -- check-assurance-spine`: **passes** — 14 Features, 35 Stories, 24 Tests, 29 Reports, **1,500** selected Story/performance contracts (was 1,025), 6,575 application/performance contracts, all fixed catalogues unchanged.
- `cargo test -p xtask`: **23/23**.
- Dashboard, traceability matrix (new EPIC-P1 table + session row), epic backlog (EPIC-P1 promoted out), and `EPIC-P0.md` closure status all updated.

## Deliberately not done

- **No implementation.** All 10 Stories are `specified`; no Test documents exist yet (written test-first when each Story starts — creating them now, before Red, would be TDD theater).
- **No `EPIC-P1_5` decomposition** — deploy tooling stays in the backlog; its G-SEC-8 half is explicitly cross-referenced from EPIC-P1's goal list.
- **No hardware purchase decision** — surfaced as the Epic's standing debt; deciding it is the user's call, and `EPIC-P1`'s exit criteria work either way (close the debt or restate it dated).
- The `pool-bench` harness-error observation from Handover 35 still stands for its owning thread; `STORY-P1-01-01` will subsume that fixture onto the general harness, which is the natural place to resolve it.

## Immediate next steps

1. **`STORY-P1-01-01`** (measurement harness) — first, by the Epic's own ordering; everything else is measured with it. Refactoring pool-bench onto it resolves Handover 35's open fixture observation en passant.
2. **`STORY-P1-01-02`** (baselines + CI gate), then **`FEAT-P1-02`** (faults) — unblocking `FEAT-P1-03`/`-04` per the dependency graph.
3. When any timing Story's Report is first filed: decide the Tier 1/Tier 2 board purchase question it will force (buy and measure, or keep restating dated debt).
