# Session Handovers

This folder holds dated handover snapshots — one subfolder per work session that produces a meaningful state change, so anyone (human or AI) picking up the project can orient from a specific point in time rather than only from the current, ever-changing [`README.md`](../README.md).

## Naming convention

- One folder per calendar date: **`session/hand-YYYY-MM-DD/`** (e.g. `session/hand-2026-07-26/`) — not one folder per handover event.
- **Every handover produced on that date lives in that same folder, as its own numbered Markdown file:** `NN<Letter>-short-slug.md` (e.g. `38A-outstanding-actions.md`), numbered in chronological order and suffixed with a letter starting at `A`. Each file is a normal, self-contained handover write-up — what changed, key decisions made, open questions, immediate next steps — and should say what it follows (e.g. "Follows: `01-initial-handover.md`") so the sequence is readable end to end without needing the index.
- **The letter identifies the *session*, not the document.** Amended 2026-07-29, and this is the important half. A session claims **one letter** when it starts — `A` if it is alone, otherwise the first letter no other session is using that date — and uses it for **every** document it files that day: `39A`, `40A`, `41A` for one session while a concurrent one writes `39B`, `40B`. Collision then becomes *impossible* rather than merely unlikely, and a day's folder reads cleanly per session instead of interleaved.

  **The convention originally read "the letter distinguishes multiple documents at the same number", and that failed twice on its first day.** Both live sessions independently picked `A` — for slot `39A` and again for `41A` — because a per-*document* letter gives two sessions no reason to differ. The second collision reached the machine-readable register, where `LE-47` and `LE-48` came to cite `hand-2026-07-28/41A` while meaning two different files (`LE-51`). A per-session letter has no such failure mode: the letter is chosen once, against what other sessions hold, not per document against nothing.

  A follow-on or a review by the *same* session simply takes its own next number with its own letter. If you genuinely need a second document at one number — a review of `39A` filed by the session that wrote it — `39A-…` and a distinct slug is enough, and the register cites the slug-bearing filename rather than the bare slot.
- **Files numbered `NN-` with no letter predate this convention and keep their names.** They are referenced by filename from other dated documents, and those documents are never retroactively edited; renaming would leave dangling links inside an immutable record. Sorting is unaffected — `34-` still sorts before `38A-`.
- **`index.html` is the index of that date's handovers, not a handover itself.** It's a self-contained, dependency-free HTML page (readable directly in a browser, no build step) listing every `NN*-*.md` file produced that date in order (both the lettered and the pre-convention forms), each with a one-line summary and a link — plus, optionally, a quick document-reference table for fast navigation. It does not duplicate the `.md` files' content; if you want to know what actually changed, follow the link and read the `.md` file.
- Once a new calendar date's folder exists, earlier dated folders (and the individual `.md` handovers inside them) are never edited or deleted — they're a historical record. If something in an old handover turns out to be wrong or superseded, the newer handover says so explicitly; it doesn't get retroactively rewritten.

## Finding the latest handover

Sort `session/` subfolders by date descending, then open that folder's `index.html` — it lists every handover from that date in order; the last entry is the most recent state. [`README.md`](../README.md)'s "New here?" pointer always links to the latest date's `index.html` directly, so that's the fastest way to find it without sorting folders yourself.

## Relationship to other documents

- [`SeedMVP.md`](../SeedMVP.md) is the founding intent and comprehensive master specification — it changes slowly and deliberately.
- [`README.md`](../README.md) is the current-state living design document — it changes continuously as decisions are made.
- A `session/hand-*/index.html` is a **snapshot**, not a source of truth — if it disagrees with the README or the seed document, they win. A handover exists to orient a new reader quickly, not to replace the documents it points to.
