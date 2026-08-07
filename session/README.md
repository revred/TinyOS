# Session Handovers

This folder holds dated handover snapshots — one subfolder per work session that produces a meaningful state change, so anyone (human or AI) picking up the project can orient from a specific point in time rather than only from the current, ever-changing [`README.md`](../README.md).

## Naming convention

- One folder per calendar date: **`session/hand-YYYY-MM-DD/`** (e.g. `session/hand-2026-07-26/`) — not one folder per handover event.
- **Every handover produced on that date lives in that same folder, as its own numbered Markdown file:** `NN<Letter>-short-slug.md` (e.g. `38A-outstanding-actions.md`), numbered in chronological order and suffixed with a letter starting at `A`. Each file is a normal, self-contained handover write-up — what changed, key decisions made, open questions, immediate next steps — and should say what it follows (e.g. "Follows: `01-initial-handover.md`") so the sequence is readable end to end without needing the index.
- **The letter marks concurrency at a number — nothing else.** Amended 2026-08-07 by the owner, superseding the 2026-07-29 letter-per-session rule below. A session filing a handover takes the **next free number with the letter `A`**. A letter past `A` exists only when another session is live in the tree at the same moment: the concurrent session files at the **same number** with `B` (a third with `C`), so `05A`/`05B` are two sessions writing side by side. The moment the concurrency ends, the letter resets — after `05A`/`05B` the next session starts **`06A`, never `06C`**. A letter never carries across numbers as a session's identity, and a date whose letters climb (`…06D`, `07F`, `08G`) is this rule being broken — which is exactly how 2026-08-07's folder read before the owner corrected it.

  **What survives from the 2026-07-28 collisions (`LE-51`):** two *live* sessions must still choose **different** letters at the same number, each against what the other holds, before writing content (claim the file first — `CONCURRENT_SESSIONS.md` rule 4). What changed on 2026-08-07 is the letter's scope: it disambiguates one number's concurrent writers, it does not name a session for the day. The 2026-07-29 rule ("a session claims one letter and keeps it all date") fixed the collision but made letters escalate monotonically across a date — by 2026-08-07 the seventh session was filing `08G` while alone in the tree, which reads as seven-way concurrency that never happened.

  A follow-on or a review by the same session takes the next number, at `A` again unless someone is concurrent there. If you genuinely need a second document at one number from one session, a distinct slug at the same `NNA-…` is enough, and the register cites the slug-bearing filename rather than the bare slot.
- **Documents filed under the superseded letter-per-session rule (2026-07-29 → 2026-08-07) keep their names.** They are cited by filename from the machine-checked registers and from other dated documents; renaming would dangle those links, for the same reason the pre-convention `NN-` files below keep theirs. One exception was safe and was taken: `hand-2026-08-07/08A` was filed as `08G` and renamed the same day by the session that wrote it, inside the still-current folder, with its citations repaired in the same commit.
- **Files numbered `NN-` with no letter predate this convention and keep their names.** They are referenced by filename from other dated documents, and those documents are never retroactively edited; renaming would leave dangling links inside an immutable record. Sorting is unaffected — `34-` still sorts before `38A-`.
- **`index.html` is the index of that date's handovers, not a handover itself.** It's a self-contained, dependency-free HTML page (readable directly in a browser, no build step) listing every `NN*-*.md` file produced that date in order (both the lettered and the pre-convention forms), each with a one-line summary and a link — plus, optionally, a quick document-reference table for fast navigation. It does not duplicate the `.md` files' content; if you want to know what actually changed, follow the link and read the `.md` file.
- Once a new calendar date's folder exists, earlier dated folders (and the individual `.md` handovers inside them) are never edited or deleted — they're a historical record. If something in an old handover turns out to be wrong or superseded, the newer handover says so explicitly; it doesn't get retroactively rewritten.

## Finding the latest handover

Sort `session/` subfolders by date descending, then open that folder's `index.html` — it lists every handover from that date in order; the last entry is the most recent state. [`README.md`](../README.md)'s "New here?" pointer always links to the latest date's `index.html` directly, so that's the fastest way to find it without sorting folders yourself.

## Relationship to other documents

- [`SeedMVP.md`](../SeedMVP.md) is the founding intent and comprehensive master specification — it changes slowly and deliberately.
- [`README.md`](../README.md) is the current-state living design document — it changes continuously as decisions are made.
- A `session/hand-*/index.html` is a **snapshot**, not a source of truth — if it disagrees with the README or the seed document, they win. A handover exists to orient a new reader quickly, not to replace the documents it points to.
