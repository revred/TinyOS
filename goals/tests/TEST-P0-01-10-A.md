# TEST-P0-01-10-A — A Specified Header Cannot Outlive Its Own Passing Report

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P0-01-10`](../stories/STORY-P0-01-10.md)
Tier: Host unit tests only — the subject is the agreement between documents in this repository
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`, `BND-18`
Protection Domain contracts: `PD-02`, `PD-12`, `PD-13`, `PD-14`
Code admission gates: `RCG-05`, `RCG-06`, `RCG-07`, `RCG-12`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

`LE-65`: nothing compares a Story's `Status:` header to the Story's own filed Reports, so
`STORY-P0-01-08` read *Specified* for four days after `REPORT-2026-07-28-11` recorded Pass on all
five clauses — with the badge gate, the `LE-44` gate and every count gate green, because header,
Feature-table cell and badge were wrong in agreement. This document specifies the gate that
closes the loop, and fixes its scope where `LE-65`'s proposal was too wide.

## Specification

### 1. The refusal

**Given** every Report's `Test(s) covered:` field and `## Result` opener,
**then** `check-assurance-spine` refuses the tree when a Story whose own header state is
`Specified` is covered — directly by id, or through one of its Tests' ids — by a Report whose
Result opens with a bolded `Pass`/`PASS`.

**And** the error names the Story, its state, the offending Report, and the fix direction:
re-verify the Report's evidence against the current tree, then advance the header — Handover
35's verify-don't-inherit rule, cited as such.

### 2. The deliberate non-refusals

**And** `In progress` is not refused: `REPORT-2026-07-30-01` records PASS while its `FEAT-P2`
Stories deliberately stay `In progress` with performance debt stated open — refusing that
combination would refuse honesty. The precedent is restated in the gate's documentation.

**And** a Report with no `## Result` section (the 2026-07-26 generation), or whose Result opens
with anything other than a bolded pass, extracts no verdict and refuses nothing — a gate that
guessed at prose would produce false refusals, and false refusals teach people to bypass gates.

### 3. Every refusal is demonstrated

**Given** that Step 1 of this session already corrected the only live instance,
**then** the committed tree passes by construction and a green run proves nothing.
**Therefore** host tests drive, on fabricated documents: the `-08` incident's exact shape
refused; the same Report beside a `Functionally Verified` header accepted; `In progress` beside
a passing Report accepted; a `Specified` Story with a resultless Report accepted; coverage
through the Test id refused the same as direct coverage. Each refusal beside its acceptance
case.

## What this test explicitly does not establish

- **That every passing Report is detected.** Only the bolded `## Result` opener grammar is read;
  a pass recorded in any other shape is invisible. The Report schema is where a stricter
  contract belongs.
- **That `In progress` headers are fresh.** Deliberately out of scope, per §2.
- **Anything about `README.md`** (`LE-34`). Still open.
- **Any performance guardrail.** None closes here and no Story's assurance state moves.

## Reports

- [`REPORT-2026-08-01-02`](../reports/REPORT-2026-08-01-02.md)
