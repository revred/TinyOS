# Epic Backlog — EPIC-P1 through EPIC-P8

Status: **Planned, not yet decomposed**

These Epics exist so the full Roadmap has a placeholder in the V&V model, but none are decomposed into Features/Stories yet — per the [goals dashboard](../index.html#jit-decomposition), decomposition happens when work on an Epic is about to start. Each row below is a direct restatement of [`SeedMVP.md`](../../SeedMVP.md#10-roadmap-alignment) Section 10's Roadmap Alignment table — this file does not add new information, it gives each phase an Epic ID so it can be referenced from Stories/Tests once decomposition begins.

| Epic | Roadmap phase | Goals to verify | Hardware | Depends on |
|---|---|---|---|---|
| `EPIC-P1` | Phase 1 — Determinism proof | G-RT-1, G-RT-3, G-PA-1 | Both MVP boards | `EPIC-P0` |
| `EPIC-P1_5` | Phase 1.5 — Deploy tooling | G-RC-6, G-DX-3 | Both MVP boards | `EPIC-P0` |
| `EPIC-P2` | Phase 2 — Shell & UX | G-RT-5, G-RT-6 | Both MVP boards | `EPIC-P0` |
| `EPIC-P3` | Phase 3 — Connectivity | G-HW-2, G-PA-4 | Both MVP boards + peripheral hardware | `EPIC-P0`, `EPIC-P2` |
| `EPIC-P4` | Phase 4 — Host bridge | G-RC-1, G-RC-2 | x86_64 mini-PC | `EPIC-P0` |
| `EPIC-P5` | Phase 5 — Agent Command Interface | G-AI-2 – G-AI-5, G-RC-2, G-RC-3 | Both MVP boards | `EPIC-P4` |
| `EPIC-P6` | Phase 6 — LLM integration | G-AI-1, G-AI-2, G-AI-3 | Jetson Orin Nano Super | `EPIC-P5` |
| `EPIC-P6B` | Phase 6b — Heterogeneous compute | G-AI-6, G-HW-6 | Jetson Orin Nano Super | `EPIC-P6` |
| `EPIC-P7` | Phase 7 — Edge bring-up | G-HW-1 – G-HW-5 | Jetson Orin Nano Super | `EPIC-P0`, `EPIC-P6B` |
| `EPIC-P8` | Phase 8 — Fleet mode | G-RC-4, G-AI-7 | Multiple units of both MVP board types | `EPIC-P4`, `EPIC-P5` |

The **5-axis CNC flagship milestone** (G-PA-8) is not a single row here — per `SeedMVP.md` Section 10, it's a cross-cutting integration checkpoint spanning `EPIC-P0` through `EPIC-P3` (scheduler, shell/G-code front-end, and connectivity all have to land first). When `motion` (see `docs/mvp-delivery-strategy.md`) reaches the point of active development, it should get its own Feature under whichever Epic is current at that time, cross-referencing all of `EPIC-P0`–`EPIC-P3`, rather than being forced into a single Epic it doesn't cleanly belong to.

To promote a row here to a full Epic file: copy the pattern in [`EPIC-P0.md`](EPIC-P0.md), decompose it into Features and Stories, and update this table's status.
