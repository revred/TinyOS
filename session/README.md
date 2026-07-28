# Session Handovers

This folder holds dated handover snapshots — one subfolder per work session that produces a meaningful state change, so anyone (human or AI) picking up the project can orient from a specific point in time rather than only from the current, ever-changing [`README.md`](../README.md).

## Naming convention

- One folder per calendar date: **`session/hand-YYYY-MM-DD/`** (e.g. `session/hand-2026-07-26/`) — not one folder per handover event.
- **Every handover produced on that date lives in that same folder, as its own numbered Markdown file:** `NN<Letter>-short-slug.md` (e.g. `38A-outstanding-actions.md`), numbered in chronological order and suffixed with a letter starting at `A`. Each file is a normal, self-contained handover write-up — what changed, key decisions made, open questions, immediate next steps — and should say what it follows (e.g. "Follows: `01-initial-handover.md`") so the sequence is readable end to end without needing the index.
- **The letter distinguishes multiple documents at the same number.** A handover claims `NNA`; a further document that belongs with it — a follow-on, a review of it, a second session's work on the same subject — takes `NNB`, then `NNC`, rather than consuming the next number. This keeps a subject's documents adjacent when sorted, and it gives two concurrent sessions a way to claim non-colliding slots at the same number instead of racing for the next one (see [`agent/CONCURRENT_SESSIONS.md`](../agent/CONCURRENT_SESSIONS.md) rule 4 — claim the slot by creating the file first).
- **Files numbered `NN-` with no letter predate this convention and keep their names.** They are referenced by filename from other dated documents, and those documents are never retroactively edited; renaming would leave dangling links inside an immutable record. Sorting is unaffected — `34-` still sorts before `38A-`.
- **`index.html` is the index of that date's handovers, not a handover itself.** It's a self-contained, dependency-free HTML page (readable directly in a browser, no build step) listing every `NN*-*.md` file produced that date in order (both the lettered and the pre-convention forms), each with a one-line summary and a link — plus, optionally, a quick document-reference table for fast navigation. It does not duplicate the `.md` files' content; if you want to know what actually changed, follow the link and read the `.md` file.
- Once a new calendar date's folder exists, earlier dated folders (and the individual `.md` handovers inside them) are never edited or deleted — they're a historical record. If something in an old handover turns out to be wrong or superseded, the newer handover says so explicitly; it doesn't get retroactively rewritten.

## Finding the latest handover

Sort `session/` subfolders by date descending, then open that folder's `index.html` — it lists every handover from that date in order; the last entry is the most recent state. [`README.md`](../README.md)'s "New here?" pointer always links to the latest date's `index.html` directly, so that's the fastest way to find it without sorting folders yourself.

## Relationship to other documents

- [`SeedMVP.md`](../SeedMVP.md) is the founding intent and comprehensive master specification — it changes slowly and deliberately.
- [`README.md`](../README.md) is the current-state living design document — it changes continuously as decisions are made.
- A `session/hand-*/index.html` is a **snapshot**, not a source of truth — if it disagrees with the README or the seed document, they win. A handover exists to orient a new reader quickly, not to replace the documents it points to.
