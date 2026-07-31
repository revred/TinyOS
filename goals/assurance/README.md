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

### Selecting a domain whose subsystem does not exist: stated open debt

Added by [`STORY-P0-01-07`](../stories/STORY-P0-01-07.md), closing `LE-35`. Handover 25 set the precedent and never wrote the rule down.

Selecting a performance domain pulls **all 25 of its guardrails** into the selecting Story's contract. Where the domain's `readiness` column in [`../performance/catalogue.tsv`](../performance/catalogue.tsv) is `design`, `stand-in-only`, `specified` or `unbuilt`, **the subsystem does not exist, and not one of those 25 can be closed.** A guardrail cannot be closed for something that has not been built, and the absence of a heap in unwritten code is evidence about nothing.

So a selection of that kind is **initialised as stated open debt at the moment it is made**, in [`open-debt.tsv`](open-debt.tsv), with a reason per `(Story, domain)` pair. Left implicit, the contract presents as satisfiable and the cheapest lie available becomes recording all 25.

`check-assurance-spine` enforces it in both directions, and the second direction matters as much as the first:

- a contract selecting a non-implemented-readiness domain **without** a debt row is rejected, naming the readiness;
- a debt row for a domain that *is* implemented (`prototype`, `prototype-cooperative`, `prototype-inactive`, `partial`) is rejected — **debt may name a subsystem that does not exist; it may not excuse one that does**;
- a debt row whose recorded readiness disagrees with the catalogue is rejected;
- a `(Story, domain)` pair present in **both** `open-debt.tsv` and `guardrail-evidence.tsv` is rejected. A gate cannot be simultaneously unclosable and closed.

Debt is not a waiver. The obligation stays in the contract and stays visible; what the register records is that it cannot be discharged yet, and why.

### Quoting a worst-case bound: provenance, not just a number

Added by [`STORY-P0-01-07`](../stories/STORY-P0-01-07.md), closing `LE-33`. The decisions are [`ADR 0004`](../../docs/adr/0004-arm64-is-the-real-time-tier.md) and [`ADR 0005`](../../docs/adr/0005-arm64-real-time-tier-is-conditional-on-secure-world-qualification.md); until now both were prose, and a Report could file a `G04` row from a QEMU x86_64 run with every gate green.

`G04` is the bound-class column — *"observed maximum and WCET bound"* — and it is the only one that asserts what cannot be exceeded rather than describing what was seen. Filing a `G04` row therefore requires more than a measurement:

1. The Report must carry a `TOS64-BOUND/1` claim line for that guardrail id, naming its `tier`, `arch`, `platform` and `qualification` — the same provenance the `TOS64-MEAS/2` envelope now emits at the moment of measurement.
2. The claim is **refused** if its tier is `T0` (an emulator), its architecture is `x86_64` (SMIs are outside the OS's authority by design), or its platform is not `qualified` in [`qualified-platforms.tsv`](qualified-platforms.tsv).
3. **A platform absent from that register is unqualified, never presumed clean.** Silence is not evidence. As of writing, the count of qualified platforms is **zero — the Raspberry Pi 5 included.**

What this does *not* do is read English. A Report may still state a bound in a sentence, and no lint here parses sentences. What it makes impossible is a bound entering the machine-readable spine from a source the ADRs disqualify.

### Two commands, and which to run when

- `cargo run -p xtask -- check-spine-files` — fast. Header agreement, field count, key uniqueness and id contiguity on all 15 hand-edited TSVs, and nothing that requires opening a second file. This is the instrument [`agent/CONCURRENT_SESSIONS.md`](../../agent/CONCURRENT_SESSIONS.md) rule 8 asks you to run **between two edits**, and it is a strict subset of the full check.
- `cargo run -p xtask -- check-assurance-spine` — everything below, including the cross-file joins the fast check deliberately skips. This is what CI gates on.

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
- a domain selection whose subsystem does not exist and which is not initialised as stated open debt, a debt row for an implemented domain, a debt row disagreeing with the catalogue's readiness, or a `(Story, domain)` pair that is both open debt and evidenced (`LE-35`);
- a `G04` bound-class evidence row whose Report carries no `TOS64-BOUND/1` claim, or whose claim is sourced from Tier 0, from `x86_64`, or from a platform holding no secure-world qualification record (`LE-33`, `ADR 0004` + `ADR 0005`);
- a malformed platform register row, a `qualified` platform citing a Report that does not exist, or an `unqualified` one citing a qualification record at all;
- **a Feature Stories-table row that disagrees with the referenced Story's own `Status:` header** — the state word compared exactly, and `criterion N` / `criteria N and M` tokens compared as a set (`LE-44`);
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
