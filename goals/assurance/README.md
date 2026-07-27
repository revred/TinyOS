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

### Between "none" and "all": the guardrail evidence register

A Story's state is all-or-nothing by design, and that is right for a *release* decision: `verified` must mean every applicable gate, or it means nothing. But it left the spine unable to say anything in between, so a Story with 20 of its 23 release gates closed was indistinguishable from one with none, and a gate blocked on absent hardware was indistinguishable from a gate blocked on nobody having looked.

[`guardrail-evidence.tsv`](guardrail-evidence.tsv) records which `PERF-Dnn-Gnn` gates carry dated evidence, at the granularity the gates are already defined at. Added by [`STORY-P0-01-05`](../stories/STORY-P0-01-05.md), closing `LE-32`.

Three properties make it legal under the no-waiver rule below, and they are not negotiable:

- **It is a count of evidence, never a score.** No rollup, no percentage, no pass rate.
- **A gate absent from it is `unevidenced`, never `passed`.** The register can only ever say that something *was* measured, not that a threshold held.
- **No Story's assurance state is derived from it.** `baseline-debt` does not become `verified` because rows accumulated. That conversion still requires every applicable gate, by hand, with Reports.

The rule that "no mapped release gate may be failed, missing, waived silently, or hidden by an aggregate score" forbids concealing a failed gate behind a summary. It does not forbid recording which gates have evidence — and the spine was weaker for lacking that, because real work was invisible until an all-or-nothing threshold flipped.

The register's own integrity check is the one that matters: **a Story may only file evidence in a domain its own contract selects.** Evidence filed against a gate nobody was ever obliged to close is a more convincing way to be wrong than having no register at all.

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
- a guardrail-evidence row whose id is malformed, whose domain disagrees with its own id, whose Story has no contract row, whose Story does not select the domain it claims evidence in, or that duplicates an existing `(guardrail, story)` pair;
- **a shipped crate that could allocate** — every crate inside the image must declare `no_std` and must not declare `#[global_allocator]`, `extern crate alloc`, or `use alloc::` outside `#[cfg(test)]`. This is the evidence behind every `PERF-Dnn-G11` row: the guardrail asks for zero heap allocations per steady-state work unit, and this system has no heap at all, which is stronger and compiler-enforced. The gate exists so that adding an allocator withdraws the evidence loudly instead of invalidating it silently;
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
