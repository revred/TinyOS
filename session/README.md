# Session Handovers

This folder holds dated handover snapshots — one subfolder per work session that produces a meaningful state change, so anyone (human or AI) picking up the project can orient from a specific point in time rather than only from the current, ever-changing [`README.md`](../README.md).

## Naming convention

- One folder per calendar date: **`session/hand-YYYY-MM-DD/`** (e.g. `session/hand-2026-07-26/`) — not one folder per handover event.
- **Every handover produced on that date lives in that same folder, as its own numbered Markdown file:** `NN-short-slug.md` (e.g. `01-initial-handover.md`, `02-mvp-delivery-strategy-handover.md`), numbered in chronological order starting at `01`. Each file is a normal, self-contained handover write-up — what changed, key decisions made, open questions, immediate next steps — and should say what it follows (e.g. "Follows: `01-initial-handover.md`") so the sequence is readable end to end without needing the index.
- **`index.html` is the index of that date's handovers, not a handover itself.** It's a self-contained, dependency-free HTML page (readable directly in a browser, no build step) listing every `NN-*.md` file produced that date in order, each with a one-line summary and a link — plus, optionally, a quick document-reference table for fast navigation. It does not duplicate the `.md` files' content; if you want to know what actually changed, follow the link and read the `.md` file.
- Once a new calendar date's folder exists, earlier dated folders (and the individual `.md` handovers inside them) are never edited or deleted — they're a historical record. If something in an old handover turns out to be wrong or superseded, the newer handover says so explicitly; it doesn't get retroactively rewritten.

## Finding the latest handover

Sort `session/` subfolders by date descending, then open that folder's `index.html` — it lists every handover from that date in order; the last entry is the most recent state. [`README.md`](../README.md)'s "New here?" pointer always links to the latest date's `index.html` directly, so that's the fastest way to find it without sorting folders yourself.

## Relationship to other documents

- [`SeedMVP.md`](../SeedMVP.md) is the founding intent and comprehensive master specification — it changes slowly and deliberately.
- [`README.md`](../README.md) is the current-state living design document — it changes continuously as decisions are made.
- A `session/hand-*/index.html` is a **snapshot**, not a source of truth — if it disagrees with the README or the seed document, they win. A handover exists to orient a new reader quickly, not to replace the documents it points to.
