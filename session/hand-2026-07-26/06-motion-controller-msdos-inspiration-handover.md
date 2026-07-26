# Handover 06 — Acknowledging `work/case-motion-controller` and the MsDOS Inspiration Angle

Session date: 26 July 2026
Follows: [05-goals-dashboard-handover.md](05-goals-dashboard-handover.md)

## Why this handover exists

A direct check: does [Handover 01](01-initial-handover.md) — the first handover of the day — acknowledge the work delivered under [`work/case-motion-controller/`](../../work/case-motion-controller/), and does it point to [`MsDOS/`](../../MsDOS) as a source of inspiration for that work?

**No, on both counts.** Handover 01 was written before `work/case-motion-controller/` existed (that folder was created across the two requests that produced Handovers that came later than 01, none of which retroactively touched 01's body). Its only mention of `MsDOS/` is the generic line already in its "What exists today" table: *"Microsoft's officially released MS-DOS source, kept for historical command-behavior reference only — not built upon."* That's accurate as a general statement but says nothing about the motion-controller case specifically.

Per [`session/README.md`](../README.md)'s convention, earlier handovers are historical records, not living documents — 01 isn't retroactively rewritten to include this. This handover is the acknowledgment instead, and the connection is also now written into the actual deliverable (not just a session note) — see below.

## What changed

Added a new paragraph to [`work/case-motion-controller/README.md`](../../work/case-motion-controller/README.md)'s "Relationship to the rest of the project" section, making explicit what was previously only implicit: `atomic-features.md`'s modern-UX category (GPU-composited touch UI, live 3D toolpath preview, voice intake) is a layer built *on top of*, not a replacement for, the same DOS-heritage `TINYCMD` baseline every other TinyOS surface uses. The `MsDOS/` submodule is cited there for its **legibility-under-pressure design ethos** — a shop-floor operator needs a command path that works even when the touch layer isn't the fastest way to stop a running program — explicitly *not* as a source of code or UI layout to copy, consistent with how `MsDOS/` has been scoped everywhere else in this project (`docs/cli-compatibility-mvp.md`'s reference note, the README's Repository Layout entry).

## Summary of what `work/case-motion-controller/` now contains

For anyone landing on this handover without having read the intervening ones:

| File | Content |
|---|---|
| [`README.md`](../../work/case-motion-controller/README.md) | Case overview, folder index, the note on why the reference manuals aren't committed, and (as of this handover) the DOS-heritage/MsDOS connection |
| [`references.md`](../../work/case-motion-controller/references.md) | Citations for both reference manuals (Fanuc Series 30i-B Plus manual, the Manual Guide milling/turning conversational-programming guide) by title/URL |
| [`requirements.md`](../../work/case-motion-controller/requirements.md) | R1–R5 functional requirements, grounded against a real owned Fanuc-controlled machine |
| [`test-cases.md`](../../work/case-motion-controller/test-cases.md) | 8 given/when/then test cases (TC-1 through TC-8) |
| [`user-stories.md`](../../work/case-motion-controller/user-stories.md) | 15 user stories (US-1 through US-15) plus the flagship demo script |
| [`atomic-features.md`](../../work/case-motion-controller/atomic-features.md) | 86 atomic OS-level features across 10 categories, produced via a delegated subagent, for a third-party developer building a modern conversational G-code app on TinyOS |

The reference manuals themselves remain local-only (`.gitignore`-excluded), per the standing rule established the first time this pattern came up.

## Next handover

None yet filed past this point at time of writing. See [`index.html`](index.html) for the running index of this date's handovers.
