# Handover 02A — `goals/index.html` Composed: Four Tabs, Ten Collapsible Sections, Two Stale Claims Removed

Presentation and accuracy only. **No code, no contracts, no register change.** Follows
[`01`](01-story-p0-07-03-grant-fails-closed.md), whose Story this session recovered and reviewed rather
than wrote.

## Why

The page was **810 lines and one flat scroll** — nine `<h2>` sections, the loose-ends register buried
two thirds of the way down inside `EPIC-P1`'s section, and the reference material (backlog, the
assurance model, folder layout) taking up as much vertical space as the Epics anyone actually opens the
page for. On the owner's instruction: sections that expand and contract, and loose ends and other
non-core-goal material moved into a tab.

## What it is now

**Four tabs**, CSS-only — radio inputs a label toggles, panels revealed by a sibling combinator:

| Tab | Holds |
| --- | --- |
| **Progress** | Overall progress, Assurance release status, Whole-system flight plan |
| **Epics** | `EPIC-P0`, `EPIC-P1` — the two decomposed |
| **Loose ends** | The readable view of the register, **lifted out of `EPIC-P1`'s section**, where it did not belong |
| **Reference** | Backlog, the model, the four planes, folder layout |

**Ten collapsible sections**, native `details`/`summary`, each heading preserved verbatim inside its
`summary` so the badges and anchor text are unchanged. The section a reader most likely wants in each
tab starts open; the rest start closed.

**No JavaScript and no build step**, deliberately — the file still opens straight from disk, which is
[`session/README.md`](../README.md)'s dependency-free rule applied to this page. It also degrades
correctly: with CSS unavailable it is the same flat document it was before, because `details` renders
open and every panel is a plain `<section>`.

## The three invariants this could have broken, and how they were held

`LE-30`'s gate made parts of this page machine-checked, so a restructure is no longer only a matter of
taste. All three were verified **after** the change, not assumed:

1. **The generated block.** `<!-- BEGIN GENERATED stat-row … -->` … `<!-- END GENERATED stat-row -->`
   moved as one unit, byte for byte. `check-assurance-spine` compares it against
   `xtask emit-dashboard` and would have refused.
2. **The two gated count strings** — `23 Features / 63 Stories / 50 Tests / 51 Reports` and
   `52 loose ends (30 open)` — present and unedited.
3. **The badges.** `51 dashboard badges agree` passes, and the raw count of `class="badge`
   occurrences is **74 before and 74 after**. No badge was added, removed or reworded — which is also
   why the *"Backlog — `EPIC-P1_5` through `EPIC-P8`"* heading keeps its now-imprecise
   `NOT DECOMPOSED` badge rather than being tidied: the Reference tab's note carries the correction
   instead.

Content preservation was checked structurally rather than by eye: `<table>` 7 → 7, `<tr>` 57 → 57,
`<h3>` 5 → 5, `<pre>` 1 → 1. `<p>` rose 46 → 50 (four tab notes) and `<h2>` 9 → 10 (the new Loose ends
heading). Every `details`/`summary`/`section`/`nav` tag balances.

## Two stale claims removed, which is the part that mattered more than the layout

Composing the page meant reading it, and it was asserting two things that are no longer true:

- **"`LE-30`… is still open."** It closed on 2026-07-28 (`42A`). The paragraph now says what actually
  changed: the tiles are generated and the counts are gated, so the *numbers* cannot drift silently —
  **but every claim in prose still can**, which is the honest version of that sentence.
- **"Last updated: 28 July … at 23 Features / 59 Stories / 46 Tests / 47 Reports"**, and *"the eighth
  consecutive session to hand-edit this page"*. Both superseded. It now dates itself 29 July, names
  which part is generated, and says plainly that **everything else is hand-written** — including the
  four tiles directly beneath it.

**Those four tiles were themselves stale**, and they are the ones a reader trusts first because they
are at the top: `41 / 49` Stories functionally verified and `46` Test docs. Recounted from
`xtask list-status`: **35 `Verified` + 9 `Functionally Verified` of 63**, 17 `Specified`, 2
`In progress`, and 50 Test documents. The tiles now read `44 / 63` and `50`, the progress bar moved
81% → 70%, and a line beneath them states they are hand-written and ungated — because the generated
block sits in a *different* section and a reader has no way to tell which is which otherwise.

That distinction is the one thing worth carrying forward from this change: **`LE-30` gated the numbers
it generates, not the numbers next to them.**

## State

```text
main                    82e3d57 + this commit; 1 ahead of origin before it
                        (main was pushed mid-session by someone — it was 17 ahead)
goals/index.html        810 -> 913 lines, 4 tabs, 10 collapsible sections
gates                   check-assurance-spine, check-spine-files, catalogue,
                        crate sizes, xtask 195/0 — all green after the change
spine                   unchanged: 23 Features, 63 Stories, 50 Tests, 51 Reports,
                        52 loose ends (30 open), 90 status headers
```

Working tree was **completely clean** when this started — the first time today, and the soak log
finally landed in `49e5b08`.
