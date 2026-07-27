# TinyOS Assurance Spine

Status: **Enforced structurally; runtime assurance evidence remains debt.**

The 625 performance tests are not a separate checklist beside development. Together with the security controls and five containment classes they are a mandatory spine through the existing V&V chain:

```text
                           Goal → Epic → Feature → Story → Test → Report
Performance: D01..D25/G01..G25 ────────────────┼────────┼────────┼──────
Security:    SEC-01..SEC-20     ────────────────┼────────┼────────┼──────
Containment:C0..C4 / BND-01..20 ──────────┼────┼────────┼────────┼──────
Charter:     PD-01..14 / RCG-01..14 / 25 paths ─┼────────┼────────┼──────
Destinations:19 applications / 9 landing zones ──┼────────┼────────┼──────
                                                │        │        │
                                      contract declared  │   raw evidence
                                                 test shape
```

[`feature-contracts.tsv`](feature-contracts.tsv) gives every Feature an implementation class, subject classes, authority posture, hostile-input declaration, boundary-test selection, and evidence obligation. [`story-contracts.tsv`](story-contracts.tsv) gives every Story exactly one row selecting performance domains—thereby selecting all 25 guardrails in each domain—security controls, and containment classes whose invariants must survive the measured workload.

[`../../SECURITY_CHARTER.md`](../../SECURITY_CHARTER.md) governs the join. Its `PD-*` and `RCG-*` rows map back to all 20 controls and all 20 boundary tests, so existing Feature/Story mappings pull the applicable process and code-admission invariants into design and evidence without creating an unrelated bolt-on suite.

[`../context/application-platforms.tsv`](../context/application-platforms.tsv) and [`../context/landing-zones.tsv`](../context/landing-zones.tsv) extend the same spine forward. They keep destination goals, selected performance domains, real applications, security controls, containment classes, roadmap horizon and claim gates in the same machine-checked context. They do not make future runtimes or applications current Features; they ensure current architecture cannot forget what those destinations will demand.

## Lifecycle gates

1. **Define the Feature boundary.** Before decomposition, `feature-contracts.tsv` declares where the implementation runs, which classes it mediates, its authority posture, hostile inputs, selected `BND-*` tests, and required evidence.
2. **Define the Story contract.** A Story cannot enter implementation until it has a row in `story-contracts.tsv` naming its performance domains, security controls, and containment classes. Adding an unmapped Story fails CI.
3. **Red.** Its `TEST-*` specification names the mapped performance, security, class, and boundary-test IDs, measurement tier, hostile inputs, authority assumptions, and safe failure state.
4. **Green.** Functional tests pass. This can earn functional `Verified`, but not assurance `verified`.
5. **Measure.** Reports execute applicable `PERF-Dnn-G01..G23`, `SEC-*`, and `BND-*` release gates in the declared deployment profile. Unavailable future hardware or subsystems remain explicit debt.
6. **Claim.** `G24` Linux and `G25` RTOS comparisons run only after absolute release gates pass, on the same hardware and safety-equivalent configuration.
7. **Release.** No mapped release gate may be failed, missing, waived silently, or hidden by an aggregate score.

## States

- `specified` — the Story is planned and its assurance contract exists.
- `baseline-debt` — the Story was functionally Verified before this spine existed, or lacks the required performance/security evidence. It is not release-assured.
- `verified` — dated raw evidence and Reports close every applicable mapped release gate.

All 23 currently functional-Verified Phase-0 Stories are `baseline-debt`; the two planned Stories are `specified`. This avoids rewriting history or pretending that functional tests measured latency tails, CPU cycles, memory allocations, active cross-process isolation, signing, or hostile-load safety.

## What CI enforces

`cargo run -p xtask -- check-assurance-spine` rejects:

- an incomplete or malformed five-class containment catalogue;
- an incomplete or malformed 20-test boundary catalogue;
- a missing or disconnected Security Charter;
- an incomplete or malformed 14-row Protection Domain catalogue;
- an incomplete or malformed 14-gate code-admission catalogue;
- a missing or invalid pair in the complete 25-row C0–C4 communication matrix;
- a missing or malformed row in the 19-target application/platform catalogue;
- a missing or malformed row in the nine-zone whole-system landing catalogue;
- an unknown application performance/security/class reference, an application selected by no landing zone, or a landing-zone promise unsupported by its selected applications;
- a security control or boundary test selected by neither the Protection Domain nor code-admission charter;
- a Feature without exactly one containment contract;
- a Story without exactly one contract row;
- a contract for a nonexistent Story or Feature;
- a Test whose ID does not resolve to a mapped Story;
- a Test whose performance, security, containment, boundary-test, or assurance-state metadata differs from its Story and Feature contracts;
- a Report that references no mapped Story or Test;
- malformed, duplicate, or unknown `Dnn` and `SEC-nn` references;
- an unknown assurance state;
- an incomplete security control catalogue;
- a broken or incomplete 625-cell performance catalogue.

The gate proves the spine is complete and cannot drift. It does not fabricate runtime measurements.

## Required Story, Test, and Report fields

New or materially revised artifacts include:

```text
Feature contract: goals/assurance/feature-contracts.tsv
Assurance contract: goals/assurance/story-contracts.tsv
Performance domains: Dnn,...
Security controls: SEC-nn,...
Containment classes: Cn,...
Boundary tests: BND-nn,...
Protection Domain contracts: PD-nn,...
Code admission gates: RCG-nn,...
Assurance state: specified | baseline-debt | verified
```

A Test expands each selected domain to its applicable `PERF-Dnn-Gnn` IDs, states which security and class invariants it attacks, and names applicable Feature-level `BND-*`, `PD-*`, and `RCG-*` contracts. A Report carries raw evidence and pass/fail per ID, actual class placement, capability set, hostile input, and failure state. One fast average or one functional pass cannot close an entire domain.
