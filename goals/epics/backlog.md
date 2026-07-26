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

**Notes from 2026-07-26's strategic objectives** (not yet decomposed, recorded here so they aren't lost before either Epic is picked up):

- **`EPIC-P3` (Connectivity)** is where a real, network-facing TCP/IP stack belongs, once decomposed — layered above `G-HW-2`'s network-class-driver goal (a NIC class driver is the device-driver half; the protocol stack is a separate Feature above it). Explicitly **not** part of `EPIC-P0`'s `FEAT-P0-07` (local IPC), which is scoped to same-machine shared memory/message channels only — see `FEAT-P0-07.md`'s own scope-boundary note for why the two are kept apart.
- **`EPIC-P1_5` (Deploy tooling)** is where the sibling `Sharc.Blue` project's `blue.atom`/`blue-sharc.exe` tooling fits as prior art for speeding up TinyOS's own development/deployment onto devices like a Raspberry Pi 5 — per a 2026-07-26 strategic objective. `Sharc.Blue` itself has no existing Raspberry Pi/ARM64 cross-compile or deployment pipeline to import wholesale (checked directly against that repo); what's reusable is the *pattern* (a small, fast, single-binary CLI/atom-catalog front end driving build/deploy actions), not a ready-made tool. Raspberry Pi 5 is already named in `README.md`'s Target Hardware & Test Matrix as a Tier 1 ARM64 portability board, deferred to "Phase 3 onward" — worth reconciling that existing placement against this objective's urgency when `EPIC-P1_5` is actually decomposed, rather than assuming they're already aligned.

The **5-axis CNC flagship milestone** (G-PA-8) is not a single row here — per `SeedMVP.md` Section 10, it's a cross-cutting integration checkpoint spanning `EPIC-P0` through `EPIC-P3` (scheduler, shell/G-code front-end, and connectivity all have to land first). When `motion` (see `docs/mvp-delivery-strategy.md`) reaches the point of active development, it should get its own Feature under whichever Epic is current at that time, cross-referencing all of `EPIC-P0`–`EPIC-P3`, rather than being forced into a single Epic it doesn't cleanly belong to.

To promote a row here to a full Epic file: copy the pattern in [`EPIC-P0.md`](EPIC-P0.md), decompose it into Features and Stories, and update this table's status.
