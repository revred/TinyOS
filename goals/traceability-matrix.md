# Traceability Matrix

Status: **Living document — update in the same PR that adds or changes any Story or Test**

The single-page cross-reference between Goals, the V&V hierarchy, and execution state. This table summarizes what the individual files under [`goals/`](README.md) already say in full — if this table and an individual file disagree, the individual file is authoritative and this table is stale; fix the table.

## EPIC-P0 (Kernel Skeleton) — fully decomposed

| Goal | Epic | Feature | Story | Test | Report | Status |
|---|---|---|---|---|---|---|
| G-RT-7, G-HW-4 (partial) | `EPIC-P0` | `FEAT-P0-01` | `STORY-P0-01-01` | `TEST-P0-01-01-A` | — | Planned |
| G-DX-5, G-DX-6 | `EPIC-P0` | `FEAT-P0-01` | `STORY-P0-01-02` | `TEST-P0-01-02-A` | — | Planned |
| G-DX-3 | `EPIC-P0` | `FEAT-P0-01` | `STORY-P0-01-03` | `TEST-P0-01-03-A` | — | Planned |
| G-RT-1 | `EPIC-P0` | `FEAT-P0-02` | *(not yet decomposed)* | — | — | Planned |
| G-RT-2 | `EPIC-P0` | `FEAT-P0-03` | *(not yet decomposed)* | — | — | Planned |
| G-HW-4 | `EPIC-P0` | `FEAT-P0-04` | *(not yet decomposed)* | — | — | Planned |

## EPIC-P1 through EPIC-P8 — backlog, not yet decomposed

See [`goals/epics/backlog.md`](epics/backlog.md) for the full Goal-to-Epic mapping for the remaining Roadmap phases. No Features, Stories, or Tests exist for these yet.

## Session cross-reference

| Session handover | What it changed in `goals/` |
|---|---|
| [`session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md`](../session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md) | Created the entire V&V model: this folder, `EPIC-P0` fully decomposed, `EPIC-P1`–`EPIC-P8` stubbed in `epics/backlog.md`. |

## How to keep this in sync

1. Adding a new Story or Test: add a row here (or extend an existing Feature's rows) in the same PR.
2. A Test's Status changes (e.g. first pass, or a regression): update the Status column and add the Report row in the "Session cross-reference" table if a handover discusses it, in the same PR that adds the Report file under `goals/reports/`.
3. Decomposing a backlog Epic into Features/Stories: move it out of the "not yet decomposed" section into its own fully-populated block, matching the `EPIC-P0` pattern above.
