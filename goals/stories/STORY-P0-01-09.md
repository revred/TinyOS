# STORY-P0-01-09 — The Numerics That Survived `-08` Become Generated or Gated

Status: **Functionally Verified (Host), 2026-08-01** — assurance state `baseline-debt`; delivered by [`REPORT-2026-08-01-01`](../reports/REPORT-2026-08-01-01.md), Pass on all five clauses, with the header advanced in the delivery commit itself — the moral of `LE-65`, found the same day
Feature: [`FEAT-P0-01`](../features/FEAT-P0-01.md)
Introduced in: [`session/hand-2026-08-01/02C-next-session-mandate-the-dashboard-moves-itself.md`](../../session/hand-2026-08-01/02C-next-session-mandate-the-dashboard-moves-itself.md) §2 Step 2

## Description

[`STORY-P0-01-08`](STORY-P0-01-08.md) deliberately generated only the *Assurance release status*
tiles and gated only the spine-count sentence, the loose-end count, and the Story badges. Every
numeric that remained hand-written has since drifted — on 2026-08-01, the very day of a hand
refresh: the tabstrip said "2 decomposed / 30 open" against a register at 3-plus-partial / 37, and
at the moment this Story was written the tabstrip *still* said "3 decomposed" while the tile above
it said 4. That is `LE-30`'s failure mode surviving in the parts `LE-30`'s closure did not cover.

This Story finishes the job for the numerics while leaving the argument human. Each in-scope item
is either generated into a marked region or extracted-and-gated like the count sentence; the
choice is stated per item, with the reason:

1. **The four *Overall progress* tiles** (Epics decomposed, Features, Stories functionally
   verified, Test docs) — **generated**, into a second marked region. They are pure `list-status`
   plus spine arithmetic with no editorial content; the same argument that generated the
   assurance tiles applies unchanged. The old tile's "(P2 partial)" parenthetical is an editorial
   judgment, not a statistic, so it moves to the prose beside the tiles rather than being emitted.
2. **The tabstrip counts** ("Epics *N* decomposed", "Loose ends *N* open") — **gated**. One number
   inside a one-line label is cheaper to extract than to generate, and the label's markup is
   layout a generator has no business owning.
3. **The progress bar width** — **gated**, derived from the functionally-verified Stories ratio
   (integer-rounded percentage). The alternative was deletion; it stays because, derived, it is a
   statistic rather than a decoration pretending to be one.
4. **The "Counted from `xtask list-status` on \<date\>" footnote** — the four state counts in it
   are **gated**; the date stays editorial. An emitter-stamped date would fail the byte-compare on
   every day boundary with no content change, which is a false refusal; the counts moving forces
   the sentence to be re-edited, which moves the date with it.
5. **The Epic-decomposition claims** ("The Epic denominator is now *N*") — **gated against the
   Epics on disk**. The roadmap population is the union of `EPIC-P*` documents under
   `goals/epics/` and the `EPIC-P*` rows of the backlog's phase table; horizon Epics (`EPIC-H*`)
   are excluded because [`backlog.md`](../epics/backlog.md) states they imply no sequence. An Epic
   counts as decomposed when at least one of its Stories carries a contract row. The next written
   Epic cannot leave the page claiming the old denominator.

**Explicitly out of scope, restating `-08`'s named debt because the temptation recurs:** the prose
argument, the per-Story tables, Report links, and the UPDATE narrative are editorial and stay
hand-written — the machine refuses their *claims* where extractable, it never writes their
*words*. Generating them wholesale would destroy the page's value; that trade was declined once
with reasons and the reasons hold.

## Depends on

[`STORY-P0-01-08`](STORY-P0-01-08.md) — the region/byte-compare and extract-and-gate machinery,
and the `emit-dashboard` command this Story extends.

## Acceptance criteria

1. **The Overall-progress tiles are generated, and the generator is the fix.**
   `cargo run -p xtask -- emit-dashboard` prints a second marked block — the four tiles computed
   from the same spine walk — and `check-assurance-spine` byte-compares the committed region
   against it, printing the expected block on mismatch. A missing begin marker and an unclosed
   region are refused with distinct messages. A CRLF checkout is not a defect.
   `emit-dashboard` still **must not** run the dashboard check (`-08` clause 1's reason: the
   command that prints the fix must not refuse to run when it is needed) and still does not write
   the file.
2. **The surviving numerics are gated, not generated.** The tabstrip's two counts, the progress
   bar's width percentage, the footnote's four state counts, and the Epic-denominator sentence are
   extracted and compared against the spine walk's own figures; the words around them are
   untouched. This Story does not acquire the power to rewrite the page's argument.
3. **The Epic population is derived from disk, not asserted.** Roadmap population and decomposed
   count as defined above, computed from `goals/epics/*.md` and the backlog phase table plus the
   Story contract rows — so writing `EPIC-P10.md` (or adding a backlog row) moves the denominator,
   and giving an Epic its first Story moves the numerator, with no human retyping either.
4. **Every refusal is demonstrated.** The committed tree satisfies all of the above by
   construction once fixed, so a green run is not evidence. Host tests drive each refusal — a
   stale generated tile, an absent region, an unclosed region, a stale tabstrip Epic count, a
   stale tabstrip loose-end count, a stale bar width, stale footnote state counts, a stale Epic
   denominator — each with an acceptance case beside it, `-08` clause 5's shape exactly.
5. **The committed page is corrected, not grandfathered.** The tabstrip's stale "3 decomposed" —
   live drift found while writing this Story — is fixed by the same change that lands the gate,
   for `STORY-P0-01-07`'s reason: exempting existing text makes the gate green on day one and
   blind to the drift it exists to catch.

## Named debt this Story leaves open

- **The badge vocabulary remains a hand-maintained mapping** (`-08`'s named debt, untouched):
  badge text is never derived by uppercasing header strings; a new state reaching the page must
  force a human to choose its wording.
- **The footnote gate assumes the four-state shape.** If a Story ever takes a state outside
  `Verified` / `Functionally Verified` / `Specified` / `In progress`, the gated sentence must be
  reshaped by hand — the gate will refuse until it is, which is the correct failure direction.
- **`LE-34` is untouched.** `README.md`'s v1 supported-set list is the same failure mode in a
  third document; this Story's shape transfers, the work does not.
- **`LE-65` is untouched.** Nothing here compares a Story's `Status:` header to its own filed
  Reports; this Story gates the page against the register, not the register against its evidence.
- **No performance guardrail closes and no Story's assurance state moves.**

## Tests

[`TEST-P0-01-09-A`](../tests/TEST-P0-01-09-A.md) — written before implementation, per the TDD
mandate.

## Reports

- [`REPORT-2026-08-01-01`](../reports/REPORT-2026-08-01-01.md)
