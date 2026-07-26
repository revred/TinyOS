# Session Handovers

This folder holds dated handover snapshots — one subfolder per work session that produces a meaningful state change, so anyone (human or AI) picking up the project can orient from a specific point in time rather than only from the current, ever-changing [`README.md`](../README.md).

## Naming convention

- One folder per calendar date: **`session/hand-YYYY-MM-DD/`** (e.g. `session/hand-2026-07-26/`) — not one folder per handover event.
- **Every handover produced on that date lives in that same folder.** If several handovers happen on the same day, they all update the one dated folder rather than spawning new sibling folders — `index.html` always reflects the latest, cumulative state for that date. (If it's ever useful to preserve an individual same-day snapshot rather than let it be absorbed into the running `index.html`, add it as an extra file inside that day's folder, e.g. `snapshot-1430.html` — the folder is the unit of "this date," not a single write.)
- `index.html` is a self-contained, dependency-free HTML page: what changed, key decisions made, open questions, and immediate next steps. It should be readable directly (open the file in a browser) without any build step.
- Once a new calendar date's folder exists, earlier dated folders are never edited or deleted — they're a historical record. If something in an old handover turns out to be wrong or superseded, the newer handover says so explicitly; it doesn't get retroactively rewritten.

## Finding the latest handover

Sort `session/` subfolders by date descending; the most recent `hand-YYYY-MM-DD/index.html` is the current snapshot. [`README.md`](../README.md)'s "New here?" pointer always links to the latest one directly, so that's the fastest way to find it without sorting folders yourself.

## Relationship to other documents

- [`TinyOS26thJulySeedMVP.md`](../TinyOS26thJulySeedMVP.md) is the founding intent and comprehensive master specification — it changes slowly and deliberately.
- [`README.md`](../README.md) is the current-state living design document — it changes continuously as decisions are made.
- A `session/hand-*/index.html` is a **snapshot**, not a source of truth — if it disagrees with the README or the seed document, they win. A handover exists to orient a new reader quickly, not to replace the documents it points to.
