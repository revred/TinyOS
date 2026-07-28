# Handover 37 — `ADR 0005`'s Provenance Paragraph, Taken on the Owner's Instruction

**This document exists because an accepted ADR's *body* was edited.** That is the whole reason it is a
handover rather than a commit message. [Handover 34](34-next-session-mandate.md) and
[Handover 35](35-le-43-closed.md) both recommended this paragraph and both declined to write it,
correctly, on the grounds that it edits a cited document's body rather than its status header.
[Handover 36](36-state-reconciliation.md) recorded it as the owner's call rather than taking it.
**The owner instructed it be taken.** It is taken here, and this is the record.

Nothing else changed. **No kernel or test code was written.**

**Concurrency (rule 7).** A session was live throughout. `465db4e` and `0bb4872` both landed while
this document was being written, and at commit time that session had **files staged in the shared
index** — `agent/CONCURRENT_SESSIONS.md`, `goals/assurance/loose-ends.tsv` and its own Handover 36.
A plain `git commit` would have swept all three into this commit under this session's authorship,
which is rule 1's exact failure. **This session's subset was verified in a throwaway worktree over
clean `HEAD` and then committed path-limited**, leaving the other session's staged entries staged and
untouched. Nothing of theirs was repaired, rewritten or staged, and `--no-verify` was not reached for.

## What was added, and to what

One subsection at the end of `ADR 0005` §"The trap this ADR sets, named up front", headed
**"Added 2026-07-28: this rule was derived three times independently before it was written down"**.

The rule itself is unchanged and was already stated: **a `Q3` residency campaign is inadmissible
unless the same instrument has been shown to detect a known perturbation.** What the paragraph adds is
that the rule was reached three separate times, from three unrelated directions, before anyone wrote
it down — and the argument for recording that is narrow and worth stating plainly:

> A rule asserted once invites *"says who?"*. A rule three sessions arrived at independently is a
> property of the work rather than an author's preference.

## The three arrivals, each verified against the tree

Not inherited from the handovers that reported them. Every quotation below was opened in its source
file before it was written into the ADR.

| Arrival | Source | Verified |
| --- | --- | --- |
| The `.org` vector-table guard was **padded past 128 bytes and made to fail** before it was trusted — `invalid .org offset '128' (at offset '204')` | [Handover 27](27-story-p1-07-02-host-half.md) §"Four things a reviewer should look at first", item 1 | Quoted verbatim, including *"A gate nobody has watched fail is a gate nobody has tested"* |
| The SIMD detector was **self-tested on `v1.16b`, `q0`, `d0` and `fadd s0`** — and correctly ignores `add x1` — before its zero was believed | [Handover 30](30-d09-measured-and-two-corrections-verified.md) §"Two corrections verified, and one improved" | Quoted verbatim, including *"A zero from an unexercised detector would have been the same class of error as the finding being corrected"* |
| `Q3`'s positive control | `ADR 0005`, this section | The third arrival, and the reason the other two are now on the record |

**The first two are build-time and static-analysis problems; the third is a timing measurement on
hardware. Nothing about the domains is shared.** What is shared is the failure mode, and that is the
sentence the paragraph exists to carry: *a negative result is the cheapest thing any instrument can
produce, and it is indistinguishable from a broken instrument until the instrument has been made to
produce a positive.*

## Two dead citations of my own, caught before the commit

Written, then checked, then corrected — recorded because the checking is the point and because the
same defect class was found in this session's previous work by Handover 35:

- Handover 30's section was cited as §"the corrections verified". **The real heading is
  §"Two corrections verified, and one improved".**
- Handover 27's was cited as §1. **The real heading is §"Four things a reviewer should look at
  first"**, with the guard as its item 1.

Both were fixed before the commit. This is the third dead `§` citation in three sessions —
`FEAT-P1-07`'s, Handover 34's, and now two of mine — which is a small pattern rather than a
coincidence: **section titles in this repository are long and get paraphrased from memory.** Nothing
mechanical catches it, and it is not worth a loose end on its own; it is worth the habit of opening
the heading before citing it.

## Why this is an amendment and not a rewrite

The distinction `ADR 0004` established, applied one level down:

- **`ADR 0004`'s body was preserved untouched** because superseding it changed its *conclusion*, and
  `README.md`, `EPIC-P1` and the Handover series cite that conclusion.
- **`ADR 0005`'s body is amended here** because this changes **no decision, no clause, and nothing any
  document cites.** `ADR 0005` is now referenced by `README.md`, `EPIC-P1`, `FEAT-P1-07`,
  `STORY-P1-07-06` and both dashboards; **every one of those references remains exactly as valid as it
  was**, because the paragraph adds evidence for a rule already stated rather than altering the rule.

The addition is dated and headed as an addition rather than interleaved into the original prose, so a
reader meets the original argument first and the provenance second. That is `EPIC-P1`'s pattern, which
is itself `ADR 0004`'s pattern.

## State at the close

```text
main                    0bb4872 + this session's commit
                        SIX commits ahead of origin before this one and UNPUSHED
                        (d89c00a, 4c1afd1, c4de6e1, c488e5f, 465db4e, 0bb4872
                         — three sessions; count this yourself, it moved twice
                         while this document was being written)
assurance spine         23 Features, 58 Stories, 45 Tests, 46 Reports
                        44 loose ends (30 open), 84 status headers
                        11 release gates with dated evidence, of 391
host tests              549 across the workspace; unchanged, no code written
Stories verified        0 / 58
open decisions          none
ADRs                    0005 accepted (supersedes 0004) + provenance addition, 0006 accepted
platforms qualified     zero, the Pi 5 included
best available work     a board session, if an adapter exists
next best               the -M virt fixture, then LE-23, then LE-30
```

**Handover 36's work order stands unchanged**, and so do Handover 34's eight traps. `LE-44`
(`FEAT-P1-07`'s Story-table criteria numbering) is registered and its `FEAT-P1-07` row is already
corrected. `goals/reports/_soak-p0-03-01.log` is still dirty and still belongs to whoever is running
that soak.

**On the push.** `main` is unpushed and carries commits from three sessions. The recommendation this
session gives is in its response to the owner rather than asserted here, because pushing is the
owner's action and not a session's.
