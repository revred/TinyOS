# Handover 35 — `LE-43` Closed: the Amendments Verified Rather Than Inherited, and a Three-Way Collision Recorded

Closes `LE-43`, which [Handover 34](34-next-session-mandate.md) raised. **The amendments this row asks
for were written by a concurrent session in `c4de6e1`, not by this one.** What this session contributes
is the part that matters more: **they were checked against `ADR 0005` clause by clause rather than
inherited**, which is the instruction that session left, and it was the right one.

Handover 34 remains the mandate for everything except its step 1, which is now done.
[Handover 33](33-two-decisions-settled.md) remains the decision record and is not superseded.

## Why this document exists at all, and what it is not

`c4de6e1` landed the fix and **deliberately left the row `open`**, with its commit message stating:
*"slot 35 was claimed for its closure, and closing another session's row is not mine to do."* That is
the correct call, and it is the reason this is a separate handover rather than a paragraph in theirs.

**This session wrote no kernel or test code.** `LE-43` is a documentation defect by construction — the
narrative knew and the machine-readable artifacts did not — so there is no test to write. That is also
why it does *not* close the underlying problem; see §"What is still owed".

## 1. The three amendments, verified

Each was read against its source rather than against the commit message describing it. **All three
hold.** `ADR 0005`'s section headings and clause text were opened directly for every row below.

| Claim in the amendment | Checked against | Verdict |
| --- | --- | --- |
| `Q1` includes the entry exception level, so the Report's existing metadata plus `current_el=` *is* `Q1` | `ADR 0005` §"What a secure-world qualification record contains", Q1 — *"the exception level TinyOS is entered at"* | Holds, verbatim |
| `Q2` may be undeterminable on closed firmware and must then say so **in those words** | Q2 — *"Where the firmware is closed and this cannot be determined, the record says so in those words"* | Holds, verbatim |
| Whether the qualification record is `STORY-P1-07-06`'s scope or a seventh Story is **not** settled | §Consequences — *"a decomposition decision for `FEAT-P1-07` §6 and is deliberately **not** settled here"* | Holds; the amendment correctly declines to settle it too |
| A `Q3` is inadmissible without a positive control | §"The trap this ADR sets, named up front" | Holds |
| Zero platforms qualified, the Pi 5 included, and it *cannot* be until `current_el=` is read | §Decision clause 3 | Holds |
| `SCR_EL3.FIQ`-routed secure groups preempt NS-EL1 irrespective of `PSTATE.I`, unattributably | §Context, mechanism bullets 1–4 | Holds, including that NS-EL1 cannot read `SCR_EL3` |
| A seventh acceptance criterion on `STORY-P1-07-06` would extend `TEST-P1-07-06-A` | [`TEST-P1-07-06-A`](../../goals/tests/TEST-P1-07-06-A.md) exists with eight sections mirroring the Story's six criteria | Holds — and see below, the constraint is real and its landing place is identifiable |
| `STORY-P1-07-06` criterion 4 already says the Report states what the numbers are *not* | Criterion 4, and `TEST-P1-07-06-A` §5 | Holds |

**One thing worth adding to the record**, because the next session should not have to find it again:
the natural eventual home for the no-bound sentence is **`TEST-P1-07-06-A` §8, *"What this test
explicitly does not establish"*** — which currently lists the comparative claim, hardware CI, `LE-23`
/`LE-18`, and release-gate closure, and **does not yet carry the bound item.** So the debt
`STORY-P1-07-06` records is real, it is one bullet wide, and it is precisely located. The session that
writes `TEST-P1-07-06-A`'s Red is the one that decides whether it goes there or becomes a criterion.

## 2. One defect found in the amendments, and fixed

**`FEAT-P1-07` cited a section heading that does not exist.** It read `ADR 0005` §"The trap this ADR
sets"; the actual heading is **§"The trap this ADR sets, named up front"**. Corrected in place.

Handover 34 has the same error in a third form — §"The trap this ADR sets against itself" — and
**that document is not edited**, per `CONCURRENT_SESSIONS` rule 5. The correction is recorded here
instead. `session/hand-2026-07-28/index.html`'s entry for Handover 33 also uses *"the trap the ADR sets
against itself"*, which is prose rather than a citation and is left as written.

Small, and worth doing anyway: a dead cross-reference in a Feature is how a reader concludes the
binding rule is unfindable and proceeds without it.

## 3. The collision, per rule 7

**Three sessions touched this row.** The protocol has no mechanism for that, so it is recorded rather
than tidied away.

```text
d89c00a  session A  ADR 0005 + ADR 0006, LE-39/LE-41 closed        (Handover 33)
4c1afd1  session B  Handover 34, LE-43 raised (open, unowned)
c4de6e1  session A  the three artifact amendments; LE-43 left open
         session C  this document; LE-43 closed
```

- **Both A and B independently wrote an `LE-43` row.** For a period the shared TSV held **two `LE-43`
  rows and `check-assurance-spine` was red** (`duplicate id LE-43`). Session A caught it with its own
  rule-8 field check and withdrew its own row in favour of the one that raised the finding.
- **This session observed the red spine and did not repair it.** A guarded write — conditional on the
  file's line count — refused to fire, and one tool call later the duplicate was gone with no action
  from here. That guard is the only reason two sessions did not write the same file in the same second,
  and it cost one `[ ... ] || exit 1`. **Rule 8's "validate before your next tool call" is worth a
  second clause: guard the write itself, not only the result.**
- **Nothing belonging to another session was repaired, rewritten, or staged**, `--no-verify` was not
  reached for, and the handover-35 slot was claimed by creating the file before its contents (rule 4).
  The slot was briefly released and re-claimed when this session expected to stand down.
- **Session A edited its own Handover 33** to correct its concurrency note and record §4. That is a
  session's own dated document, so rule 5 does not bite; it is noted because the diff is large (+69)
  and a reader diffing 33 will want to know why.

## 4. What landed

| Artifact | Change |
| --- | --- |
| `loose-ends.tsv` | **`LE-43` closed** against `hand-2026-07-28/35`, amended **in place** on the existing row per the `LE-39`/`LE-41`/`LE-33` convention — never a second row with the same id |
| `FEAT-P1-07` | `ADR 0005` trap-section citation corrected to the real heading |
| `goals/index.html` | `43 rows, 29 open` in both places; `LE-43`'s paragraph moved from *"open, and its fix has already landed"* to closed; **Closed (13) → (14)** |
| `session/hand-2026-07-28/index.html` | count line to `43 rows, 29 open`; entry 35 added |

`LE-43`'s `owner_path` records what closed it, that the amendments were verified rather than inherited,
and **that the mechanical half is deliberately not closed here.**

`check-assurance-spine` green at close: 23 Features, 58 Stories, 45 Tests, 46 Reports, **43 loose ends
(29 open)**, 84 status headers, 11 release gates with evidence. **549 host tests** across the workspace,
unchanged — no code was written.

## 5. What is still owed, and it is the important part

**Closing `LE-43` removed one instance and changed nothing mechanical.** Stated plainly because the
row's own text says so and a reader skimming a closed row should not miss it:

> **A Report from `FEAT-P1-07` quoting one of its numbers as a `G04`-class bound would still be wrong
> under `ADR 0005`, and would still pass every gate in this repository.**

That is **`LE-33`'s second condition**, which is open, and it is the fourth time this week the same
shape has been registered — `LE-28`, `LE-33`, `LE-36`, `LE-43`. Amending prose is the cheap half. The
lint is the half that stops the next one, and it needs the `TINYOS-MEAS/1` envelope to carry a platform
identity and a qualification-record reference.

**A suggestion this session did not act on**, raised by session A and left to the owner because it
edits an accepted ADR's body rather than its status header: `ADR 0005`'s trap section cites one
provenance ([Handover 32](32-next-session-mandate.md) §Traps trap 3). There are **three independent
arrivals** at the same rule — the `.org` guard padded past 128 bytes before its zero was trusted, the
SIMD detector self-tested on `v1.16b` and `fadd s0`, and now `Q3`'s positive control. A rule with three
derivations is much harder to argue away than one asserted once. **It is one paragraph and it is not
`LE-43`'s scope**, so it is offered, not taken — `ADR 0004`'s treatment is the precedent for leaving a
cited body alone absent a reason.

## 6. Work order for the next session

Unchanged from [Handover 34](34-next-session-mandate.md) minus its step 1, and its eight traps all
stand — including the seventh, which is `ADR 0005`'s own and the sharpest in the set.

1. **The board, if a loopback-tested serial adapter is in your hand.** Still the highest-value session
   available by a wide margin. Handover 26 §"If an adapter *is* in your hand" is unsuperseded. It yields
   the first `Q1` and the start of `Q2`, and **not a bound** — do not let a successful capture be
   written up as one.
2. **The `-M virt` fixture.** [Handover 31](31-qemu-virt-fixture-scoping.md) is the scoping; its §7
   lists four decisions to settle before writing anything. `ADR 0005` does not touch it.
3. **`LE-23`** — re-record the timing baseline from a CI run; `LE-24` may come free.
4. **`LE-30`** — generate the dashboard from `list-status`. This session hand-edited
   `goals/index.html` in **five** more places, the sixth consecutive session to hand-edit it. The
   argument for the row is now six sessions long.

## State at the close

```text
main                    c4de6e1 + this session's commit
                        THREE commits ahead of origin before this one and UNPUSHED
                        (d89c00a, 4c1afd1, c4de6e1 — two sessions' work, not one)
assurance spine         23 Features, 58 Stories, 45 Tests, 46 Reports
                        43 loose ends (29 open), 84 status headers
                        11 release gates with dated evidence, of 391
host tests              549 across the workspace; unchanged, no code written
Stories verified        0 / 58
open decisions          none
ADRs                    0005 accepted (supersedes 0004), 0006 accepted
platforms qualified     zero, the Pi 5 included
best available work     a board session, if an adapter exists
next best               the -M virt fixture
```

**`main` is unpushed and carries three other commits from two sessions.** Handover 34's eighth trap
said to check before starting and to say so if you push someone else's commit; this session did not
push, for that reason and because a concurrent session was live when the work began.

`goals/reports/_soak-p0-03-01.log` is still dirty and still belongs to whoever is running that soak.
Left alone, as Handovers 32, 33 and 34 all asked.

**An empty untracked file named `ADR` sat in the repository root.** Zero bytes, never staged, created
at 11:11 and unattributable to any session — the signature of a shell redirect that never received
output. Session A left it because it could not rule out that it was the other session's. It was not
this session's either, and with both other sessions closed there is nobody left for it to belong to, so
it was **deleted** — zero bytes, nothing recoverable to lose. Recorded here so the deletion is not a
silent one.
