# Handover 38A — Every Outstanding Action from Handovers 34–37, in One Place

**The start-here document.** Supersedes [Handover 34](34-next-session-mandate.md) *as the work order*,
and consolidates what [35](35-le-43-closed.md), [36](36-state-reconciliation.md) and
[37](37-adr-0005-provenance.md) left owed. Those four remain the reasoning and are **not** superseded as
records — but a session should no longer have to read four documents to learn what to do.

**Nothing here is new work.** Every row below is traced to the document that raised it, and every state
claim was checked against the tree rather than inherited. Where a document's item is **done**, it is
listed as done rather than dropped, so a reader of 34–37 can reconcile without diffing.

`main` is at `748aae7`, **eight commits ahead of `origin` and unpushed**, carrying three sessions' work.
Count it yourself — it moved four times while these documents were being written.

## 1. The work order

**If a loopback-tested USB-serial adapter is in your hand, take the board.** Still the highest-value
session available by a wide margin, unchanged since Handover 26 §"If an adapter *is* in your hand".

| # | Action | Source | Blocked on |
| --- | --- | --- | --- |
| **W1** | **Board session.** Closes `STORY-P1-07-01` criteria **3 and 4** and `STORY-P1-07-02` clause 2 in one sitting | 34 §"What to do"; 26 | A physical adapter — five sessions now |
| **W2** | **The `-M virt` fixture.** Four decisions first, §2 below | 34 step 2; 31 | Nothing. This is the best unblocked work |
| **W3** | **`LE-23`** — re-record the timing baseline from a CI run. **`LE-24` may come free, and `LE-42` depends on it** | 34 step 3 | Nothing |
| **W4** | **`LE-30`** — generate the dashboards from `list-status` | 34 step 4 | Nothing |
| **W5** | **The allocation / pool-claim profiler**, adopted as a *pattern* from Sharc.Blue. **Unranked — see below** | [39A](39A-allocation-profiler-scoping.md) | Five decisions, 39A §7 |

**On W1, and this is the part `ADR 0005` changed:** it is **criteria 3 and 4**, not 2 and 4 — criterion
2 is the host-testable stack/`.bss`/`EL2 → EL1` drop. That correction is `LE-44`. Criterion 3 is
`CurrentEL` printed first, which is `current_el=`, which is **the first `Q1` evidence any platform in
this project has.** The session also begins `Q2`. **It does not produce a bound.** Do not let a
successful capture be written up as one.

**On W4:** `goals/index.html` has now been hand-edited by **seven consecutive sessions**. `LE-44` is a
second argument for the same row — both are hand-maintained cross-references with no machine behind them.

**On W5, and its ranking is deliberately left open.** [39A](39A-allocation-profiler-scoping.md) scopes it.
The instrument is **not** a `GlobalAlloc` port — `validate_no_heap` forbids one in all six shipped crates
and there would be no heap to count — it is a pool-claim and bounded-resource census carrying Sharc's
two-atomics-never-written-in-ship design. **The argument for ranking it above W3/W4:** it is the only
proposed work that would *explain* `LE-42`'s 17.6–39.1× `D09` overshoot rather than re-measure it, and
`G11`'s *"pool claims are separately counted"* clause plus all 25 `G12` rows have no instrument at all.
**Corrected — it is not "50 waiting rows"**, as 39A first said: `guardrail-evidence.tsv` already closes
**10 `G11` rows structurally** — *no heap exists, every shipped crate is `no_std` with no
`global_allocator`, stronger than the guardrail asks* — so the **heap half needs no instrument ever**, and
the other 15 domains are claimable at zero tool cost. `G12` is at **0 of 25** and, being a *latency*
guardrail, cannot close as a bound on Tier 0 under `ADR 0005` at all. **The argument against:** `LE-33`'s second condition should arguably
land first, so numbers this tool produces cannot be promoted into bounds by a gate that does not yet
exist. **`G11`'s target is zero, and a zero is the cheapest thing any counter can produce** — 39A §5 makes
the positive control a Red clause, which is trap 7's fourth arrival.

## 2. The four decisions `-M virt` needs first

From [Handover 31](31-qemu-virt-fixture-scoping.md) §7, which declined to take them. **Restated in full
here so W2 is startable from this document alone.**

1. **Placement** — `FEAT-P0-01`/`STORY-P0-01-07` (31's recommendation) versus re-decomposing
   `FEAT-P1-07`. Handover 26 trap 6: a seventh `FEAT-P1-07` Story means re-decomposition, not extension.
2. **Who owns the AArch64 binary crate** — this Story, or `STORY-P1-07-05` with this Story depending on
   it. 31 §3 argues the former on de-risking grounds. **Nothing in the workspace currently links an
   AArch64 binary**; `hal-arm64` is `[lib]` only.
3. **Whether a `virt` fault run can satisfy `TEST-P1-07-02-A` clause 2.** 31's reading is that it
   **cannot** — clause 2 is a *board* clause. If the Story author reads it differently, that reading goes
   in the Test document under its own heading with its reason. **Do not quietly widen it.**
4. **Contract selections.** A row is needed in `story-contracts.tsv`. `D01` is the obvious domain, but
   **`LE-35` bites here**: selecting a domain pulls all 25 guardrails in and this Story closes none of
   them, so the selection must be initialised as **stated open debt** — the rule `LE-35` says has never
   been written down. **Writing that rule is part of W2**, not a follow-on.

One sentence governs the whole scope: **it produces no timing evidence, closes no release gate, and does
not touch `LE-09`.** `ADR 0005` does not reach it either — a QEMU guest is not a qualifiable platform,
because its secure-world configuration is the emulator's rather than a product's.

## 3. Owed by 35, 36 and 37 — the mechanical debt

**These are the rows that stop recurrence rather than removing one instance.** All four are `unowned`.

| Row | What is owed | Raised by |
| --- | --- | --- |
| **`LE-33`** | **The second condition.** A lint refusing a `G04`-class bound sourced from x86_64, Tier 0, **or an ARM64 platform holding no qualification record** — which means `TINYOS-MEAS/1` must carry a platform identity and a qualification-record reference. **This is the single highest-value non-hardware row in the project.** | 33; sharpened by 35 §5 |
| **`LE-44`** | Cross-check every Feature Stories-table row against the referenced Story's own `Status:` header. The state word compares exactly; `criteri(on\|a) N` tokens can be extracted and matched. Both documents are already parsed by `validate_status_headers`, so it opens nothing new. | 36 |
| **`LE-36`** | **Amended, and now a smaller job than its row used to describe.** A field-count guard is **necessary and demonstrably not sufficient** — both duplicate `LE-43` rows were well-formed at eight fields. The guard must also check id uniqueness and contiguity, which `validate_loose_ends` already does, so the answer is **a fast subcommand wrapping the existing validator**, not a new field counter. | 36 §"Rule 8, corrected" |
| **`LE-35`** | The unwritten rule for initialising selected guardrails as stated open debt. **Load-bearing, not theoretical** — W2 hits it immediately. | 32; confirmed by 31 §7.4 |

**Why `LE-33` leads this list.** Closing `LE-43` amended three artifacts and changed nothing mechanical.
**A Report from `FEAT-P1-07` quoting one of its numbers as a `G04` bound would still be wrong under
`ADR 0005` and still pass every gate in this repository.** That is four instances of the same shape in
one week — `LE-28`, `LE-33`, `LE-36`, `LE-43` — plus `LE-44` as the fifth. Prose is the cheap half.

## 4. Two decisions nobody has taken

Neither is a defect and neither has an owner. **Both are recorded so they are not lost by being
mentioned only in passing.**

- **Is the secure-world qualification record `STORY-P1-07-06`'s scope, or a seventh Story?**
  `ADR 0005` **deliberately declines to settle it** and routes it to `FEAT-P1-07` §6. A seventh Story
  means re-decomposing the Feature, which is a scope decision rather than a diff. **The session that
  starts `-06` cannot avoid this**, so it should be taken deliberately rather than by default.
- **Does `TEST-P1-07-06-A` §8 absorb the no-bound sentence, or does it become a criterion?**
  §8, *"What this test explicitly does not establish"*, lists four items and **does not yet carry the
  bound item** — verified by opening it. `STORY-P1-07-06` records it as named debt rather than a seventh
  acceptance criterion, deliberately, because adding a criterion would extend the Test document and
  **the Red comes first.** The debt is one bullet wide and precisely located.

## 5. Done, so 34–37 can be reconciled without diffing

| Item | Where it was owed | Landed |
| --- | --- | --- |
| `LE-43` — amend the artifacts carrying `LE-09`'s closure condition | 34 step 1 | `c4de6e1` (amendments), `c488e5f` (row closed, amendments verified clause by clause) |
| `FEAT-P1-07`'s `-01` row: *"criteria 2 and 4"* → **3 and 4** | 36 / `LE-44` | `465db4e` |
| `ADR 0005`'s trap-section citation, dead in `FEAT-P1-07` | 35 §2 | `c488e5f` |
| `CONCURRENT_SESSIONS` rule 8 — the field-check correction | 36 | `0bb4872` |
| **`ADR 0005`'s provenance paragraph** — recommended by 34 and 35, declined by both, recorded by 36 as the owner's call | 37 | `851bee1`, **on the owner's instruction** |
| Handover filename convention | this document | `748aae7` |

`LE-39` and `LE-41` closed in `d89c00a`; `LE-43` in `c488e5f`. **Fourteen rows closed, thirty open.**

## 6. Traps — ten, and all of them binding

Six are Handover 32's, restated because chasing them is exactly what this document removes. Two are 34's
additions, two are new since.

1. **A green ARM64 fixture is not ARM64 coverage.** 32's trap 1 because that session was caught by it.
2. **Do not patch `LE-37`/`LE-38` directly.**
3. **An external reviewer's confidence is not evidence, and neither is your own.**
4. **`LE-35` is load-bearing, not theoretical** — see §2 decision 4.
5. **Do not reach for `--update-baseline` locally.** One command from turning a known offset into a false green (`LE-28`).
6. **Validate a hand-edited machine-checked file before your next tool call** — and see trap 9, which corrects the instrument.
7. **A qualification campaign is easy to fake by accident.** Not by dishonesty — by running an instrument never shown to detect anything, getting a zero, and filing it. **An excursion not observed is not an excursion that cannot occur.** `ADR 0005` makes a `Q3` inadmissible without a positive control in the same Report. As of `851bee1` the ADR records that this rule was **derived three times independently** — the `.org` guard padded past 128 bytes, the SIMD detector self-tested on `v1.16b` and `fadd s0`, and now `Q3`.
8. **`main` is unpushed** and carries three sessions' commits. Check before you start; say so in your handover if you push someone else's commit.
9. **Validate with the check that *would* fail, and guard the write — not only the result.** Both duplicate `LE-43` rows were well-formed at eight fields, so the field-count pass ran, passed, and could not have caught it; `check-assurance-spine`'s id check did. And a write gated on the file's line count is what stopped two sessions writing one file in the same second. **Re-derive your state when a concurrent commit lands mid-turn.**
10. **Open the heading before you cite it.** Four dead `§` citations in three sessions — `FEAT-P1-07`'s, Handover 34's, and two of Handover 37's, all caught and all avoidable. Section titles here are long and get paraphrased from memory. Handover 37 judged this **not worth a loose end**, and that judgment stands; if it recurs a fifth time, it is a machine, not a habit.

## 7. The filename convention this document is named under

Handover files are now **`NN<Letter>-short-slug.md`**, lettered from `A`. A handover claims `NNA`; a
follow-on, a review of it, or a second session on the same subject takes `NNB`, then `NNC`, rather than
consuming the next number. Recorded in [`session/README.md`](../README.md) §"Naming convention", which is
its authoritative home.

**What the letter buys beyond tidiness:** two concurrent sessions can claim **non-colliding slots at the
same number** instead of racing for the next one. Not hypothetical here — two handover-number collisions
in one day (17, then 18/19), and this week a three-way collision over `LE-43` in which slot 35 was
claimed by one session while another amended the artifacts that closure depended on.

**Files `00-` through `37-` keep their names.** Their filenames are cited from other dated documents —
`33-` links `35-`, `37-` links `35-` and `36-` — and dated documents are never retroactively edited, so a
rename either leaves dangling links inside an immutable record or edits one to repair them. **The
convention is not worth breaking the rule that makes the record trustworthy.** Sorting is unaffected:
`34-` sorts before `38A-`.

## State at the close

```text
main                    748aae7 + this document's commit
                        EIGHT commits ahead of origin before this one, UNPUSHED
                        three sessions' work; it moved four times mid-writing
assurance spine         23 Features, 58 Stories, 45 Tests, 46 Reports
                        44 loose ends (30 open), 84 status headers
                        11 release gates with dated evidence, of 391
host tests              549 across the workspace; unchanged, no code written
Stories verified        0 / 58
open decisions          none owed; two untaken, §4
ADRs                    0005 accepted (supersedes 0004) + provenance, 0006 accepted
platforms qualified     zero, the Pi 5 included
best available work     the board, if an adapter exists
best UNBLOCKED work     the -M virt fixture (W2), after §2's four decisions
highest-value row       LE-33's second condition — the gate that stops recurrence
```

Counts describe the tree at this document's close — stated because Handover 34's grounding table and its
State block disagreed on exactly this point, both truthfully, and a reader read it as a failed check.

`goals/reports/_soak-p0-03-01.log` has been dirty for six sessions. It belongs to whoever is running that
soak. **Leave it.**
