# TEST-P0-01-09-A — The Numerics That Survived `-08` Become Generated or Gated

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P0-01-09`](../stories/STORY-P0-01-09.md)
Tier: Host unit tests only — the subject is the agreement between two files in this repository, and no CPU this project lacks is involved
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`, `BND-18`
Protection Domain contracts: `PD-02`, `PD-12`, `PD-13`, `PD-14`
Code admission gates: `RCG-05`, `RCG-06`, `RCG-07`, `RCG-12`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

`STORY-P0-01-08` split [`goals/index.html`](../index.html) into generated tiles, gated claims,
and free prose — and stopped there on purpose, leaving the *Overall progress* numerics
hand-written. They drifted the very next time the register moved: on 2026-08-01 the tabstrip
claimed "2 decomposed / 30 open" against a register at 3-plus-partial / 37, and after that same
day's hand refresh the tabstrip still said "3 decomposed" under a tile saying 4. This document
specifies the machinery that removes the remaining hand-written numbers from the page while
leaving every word of its argument alone.

The design question, settled per item in the Story: tiles are generated (pure arithmetic),
inline counts are gated (one number inside human markup), and the Epic population is derived
from what is on disk rather than from anyone's memory of it.

Clause 4 carries forward `-08` clause 5 and `ADR 0005`'s trap: once this Story fixes the tree,
every check passes by construction, and a green `check-assurance-spine` is not evidence that any
of them can reject. The tests are the evidence.

## Specification

### 1. The Overall-progress tiles are generated, and the generator prints the fix

**Given** `cargo run -p xtask -- emit-dashboard`,
**then** it prints, in addition to `-08`'s stat-row block, an `overall-progress` marked block —
begin marker, four `<div class="stat">` tiles (Epics decomposed as *N / D*, Features, Stories
functionally verified as *N / S*, Test documents), end marker — computed from the same walk
`check-assurance-spine` performs.

**And** `check-assurance-spine` locates that region in the committed page and byte-compares it
against the emitter's output, printing the expected block verbatim on mismatch.

**And** a page with no begin marker is refused naming the command that produces one; a page that
opens the region and never closes it is refused separately — a truncated region and a stale
region are different defects.

**And** a CRLF checkout is not a defect; the comparison normalises line endings first.

**And** `emit-dashboard` does not run the dashboard check and does not write the file — `-08`
clause 1's reasons, restated because this is the clause most likely to be "simplified" away.

### 2. The surviving numerics are gated, not generated

**Given** the committed page,
**then** `check-assurance-spine` refuses it when any of the following extracted claims disagrees
with the spine walk that is checking it:

- the tabstrip's `Epics … N decomposed` count;
- the tabstrip's `Loose ends … N open` count;
- the progress bar's `width:N%`, where *N* is the integer-rounded percentage of Stories whose
  header state is `Verified` or `Functionally Verified`;
- the footnote's four state counts — `N Verified + N Functionally Verified of N Stories,
  N Specified, N In progress`;
- the Epic-denominator sentence — `The Epic denominator is now N`.

**And** each refusal message carries the expected text verbatim, so the fix is in the error.

**And** no check in this Story rewrites any prose. The words around every gated number are
untouched, and the per-Story tables, Report links and UPDATE narrative are not read at all.

### 3. The Epic population is derived from disk

**Given** `goals/epics/` and the backlog's phase table,
**then** the roadmap population is the union of `EPIC-P*` document ids and the `EPIC-P*` ids in
the first column of [`backlog.md`](../epics/backlog.md)'s phase table (the table above the
*Destination horizons* heading). Horizon Epics (`EPIC-H*`) are excluded: the backlog states their
ids "are not inserted into the numbered critical path and do not imply sequence".

**And** an Epic in that population counts as **decomposed** when at least one Story contract row
belongs to it (by the Story id's Epic token).

**And** both derivations are pure functions with their own tests: a new `EPIC-P10.md`, a new
backlog row, or a first Story under a previously story-less Epic each moves the derived figure
with no human retyping it.

### 4. Every refusal is demonstrated

**Given** that this Story's own changes make the committed tree satisfy clauses 1–3,
**then** a passing `check-assurance-spine` is **not** evidence that any check can reject, and no
Report may cite it as such.

**Therefore** host tests drive each refusal with a fabricated input: a stale generated tile, an
absent `overall-progress` region, an unclosed region, a stale tabstrip Epic count, a stale
tabstrip loose-end count, a stale bar width, stale footnote state counts, a stale Epic
denominator. **And each has an acceptance case beside it** — the emitted block passing its own
check, a CRLF page passing, agreeing counts accepted — because a check that only ever rejects is
as uninformative as one that only ever accepts.

### 5. The committed page is corrected, not grandfathered

**Given** the live drift found while specifying this Story (the tabstrip's "3 decomposed" under a
tile saying 4),
**then** the same change that lands the gates corrects the page, and nothing is exempted —
`STORY-P0-01-07`'s reason: a gate green on day one against known-stale text is blind to exactly
the drift it exists to catch.

## What this test explicitly does not establish

- **That the page is complete or correct as prose.** A tile that is accurate and an argument that
  is wrong look identical to every check here.
- **That "decomposed" means finished.** The derived numerator counts Epics with at least one
  Story contract row; `EPIC-P2`'s decomposition is genuinely partial and the prose beside the
  tiles still says so — in words, which nothing here generates.
- **That the footnote's state vocabulary is future-proof.** A fifth Story state reshapes the
  gated sentence by hand; the gate refuses until it is reshaped, which is the intended direction
  of failure.
- **Anything about `README.md`** (`LE-34`) **or about headers versus their own Reports**
  (`LE-65`). Both stay open.
- **Any performance guardrail.** None closes here and no Story's assurance state moves.

## Reports

- [`REPORT-2026-08-01-01`](../reports/REPORT-2026-08-01-01.md)
