# Handover 39B — Four Prose Rules Become Gates, and What the Fourth One Found

Executes [Handover 38A](38A-outstanding-actions.md) **§3 in full** — the four rows it grouped under
*"the rows that stop recurrence rather than removing one instance"*. `LE-33`, `LE-35`, `LE-36` and
`LE-44` are closed, in one Story with a Test document written first.

## 0. Concurrency, per `CONCURRENT_SESSIONS` rule 7

**Two other sessions were live in this tree**, and both are named here because the reader's
alternative is reconstructing a race from a commit graph.

- **`7e4e79b` and `dcbccad`** — the allocation-profiler scoping session, which claimed `39A` while
  this work was in progress. Slot **`39B`** was taken as the convention provides, and every
  `closed_in` value in the register was re-pointed from `39A` to `39B` **before** the register was
  validated. That session's documents are not edited here (rule 5); §5 records what each changes for
  the other, in both directions.
- **`4b35b5e`** — the soak session, which registered `LE-45` and `LE-46`. **`LE-45` and `LE-46` are
  not this session's findings and no credit for them is claimed here**; they were read in full while
  their rows were staged and this session's register edits were pending, so that neither set could be
  written over the other.
- **That commit also swept this session's `session/hand-2026-07-28/index.html` edit — the `39B`
  entry — into itself**, under that session's authorship. It is `585a027`'s shape again, smaller and
  harmless: the content is correct, it is one entry, and nothing unfinished shipped. **It is not
  repaired here** (rule 5 — you record it in *your* document and point back, you do not edit
  theirs), and it is recorded rather than left for someone to reconstruct from a graph. It is also
  the fourth instance in one week of the single failure mode `CONCURRENT_SESSIONS` rule 1 exists to
  prevent, which is worth someone's attention independent of this Story.
- **`LE-45`'s own text names "a second agent committing to this tree at 11:42Z, 11:44Z and 11:45Z"**
  as the only hypothesis for a soak anomaly it explicitly refuses to explain with it. Some of those
  commits may have been this session's. It is recorded here as a fact about the window, and — like
  that row — **not offered as an explanation.**

Notably, `LE-45`'s owner decision cites `ADR 0005` and `LE-33` by name for the principle it applies:
discounting an unexplained failure to meet a gate's own stated precondition is the over-claiming
pattern. That principle acquired a machine today, one register along.

## 1. What landed

| Row | The machine | Where |
|---|---|---|
| **`LE-33`** | `TINYOS-MEAS/2` carries `platform=` and `qualification=`; a platform register; a bound-provenance gate refusing a `G04` row from Tier 0, from x86_64, or from an unqualified platform | `os/src/xtask/src/bound_provenance.rs`, `goals/assurance/qualified-platforms.tsv` |
| **`LE-35`** | The rule written into [`goals/assurance/README.md`](../../goals/assurance/README.md), plus `open-debt.tsv` and a two-directional gate | `assurance.rs`, `goals/assurance/open-debt.tsv` |
| **`LE-36`** | `cargo run -p xtask -- check-spine-files` — the instrument `CONCURRENT_SESSIONS` rule 8 named but did not have | `os/src/xtask/src/spine_files.rs` |
| **`LE-44`** | Every Feature Stories-table row cross-checked against the referenced Story's own `Status:` header | `assurance.rs` |

`STORY-P0-01-07` · [`TEST-P0-01-07-A`](../../goals/tests/TEST-P0-01-07-A.md) ·
[`REPORT-2026-07-28-10`](../../goals/reports/REPORT-2026-07-28-10.md). **593 host tests**, from 549.
No kernel behaviour changed and no performance guardrail closed.

## 2. The two numbers to read carefully, in opposite directions

**`0 bound claims checked` is not good news and must never be reported as any.** No `G04` row exists
in the evidence register, so the `LE-33` gate examined nothing on the committed tree. A green
`check-assurance-spine` is therefore *not* evidence that it works — this is `ADR 0005`'s trap section
and 38A trap 7 applied to this session's own instrument, which is the fourth independent arrival of
that rule and the second on the same day.

The count is **printed rather than suppressed** for exactly that reason: "nothing was wrong" and
"nothing was looked at" are otherwise the same silence. What makes the zero believable is thirteen
refusal tests plus an acceptance case that passes **only** because a fabricated platform is marked
`qualified`. The same argument governs `24 open-debt selections` and `59 Feature/Story rows agree` —
both are satisfied by the tree by construction.

**`LE-44` found two classes of live disagreement on its first run, and this is the part worth
carrying forward.** In 59 rows:

- **`STORY-P1-03-02`'s own header read `In progress`** while `FEAT-P1-03` recorded it `Verified` in
  its Stories table *and* in its own `Status:` line, four days after
  [Handover 04](04-story-p1-03-02-wx-seal-and-first-real-task.md) implemented and verified it. Nobody
  knew. The Story header was corrected.
- **Seven Feature-table cells read `Verified` where the Story's own header read
  `Functionally Verified`.** Those are distinct states in this project's `Status:` vocabulary and the
  difference is assurance debt: a reader who sees plain `Verified` in a Feature's table will not go
  looking for the `baseline-debt` the Story is carrying. Every one of those tables mitigated it in
  prose — they all wrote "; assurance `baseline-debt`" alongside — but the **state word was
  overstated in seven places**, which is `LE-44`'s originating defect with the sign reversed.

**Nothing was grandfathered.** Exempting the existing rows would have made the gate green on day one
and blind to precisely the drift it exists to catch. That was the one real design decision in
`LE-44`, and it is the reason the row was worth more than the typo that raised it.

## 3. What `W2` gains, and the one thing it still owes

[38A §2 decision 4](38A-outstanding-actions.md) said the `-M virt` Story hits `LE-35` immediately,
that the rule had never been written down, and that **writing it is part of `W2` rather than a
follow-on**. It is written now, in [`goals/assurance/README.md`](../../goals/assurance/README.md)
under its own heading and enforced in both directions. `W2` is a decision lighter.

**The three other decisions are untouched and still owed**: placement, who owns the AArch64 binary
crate, and whether a `virt` fault run can satisfy `TEST-P1-07-02-A` clause 2. 38A restates all four
in full and remains the document to open.

**One thing changed that a `W2` session must notice.** Handover 31 recommended `STORY-P0-01-07` as
the `-M virt` Story's slot under `FEAT-P0-01`. **That slot is now taken by this work**, so the
recommendation becomes `STORY-P0-01-08`. Decision 1 was open anyway — 31 declined to take it — so
nothing is foreclosed; the number simply moved.

## 4. Applied to the tree, not only to fixtures

`LE-35`'s register was backfilled: **24 `(Story, domain)` pairs across 21 Stories**, each with its
own reason. Eleven of them are `D25` — the same eleven `LE-29` identified as a contracted obligation
with no evidence behind any of them, now visibly **unclosable** rather than merely unmet. That is a
real change in what the spine says about itself, and it cost one register.

`D17` remains selected by zero Stories, which is `LE-35`'s other half and stays open as a coverage
question under `LE-29`. What no longer blocks it is the missing rule.

## 5. What 39A changes for this document, and what this document changes for 39A

39A argued **against** ranking the allocation profiler above `W3`/`W4` on one ground: *"`LE-33`'s
second condition should arguably land first, so numbers this tool produces cannot be promoted into
bounds by a gate that does not yet exist."*

**That gate now exists, so that specific objection is discharged.** It does not follow that `W5`
should be ranked — 39A's five decisions are untouched and its ranking was deliberately left open.
What is settled is that one of the two arguments it weighed no longer applies.

In the other direction, 39A sharpens something this session should not overstate: `G12` is a
*latency* guardrail, so under `ADR 0005` it cannot close as a bound on Tier 0 at all — and the gate
built here is exactly what will refuse it. The two documents agree.

## 6. Traps

38A's ten stand unchanged. Three earned their keep here and one is new.

1. **Trap 7 arrived for the fourth time**, and 39A independently reported the same shape hours
   earlier. When a rule derives itself four times from four unrelated directions it is not a habit —
   it is `SEC-19`, and it belongs in a gate. It is now `TEST-P0-01-07-A` clause 2, as a standing
   requirement on any gate this project adds.
2. **Trap 9 fired exactly as written.** A concurrent commit landed mid-turn; state was re-derived
   (`git log`, `git status`, the diff of what was about to be touched) before continuing, which is
   how the `39A` collision was caught before it became two documents at one number.
3. **Trap 6/9's instrument now exists** and was used on itself: the hand edit closing these four
   register rows was validated with `check-spine-files` before the next tool call.
4. **New, and small: a gate applied to a tree it was not written against will find things.** The
   correct response is to fix them and say what was found, not to soften the gate until the tree
   passes. Seven cells and one header changed here. A version of this session that had widened
   `Verified` to accept `Functionally Verified` would have shipped a green check that could never
   catch the defect it was built for.

## 7. Known limits, stated rather than implied

- **The `LE-33` gate reads the machine-readable spine, not English.** A Report can still write "the
  worst case is 1.2 µs" in a sentence. What is now impossible is a bound entering the spine from a
  disqualified source. `TEST-P0-01-07-A` says so under its own heading.
- **`ADR 0005` §"Consequences" still says `TINYOS-MEAS/1`.** It was accurate when written and the
  envelope moved to `/2` when the requirement was implemented. The ADR body is deliberately not
  edited — `ADR 0005` itself sets the rule that a cited document is not rewritten underneath its
  citers. The current name lives in the Story, the Test and the assurance README.
- **`check-spine-files` is a subset by construction and by one structural test**, not by proof.
- **A pre-existing Windows-host limitation, restated so nobody reads it as a regression from this
  work**: `cargo clippy --workspace --all-targets` cannot build the `kernel`/`exec` *binaries* on
  Windows, because `hal-x86_64` gates `boot`, `serial`, `qemu_exit` and `interrupts` on
  `cfg(not(target_os = "windows"))`. CI runs that job on Linux, where it passes.
  `cargo clippy -p xtask --all-targets -- -D warnings` and `-p kernel --lib --all-features` are both
  clean here.
- **`goals/index.html` has now been hand-edited by eight consecutive sessions.** `LE-30` gained its
  third argument today and remains open.

## 8. The work order after this session

Unchanged from [38A](38A-outstanding-actions.md), except that its §3 table is now empty.

| # | Action | Blocked on |
|---|---|---|
| **W1** | **The board.** `STORY-P1-07-01` criteria **3 and 4** and `STORY-P1-07-02` clause 2 in one sitting; first `Q1`, the start of `Q2`, **and not a bound** | A loopback-tested USB-serial adapter — six sessions now |
| **W2** | **The `-M virt` fixture.** **Three** decisions now, not four | Nothing. Still the best unblocked work |
| **W3** | `LE-23` — re-record the baseline from a CI run; `LE-24` may come free and `LE-42` depends on it | Nothing |
| **W4** | `LE-30` — generate the dashboards from `list-status`. Eight hand-edits now, and `LE-34` is its sibling | Nothing |
| **W5** | The allocation / pool-claim profiler ([39A](39A-allocation-profiler-scoping.md)) | Its five decisions; its `LE-33` objection is discharged |

## State at the close

```text
main                    7e4e79b + this session's commit
                        TEN commits ahead of origin before this one, UNPUSHED
                        two concurrent sessions committed mid-turn: 7e4e79b and dcbccad
assurance spine         23 Features, 59 Stories, 46 Tests, 47 Reports
                        46 loose ends (28 open), 85 status headers
                        11 release gates with evidence
                        24 open-debt selections, 5 platforms (0 qualified)
                        0 bound claims checked -- see section 2, this is not good news
                        59 Feature/Story status rows agree
host tests              593 across the workspace, from 549
Stories verified        0 / 59 assurance-verified; unchanged and correct
platforms qualified     zero, the Pi 5 included -- now a machine-readable value
open decisions          none owed; two untaken (38A section 4), plus W2's three and W5's five
best available work     the board, if an adapter exists
best UNBLOCKED work     the -M virt fixture, one decision lighter than yesterday
```

Counts describe the tree at this document's close.

`goals/reports/_soak-p0-03-01.log` is still dirty and was still left alone. Seventh session.
