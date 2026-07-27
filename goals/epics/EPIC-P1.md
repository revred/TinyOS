# EPIC-P1 — Determinism Proof

Status: **In progress — `FEAT-P1-01`/`FEAT-P1-02`/`FEAT-P1-03` complete (all exit criteria met; assurance `baseline-debt`, Tier 0 evidence only), `FEAT-P1-04` half done (`STORY-P1-04-01` Verified 2026-07-28), `FEAT-P1-05`/`FEAT-P1-06` still specified**
Roadmap phase: [Phase 1 — Determinism proof](../../README.md#roadmap)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)
Depends on: [`EPIC-P0`](EPIC-P0.md) (functionally complete — all 25 Stories Verified; its assurance debt is exactly what this Epic starts paying down)

## Goal

Turn `EPIC-P0`'s functionally-verified skeleton into a *demonstrated* deterministic system: real fault handling, active per-task address spaces, timer-driven preemption with deadline/WCET enforcement, a CI gate that fails on **timing** regressions the way it already fails on functional ones, and hostile-load evidence that exhaustion is contained — per `README.md`'s Phase 1 line ("deadline monitor, priority inheritance, worst-case timing benchmarks and regression suite; CI gate on timing regressions, not just functional correctness").

This Epic is where the assurance spine's `baseline-debt` first starts converting to `verified`: `EPIC-P0` built the mechanisms and honestly marked every Story's performance/security contract as debt; `EPIC-P1` exists to produce the dated, raw-evidence Reports that close those gates for the timing and isolation controls in its scope. It also directly targets the Security Charter's named runtime-evidence gaps that block any strong security claim: active per-task address spaces, fault containment, teardown, and executable sealing (W^X) — see `SECURITY_CHARTER.md` and Handover 33's list.

## Goals verified (from `SeedMVP.md` §3)

- **G-RT-1** — Preemptive, priority-based scheduling with bounded interrupt latency and documented priority-inversion avoidance — `EPIC-P0` built the priority scheduler, priority-inheriting lock, and cooperative dispatch; this Epic makes preemption *real* (timer-driven) and its latencies *measured*.
- **G-RT-3** — WCET-aware task model: every RT task declares a WCET budget; the scheduler **and the CI timing regression suite** both hold code to that budget.
- **G-PA-1** — Deterministic actuation: bounded, tested worst-case latency from decision to actuation, *enforced by the scheduler*, not merely observed.
- **G-SEC-2** — Process memory private by construction: active per-process address spaces, W^X/NX, generation-safe teardown, complete context switching.
- **G-SEC-8** — *(partial in this Epic)* Ransomware/worms cannot gain durable ambient reach — this Epic contributes the immutable-image/W^X substrate; signed atomic updates and recovery land with `EPIC-P1_5`'s deploy tooling, per `SeedMVP.md` §9's phase table.
- **G-SEC-12** — Exhaustion is contained: declared CPU/memory/time/depth/rate bounds with priority-safe RT reserves and bounded recovery, proven under hostile load.
- **G-SEC-13 through G-SEC-15** — per `SeedMVP.md` §9's Phase 1 row (the epic-backlog table's shorter "-12" listing is superseded by SeedMVP's "-12 through -15"): containment-class integrity, unified policy/spoor provenance, and secret isolation apply to every fault path, teardown path, and measurement artifact this Epic adds.

## Hardware & test tier

**User direction (2026-07-27, Handover 37): the Raspberry Pi 5 is the only viable hardware in the short term.** That supersedes, for this Epic's planning, `SeedMVP.md` §9's generic "both MVP boards" line — and it carries an honest consequence this Epic must not paper over: the Pi 5 is an **ARM64** board, and TinyOS today has an x86_64 HAL only (`hal-x86_64`; the arch-neutral `hal` crate was designed for a future device-tree/ARM64 backend, but none exists — that work sits in `EPIC-P7` per the current backlog, with `README.md`'s test matrix listing the Pi 5 as a Tier 1 portability board "Phase 3 onward"). So the concrete hardware ladder for this Epic is:

1. **Tier 0 (QEMU `q35`) now** — every mechanism in this Epic is proven here first, exactly as in `EPIC-P0`.
2. **Raspberry Pi 5 as the first physical timing target** — reaching it requires pulling forward a minimal ARM64 bring-up slice (boot + timer + the measurement harness's primitives) that the current plan parks in `EPIC-P7`, plus a deploy path (`EPIC-P1_5`'s territory). **Sequencing decided by the user 2026-07-27: Option B with a carve-out** ([`Handover 03`](../../session/hand-2026-07-27/03-le-09-arm64-pi5-slice-proposal.md)) — the ARM64 `CNTVCT_EL0`/`CNTFRQ_EL0` cycle source and timebase are written **now** as host-testable code, since they need no board and are the only thing that actually tests the measurement harness's arch-neutrality claim — **delivered 2026-07-27 as [`STORY-P1-01-03`](../stories/STORY-P1-01-03.md)**, with its own contract row and Test document first, and the seam held (no implementation line of `hal`, `kernel` or `xtask` changed); UART-borne pass/fail folds into `STORY-P1-01-02`, because a gate that can only read a QEMU exit code can never gate hardware; and every board-dependent piece (AArch64 boot + target spec, the PL011 driver, the SD-card/serial run path) waits until `FEAT-P1-02` exits, after which a fault on that board is a reportable event over the UART instead of a silent hang with no output at all. `LE-09` itself stays open until a Pi 5 has actually produced a measurement — a decision is not evidence.
3. Per the assurance spine's no-waiver rule, **hardware-tier timing evidence is named, explicit release-blocking debt** on every timing claim until it is measured on the Pi 5. QEMU cycle counts calibrate the *harness and the regression mechanism*; they never substitute for hardware WCET numbers, and no Report in this Epic may claim otherwise.

New test types this phase introduces (per `SeedMVP.md` §7): **timing regression / WCET benchmarks**, **property-based tests**, and **hostile-load tests**.

## Features

| Feature | Summary | Status |
|---|---|---|
| [`FEAT-P1-01`](../features/FEAT-P1-01.md) | Timing measurement harness & CI timing-regression gate | Complete (all 3 Stories Verified 2026-07-27) |
| [`FEAT-P1-02`](../features/FEAT-P1-02.md) | Real CPU exception handling & fault containment | Functionally complete (both Stories Verified 2026-07-27; `LE-17` fault-latency baseline closed the same day) |
| [`FEAT-P1-03`](../features/FEAT-P1-03.md) | Active per-task address spaces, W^X & teardown | Complete — `STORY-P1-03-01` (CR3 switching mechanism) Verified 2026-07-27; `STORY-P1-03-02` (W^X, sealing, teardown, first real scheduled task) and `STORY-P1-03-03` (IAT resolution, the `os` system image, the D04 switch cost) Verified 2026-07-28. Every exit criterion met |
| [`FEAT-P1-04`](../features/FEAT-P1-04.md) | Timer-driven preemption, deadline monitor & WCET watchdog | In Progress — `STORY-P1-04-01` (timer-driven preemption, extended state, inversion avoidance under real preemption) Verified 2026-07-28, closing `LE-01` and `LE-14`; `STORY-P1-04-02` (WCET watchdog on the real timer, `LE-02`) not started, so the Feature does not exit |
| [`FEAT-P1-05`](../features/FEAT-P1-05.md) | Hostile-load & exhaustion-containment proof | Specified |
| [`FEAT-P1-06`](../features/FEAT-P1-06.md) | Deterministic actuation proof (G-PA-1 flagship path) | Specified |

**Ordering rationale.** `FEAT-P1-01` first — every other Feature's exit criteria are stated in measured numbers, so the ruler must exist before the things it measures (and `EPIC-P0`'s pool-bench fixture already prototyped the pattern to generalize: cycle-calibrated measurement, percentile reporting over serial, harness-side parsing). `FEAT-P1-02` before `FEAT-P1-03`, preserving Handover 32/33/35's standing reasoning: a live per-task page-table switch with no real fault handler behind it is strictly more dangerous than the current identity map — so faults first, then CR3. `FEAT-P1-04` needs `FEAT-P1-02` (an overrun/preemption path that faults must land in a real handler, not the fail-closed diverge-and-report default). `FEAT-P1-05` and `FEAT-P1-06` are proof Features consuming all of the above; `FEAT-P1-06` is the Epic's flagship exit — the first end-to-end, scheduler-enforced decision-to-actuation bound, the primitive `G-PA-8`'s CNC milestone eventually builds on.

## Exit criteria for this Epic

- All six Features reach **Verified** at minimum Tier 0, each with valid contract rows, Tests, and dated Reports.
- `xtask`'s timing-regression gate runs in CI on every PR and has demonstrably failed at least once on a deliberately-introduced regression (the same "prove the gate can fail" discipline `fixture-broken-boot` established for boot).
- At least the timing/isolation controls in this Epic's scope (`SEC-03` active-address-space isolation, `SEC-19` on fault paths, `SEC-20` exhaustion) hold **evidence-backed `verified` state on at least one Story each** — the first conversions of `baseline-debt` anywhere in the repository.
- The Security Charter runtime-evidence list shrinks by, at minimum: active per-task address spaces, fault containment/teardown, and executable sealing (W^X) — with the remaining items (production capability spaces, IOMMU, quarantine/provenance, immutable updates, hostile-campaign testing) explicitly re-confirmed as open, not silently forgotten.
- Tier 1/Tier 2 hardware timing debt is either closed (boards purchased and measured) or restated as explicit, dated release-blocking debt in every timing Report this Epic files.
- `EPIC-P1_5` (Deploy tooling) can begin with a system whose determinism claims are measured, not asserted.
