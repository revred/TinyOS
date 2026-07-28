# Handover 36 — State Reconciliation: Handover 34 Still Governs, Its State Block Does Not

**Not a supersession.** [Handover 34](34-next-session-mandate.md) remains the start-here document:
its reasoning, its work order and all eight of its traps stand. What has gone stale is the block a
session checks *first* — `main`'s position and the register counts — plus two defects in the document
itself and one it did not catch. Those are corrected here rather than there, per
[`CONCURRENT_SESSIONS`](../../agent/CONCURRENT_SESSIONS.md) rule 5.

Read 34 for what to do. Read this for where things actually are.

## Handover 34, re-grounded

It was checked again rather than trusted, and **every substantive claim in it holds** — this is worth
recording explicitly, because 34's own trap 3 is that confidence is not evidence.

| 34's claim | Checked |
| --- | --- |
| `ADR 0004`'s body unedited, status header + forward pointer only | Diff since `ff980d0` is 10 insertions, 1 deletion — exactly that |
| `LE-39`/`LE-41` closed with `closed_in` populated | Both `closed`, both cite `hand-2026-07-28/33` |
| `LE-33` grew rather than splitting | Second condition on the existing row |
| 23 Features / 58 Stories / 45 Tests / 46 Reports, 84 headers, 11 gates | Spine agrees exactly |
| 549 host tests, `hal-arm64` at 115 | Both confirmed by run |
| 0 / 58 Stories verified; zero platforms qualified | Confirmed |
| A board session closes `-01`'s capture and `-02`'s clause 2 | Matches both Story status headers |

## What is stale in Handover 34

| 34 says | Actually |
| --- | --- |
| `main` at `d89c00a`, **one** commit ahead of `origin`, unpushed | **`c488e5f`, four ahead of `ff980d0`, still unpushed** |
| Work order step 1: `LE-43` | **Done.** Amendments in `c4de6e1`, row closed by [Handover 35](35-le-43-closed.md) in `c488e5f`. The fallback list now starts at the `-M virt` fixture |
| 43 loose ends (30 open) | **44 loose ends (30 open)** — `LE-43` closed, `LE-44` raised below |
| *"inside `goals/`, only `EPIC-P1.md` and `index.html`"* reference `ADR 0005` | Four documents plus the register: `EPIC-P1.md`, `index.html`, `FEAT-P1-07.md`, `STORY-P1-07-06.md` — the last two **were the point of the fix** |
| Trap 8: `main` is one commit ahead | Four, and **three of them belong to other sessions.** Trap 8's substance is *more* urgent than when written, not less |

## Two defects in Handover 34 itself

**1. A dead ADR citation.** It cites `ADR 0005` §"The trap this ADR sets against itself"; the real
heading is **§"The trap this ADR sets, named up front"**. Handover 35 caught this and fixed the same
error in `FEAT-P1-07`. It matters because 34 is the start-here document and that trap is the one it
calls sharpest — **a reader who cannot find the section may proceed without it.**

**2. An internal contradiction that reads as a failed check.** Its grounding table records *"42 loose
ends (29 open)"* while its State block says *"43 (30 open)"*. **Both were true** — before and after 34
itself added `LE-43` — but the table is explicitly a *verification* table, so a reader checking it
against the tree finds 43 and concludes the grounding was sloppy when it was not. One clause — *"as
Handover 33 left it"* — would have prevented that, and it is a cheap habit for any future grounding
table: **state the moment a verified count describes.**

## What 34 did not catch — `LE-44`

**`FEAT-P1-07`'s Stories table said `STORY-P1-07-01` needs a board for "criteria 2 and 4". The Story's
own status header says 3 and 4, and its criteria list confirms the Story.**

- **Criterion 2** is the stack, `.bss` zeroing and the `EL2 → EL1` drop — host-testable.
- **Criterion 3** is `CurrentEL` printed before anything else — board-only.
- **Criterion 4** is a known byte sequence over PL011 — board-only.

The `-02` row was correct, which is exactly why this read as a typo rather than a pattern. **It is not
a typo in consequence**: criterion 3 is `current_el=`, which under `ADR 0005` is *also the first `Q1`
evidence*, so the Feature under-stated what a board session must close **on precisely the criterion
that produces qualification evidence.** The `-01` row is fixed here.

**The fix is the cheap half, so the mechanism is registered as `LE-44`.** `check-assurance-spine`
validates all 84 `Status:` headers for grammar ([`assurance.rs`'s `validate_status_headers`](../../os/src/xtask/src/assurance.rs))
but **never reads the per-Story status column inside a Feature's Stories table**, so a Feature and its
Story can disagree about which criteria are blocked indefinitely while every gate stays green. The
state word can be compared exactly and `criteri(on|a) N` tokens extracted and required to match; both
documents are already parsed, so nothing new needs opening.

That is the same class as `LE-30` (generate rather than hand-maintain), `LE-28` and `LE-33` (a warning
or a prose rule where a gate belongs) — and it is **the fifth instance this week** of the shape
`LE-43` named. Registering the machine rather than only fixing the instance is the whole lesson of
that row.

## What landed

| Artifact | Change |
| --- | --- |
| `FEAT-P1-07` | `-01` row: *"criteria 2 and 4"* → **"criteria 3 and 4"**, matching the Story |
| `loose-ends.tsv` | **`LE-44` raised** — the missing Feature-table/Story-header cross-check |
| `goals/index.html` | `44 rows, 30 open` in both places; `LE-44` paragraph added |
| `session/hand-2026-07-28/index.html` | count line to `44 rows, 30 open`; entry 36 added |

`check-assurance-spine` green: 23 Features, 58 Stories, 45 Tests, 46 Reports, **44 loose ends (30
open)**, 84 status headers, 11 release gates with evidence. Performance catalogue 625, crate sizes
green. **549 host tests pass, unchanged** — no code was written.

## Still open, and explicitly the owner's call

**`ADR 0005`'s trap section cites one provenance where three exist.** It points at
[Handover 32](32-next-session-mandate.md) §Traps trap 3. The same rule was arrived at independently
three times: the `.org` guard padded past 128 bytes before its zero was trusted, the SIMD detector
self-tested on `v1.16b` and `fadd s0`, and now `Q3`'s positive control. **A rule with three
independent derivations is far harder to argue away than one asserted once.**

It is one paragraph. Two sessions have now recommended taking it and neither has, because it edits an
**accepted ADR's body** rather than its status header, and `ADR 0004`'s treatment is the precedent for
leaving a cited body alone absent a reason. **Left to the owner deliberately** — it is not a session's
call, and it is recorded here so it is not lost by being mentioned only in passing.

## Work order

Unchanged from [Handover 34](34-next-session-mandate.md) minus its step 1, whose eight traps all stand:

1. **The board, if a loopback-tested serial adapter is in your hand.** Highest value by a wide margin.
   It yields the first `Q1` and the start of `Q2` — and **not a bound.** Note `LE-44`'s correction
   before you start: it is criteria **3 and 4** of `STORY-P1-07-01` that need the board.
2. **The `-M virt` fixture.** [Handover 31](31-qemu-virt-fixture-scoping.md) §7 lists four decisions
   to settle before writing anything.
3. **`LE-23`** — re-record the timing baseline from a CI run; `LE-24` may come free.
4. **`LE-30`** — generate the dashboard from `list-status`. This session hand-edited
   `goals/index.html` again, the **seventh** consecutive session to do so. `LE-44` is now a second
   argument for the same row: both are hand-maintained cross-references with no machine behind them.

## State at the close

```text
main                    c488e5f + this session's commit
                        FOUR commits ahead of origin (ff980d0) before this one, UNPUSHED
                        three of the four belong to other sessions
assurance spine         23 Features, 58 Stories, 45 Tests, 46 Reports
                        44 loose ends (30 open), 84 status headers
                        11 release gates with dated evidence, of 391
host tests              549 across the workspace; unchanged, no code written
Stories verified        0 / 58
open decisions          none owed; one offered (ADR 0005 provenance), owner's call
ADRs                    0005 accepted (supersedes 0004), 0006 accepted
platforms qualified     zero, the Pi 5 included
start-here document     Handover 34, read with this one
best available work     a board session, if an adapter exists
next best               the -M virt fixture
```

Counts above describe the tree **at this document's close**, per this session's own §"Two defects"
lesson.

`goals/reports/_soak-p0-03-01.log` is still dirty and still belongs to whoever runs that soak. Left
alone, as Handovers 32 through 35 all asked.
