# Handover 04 — SeedMVP Rename, agent.md, and the Goals V&V Model

Session date: 26 July 2026
Follows: [03-cnc-kinematics-merge-handover.md](03-cnc-kinematics-merge-handover.md)

## What changed

1. **`TinyOS26thJulySeedMVP.md` renamed to [`SeedMVP.md`](../../SeedMVP.md).** Every reference across the repository (README, docs/*, session handovers) updated to the new filename. Section 12 (Cross-Reference Index) refreshed to list `agent.md` and `goals/` alongside the existing document map.
2. **New: [`agent.md`](../../agent.md)** — the single, tool-agnostic entry point for any coding agent working in this repository (a generic equivalent of a tool-specific `CLAUDE.md`). Points to `SeedMVP.md`, `README.md`, `agent/CODING_STANDARDS.md`, the latest session handover, and `goals/`, in that order, plus restates the rules that never bend (priority ordering, no privileged bypass, mandatory TDD, the 20K-LOC crate ceiling, SOLID enforcement, fail-safe behavior, and the `os/src/` code-location rule).
3. **New: [`goals/`](../../goals/) — a Verification & Validation model.** Goals (from `SeedMVP.md` §3) decompose into Epics (mapped 1:1 to Roadmap phases) → Features → Stories → Tests → Reports, cross-referenced against session handovers. `EPIC-P0` (Kernel skeleton) is fully decomposed as the worked reference example: 4 Features, 3 Stories (for `FEAT-P0-01`, the walking skeleton), 3 Test specifications. `EPIC-P1` through `EPIC-P8` are stubbed in `goals/epics/backlog.md`, deliberately not pre-decomposed — per the model's own "just-in-time decomposition" principle, an Epic is broken into Features/Stories only when work on it is about to start.

## Key decisions made this handover

- **`agent.md` (file) vs `agent/` (folder) coexist deliberately.** They are different filenames, not a collision — `agent.md` is documented as pointing to `agent/CODING_STANDARDS.md`, not duplicating it.
- **Epic IDs are named after Roadmap phases** (`EPIC-P0`, `EPIC-P1`, `EPIC-P1_5`, ... `EPIC-P8`), not arbitrary sequence numbers, so an Epic ID always tells you which Roadmap phase and which row of `SeedMVP.md` Section 10 (Roadmap Alignment) to check.
- **A Story is only "Verified" once a Report exists confirming a passing Test** — implementation alone never earns that status, consistent with the existing TDD mandate.
- **The traceability matrix (`goals/traceability-matrix.md`) is a living summary, not a second source of truth** — if it disagrees with an individual Epic/Feature/Story/Test file, the individual file wins, and the matrix should be fixed in the same PR.

## Documents touched

- Renamed: `TinyOS26thJulySeedMVP.md` → `SeedMVP.md`
- New: `agent.md`
- New: `goals/README.md`, `goals/traceability-matrix.md`, `goals/epics/EPIC-P0.md`, `goals/epics/backlog.md`, `goals/features/FEAT-P0-01.md` through `FEAT-P0-04.md`, `goals/stories/STORY-P0-01-01.md` through `STORY-P0-01-03.md`, `goals/tests/TEST-P0-01-01-A.md` through `TEST-P0-01-03-A.md`, `goals/reports/README.md`
- Updated: `README.md` (Repository Layout, "New here?" pointer), `docs/mvp-delivery-strategy.md` (top-level structure diagram), all files referencing the old seed-doc filename

## Next handover

None yet filed past this point at time of writing. See [`index.html`](index.html) for the running index of this date's handovers.
