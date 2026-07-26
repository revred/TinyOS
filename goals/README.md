# TinyOS Verification & Validation (V&V) Model

This folder is where TinyOS's goals become traceable, testable work — and stay traceable once they're built. It exists so that, at any point, the question "does this Story actually verify a real Goal, and has it actually been tested?" has a written, checkable answer instead of a remembered one.

## The model

```text
Goal  →  Epic  →  Feature  →  Story  →  Test  →  Report
 │                                        │
 └──────────── cross-referenced against Sessions ─────┘
```

| Level | What it is | Where it lives | Example ID |
|---|---|---|---|
| **Goal** | A concrete, falsifiable goal from [`SeedMVP.md`](../SeedMVP.md) Section 3 (Goal Taxonomy). Goals are not created here — they're defined in the master specification and referenced from here. | `SeedMVP.md` §3 (not duplicated in this folder) | `G-RT-1`, `G-PA-8`, `G-DX-5` |
| **Epic** | A body of work large enough to span multiple features, mapped 1:1 to a Roadmap phase from [`README.md`](../README.md#roadmap) — so "what does this Epic need to prove" always has a one-line answer tied to the phase it belongs to. | `goals/epics/EPIC-<phase>.md` | `EPIC-P0` (Kernel skeleton) |
| **Feature** | A coherent, independently describable capability within an Epic — usually maps closely to one crate or one clear sub-area of a crate from [`docs/mvp-delivery-strategy.md`](../docs/mvp-delivery-strategy.md). | `goals/features/FEAT-<epic>-<NN>.md` | `FEAT-P0-01` (Workspace bootstrap & walking skeleton) |
| **Story** | A single implementable, testable unit of work within a Feature — small enough to be built test-first in one sitting, per the TDD mandate. | `goals/stories/STORY-<feature>-<NN>.md` | `STORY-P0-01-01` |
| **Test** | The concrete test(s) that verify a Story. Before code exists, a Test entry is a *specification* of the test to be written first (red, per TDD); once implemented, it links to the actual test in the codebase. | `goals/tests/TEST-<story>-<letter>.md` | `TEST-P0-01-01-A` |
| **Report** | A record of a Test actually being run — result, environment/tier, date, and a link to the CI run or session where it happened. Filed when tests start executing, not before. | `goals/reports/` | `REPORT-2026-MM-DD-<NN>` |

## ID scheme

- **Epics** are named after the Roadmap phase they implement: `EPIC-P0`, `EPIC-P1`, `EPIC-P1_5`, `EPIC-P2`, ... `EPIC-P8`. This is deliberate — an Epic ID always tells you which Roadmap phase to check for schedule/hardware/test-tier context, per [`SeedMVP.md`](../SeedMVP.md#10-roadmap-alignment) Section 10.
- **Features** are `FEAT-<epic>-<NN>`, two-digit, sequential within the Epic: `FEAT-P0-01`, `FEAT-P0-02`, ...
- **Stories** are `STORY-<feature>-<NN>`, two-digit, sequential within the Feature: `STORY-P0-01-01`, `STORY-P0-01-02`, ...
- **Tests** are `TEST-<story>-<letter>`, uppercase letter, sequential within the Story (a Story can require more than one test): `TEST-P0-01-01-A`, `TEST-P0-01-01-B`, ...
- **Reports** are `REPORT-YYYY-MM-DD-<NN>`, sequential within the date.

## Status lifecycle

Every Epic, Feature, Story, and Test carries a `Status` field with one of these values:

- **Planned** — defined, not yet started.
- **In Progress** — actively being worked on.
- **Blocked** — cannot proceed; the blocking reason is stated explicitly (e.g. "waiting on MVP hardware purchase").
- **Verified** — the corresponding Test(s) have passed and a Report exists confirming it, in the relevant tier (see [Target Hardware & Test Matrix](../README.md#target-hardware--test-matrix)).

A Story is never marked **Verified** on the strength of its implementation alone — only a passing Test with a filed Report earns that status, consistent with [`agent/CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md#test-driven-development-mandatory)'s "correctness is proven through TDD, not asserted."

## Just-in-time decomposition, not upfront exhaustive planning

Only **EPIC-P0** is decomposed to Feature/Story/Test level today, as the worked reference example — it's also the next real work per the Roadmap. `EPIC-P1` through `EPIC-P8` exist as stub entries in [`goals/epics/backlog.md`](epics/backlog.md), each pointing at its Roadmap phase and the Goals it will need to verify (already listed in `SeedMVP.md` Section 10's Roadmap Alignment table), but are **not** pre-decomposed into Features and Stories. Decomposing every future Epic today would produce dozens of Story files describing work that hasn't been designed yet — speculative detail this project's own principles (see [`CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md) on not adding abstraction before it's needed) argue against. Decompose an Epic into Features/Stories when work on it is about to start, not before.

## Cross-referencing sessions and reports

- Every Epic/Feature/Story/Test file has an **"Introduced in"** field linking to the [`session/hand-YYYY-MM-DD/NN-*.md`](../session/) handover that created or last substantively changed it, so the history of *why* something is scoped the way it is stays attached to it.
- Every Test file has a **"Reports"** field listing every `REPORT-*` that has run it, most recent first — empty until the first run.
- [`goals/traceability-matrix.md`](traceability-matrix.md) is the single-page cross-reference: every Goal, which Epic/Feature/Story addresses it, its current Test/Report status, and which session introduced it. Update this file in the same PR that adds or changes any Story or Test — it should never be allowed to drift out of sync with the individual files it summarizes.

## Folder layout

```text
goals/
  README.md                    This file
  traceability-matrix.md       Master cross-reference table
  epics/
    EPIC-P0.md                 Fully decomposed (worked example)
    backlog.md                 EPIC-P1 through EPIC-P8, stubbed, not yet decomposed
  features/
    FEAT-P0-01.md ... 
  stories/
    STORY-P0-01-01.md ...
  tests/
    TEST-P0-01-01-A.md ...
  reports/
    README.md                  Report format/schema; empty until Phase 0 testing begins
```
