# Handover 38A — Handover Filenames Gain a Letter

Follows [`37-adr-0005-provenance.md`](37-adr-0005-provenance.md). **This is the first document named
under the convention it records**, which is the whole of its evidence that the convention works.

No code, no contracts, no register change. `LE-*` count and every gate unchanged.

## The convention

Handover files are now **`NN<Letter>-short-slug.md`**, lettered from `A`:

```text
38A-handover-naming-convention.md      a handover claims NNA
38B-…                                  a follow-on, a review, or a second session at the same number
38C-…                                  and so on
```

Recorded in [`session/README.md`](../README.md) §"Naming convention", which is its authoritative home —
this document is the narrative, not the rule.

**What the letter buys, beyond tidiness.** It gives two concurrent sessions a way to claim
**non-colliding slots at the same number** instead of racing for the next one. That is not hypothetical
here: this repository produced **two handover-number collisions in one day** (17, then 18/19) and, this
week, a three-way collision over `LE-43` in which slot 35 was claimed by one session while another was
amending the artifacts that slot's closure depended on. `CONCURRENT_SESSIONS` rule 4 says claim the slot
by creating the file first; the letter makes a claimed slot cheaper to work around than to contest.

It also keeps a subject's documents adjacent when sorted, so a review of `38A` sits at `38B` rather than
seven numbers away.

## What is deliberately not done

**Files `00-` through `37-` keep their names.** No retroactive rename.

The reason is not inertia. Their filenames are referenced from other dated documents — `33-two-decisions-settled.md`
links `35-le-43-closed.md`, and `37-adr-0005-provenance.md` links both `35-` and `36-` — and
`session/README.md`'s own standing rule is that earlier dated documents are **never retroactively edited**.
A rename therefore has two exits and both are bad: leave dangling links inside an immutable record, or
edit an immutable record to repair them. **The convention is not worth breaking the rule that makes the
record trustworthy.**

Sorting is unaffected — `34-` sorts before `38A-` either way — so the mixed folder costs a reader nothing.

## State

Unchanged from [Handover 36](36-state-reconciliation.md) except `main`'s position, plus
[Handover 37](37-adr-0005-provenance.md)'s ADR edit:

```text
main                    851bee1 + this session's commit, still UNPUSHED, from three sessions
assurance spine         23 Features, 58 Stories, 45 Tests, 46 Reports
                        44 loose ends (30 open), 84 status headers
host tests              549, unchanged
start-here document     Handover 34, read with 36
best available work     a board session, if an adapter exists; else the -M virt fixture
```

The work order in [Handover 34](34-next-session-mandate.md) minus its step 1 still stands, as does
[Handover 36](36-state-reconciliation.md)'s correction of 34's state block, and `LE-44`'s correction
that it is criteria **3 and 4** of `STORY-P1-07-01` that need a board.
