# STORY-P0-01-10 — A Specified Header Cannot Outlive Its Own Passing Report

Status: **Functionally Verified (Host), 2026-08-01** — assurance state `baseline-debt`; delivered by [`REPORT-2026-08-01-02`](../reports/REPORT-2026-08-01-02.md), Pass on all four clauses, header advanced in the delivery commit. `LE-65` closed, with its `In progress` half deliberately dropped on the `REPORT-2026-07-30-01` precedent
Feature: [`FEAT-P0-01`](../features/FEAT-P0-01.md)
Introduced in: [`session/hand-2026-08-01/02C-next-session-mandate-the-dashboard-moves-itself.md`](../../session/hand-2026-08-01/02C-next-session-mandate-the-dashboard-moves-itself.md) §2 Steps 1 and 3; registered as `LE-65`

## Description

`LE-65`, raised this morning against a four-day-old falsehood on the most-machine-checked page in
the project. [`STORY-P0-01-08`](STORY-P0-01-08.md)'s header read *Specified* from 2026-07-28 to
2026-08-01 while [`REPORT-2026-07-28-11`](../reports/REPORT-2026-07-28-11.md) — filed by the
Story's own delivery session, linked from the Story's own Reports section — recorded **Pass, all
five clauses**. Every gate stayed green the whole time, because every gate compares *sideways*:
the badge gate compares the dashboard to the header, the `LE-44` gate compares the Feature table
to the header, and nothing compares the header to the Story's own evidence. Header, table cell
and badge were wrong *together*, in agreement.

The refusable class is narrower than `LE-65`'s owner-path proposed, and finding that is part of
this Story's work. The proposal was to refuse `Specified` **and** `In progress` headers with a
passing Report — but `In progress` beside a passing Report is *legitimate current state* in this
repository: [`REPORT-2026-07-30-01`](../reports/REPORT-2026-07-30-01.md) records **PASS on all
four** of its mandate's criteria while its `FEAT-P2` Stories deliberately stay `In progress`,
because their `D23`/`D14` performance numbers are stated open debt. A gate that refused that
would be refusing honesty. What is *never* legitimate is `Specified` — "not started" — above a
filed Report recording a pass: the Report's existence contradicts the state outright, whatever
fraction of the Story it covers.

## Depends on

[`STORY-P0-01-07`](STORY-P0-01-07.md) — the `Status:` vocabulary and header validation this gate
extends. The report-side join reuses the `Test(s) covered:` field that report coverage already
validates.

## Acceptance criteria

1. **A `Specified` Story header with a passing linked Report is refused.**
   `check-assurance-spine` reads every Report's `Test(s) covered:` field (the join that report
   coverage already enforces), resolves the covered Story — directly, or through a covered Test's
   id — reads the Report's `## Result` opener, and refuses the tree when a Story whose header
   state is `Specified` has any Report recording a pass. The error names the Story, its state,
   the Report, and the fix direction: re-verify the evidence, then advance the header, per
   Handover 35's verify-don't-inherit rule.
2. **`In progress` is deliberately not refused, and the reason is recorded.** The
   `REPORT-2026-07-30-01` precedent above, restated in the gate's own documentation so a later
   reader does not "complete" the check into refusing honest partial delivery.
3. **Only an unambiguous pass triggers.** The verdict is read from the first bolded token after
   the `## Result` heading; `Pass`/`PASS` triggers, anything else — including a Report with no
   `## Result` section, which is what the 2026-07-26 generation of Reports looks like — extracts
   no verdict and refuses nothing. A gate that guessed at prose would generate false refusals,
   and a false refusal teaches people to bypass gates.
4. **Every refusal is demonstrated.** The committed tree passes by construction (Step 1 already
   corrected the one live instance), so host tests drive: the `-08` incident's shape refused
   (Specified + linked passing Report), the same Report beside a `Functionally Verified` header
   accepted, `In progress` beside a passing Report accepted, a Specified Story with a resultless
   Report accepted, a Report covering the Story via its Test id refused all the same — each
   refusal with its acceptance case beside it.

## Named debt this Story leaves open

- **The verdict grammar is one bolded opener.** A Report that records a pass in any other shape
  is invisible to this gate; the Report schema, not this gate, is where a stricter contract
  belongs if one is ever wanted.
- **`In progress` headers can still sit above fully-passing Reports.** Deliberate, per criterion
  2 — distinguishing "all clauses passed" from "the delivered half passed" is a prose judgment
  this gate refuses to guess at.
- **`LE-34` remains untouched**, as it was by `-08` and `-09`.
- **No performance guardrail closes and no Story's assurance state moves.**

## Tests

[`TEST-P0-01-10-A`](../tests/TEST-P0-01-10-A.md) — written before implementation, per the TDD
mandate.

## Reports

- [`REPORT-2026-08-01-02`](../reports/REPORT-2026-08-01-02.md)
