# Handover 05B — Cover Note and Agenda for the Next Session

**The start-here document.** Supersedes [38A](../hand-2026-07-28/38A-outstanding-actions.md) *as the work
order* and folds in the amendments `45A` §8 and `01` §9 made to it. 38A remains the reasoning, its ten
traps still stand, and its §2 (the four `-M virt` decisions) is still the only place those are written
out in full.

`main` is at **`280fe6e`**, **four commits ahead of `origin`, unpushed**.

## 1. Where things stand

```text
assurance spine    23 Features, 63 Stories, 50 Tests, 51 Reports
                   53 loose ends (31 open), 90 status headers
                   11 release gates with dated evidence, of 391
host tests         621, 0 failed
Stories            35 Verified + 9 Functionally Verified of 63; 0 assurance-verified
platforms          5 known, 0 qualified — the Pi 5 included (ADR 0005)
soak               ~61h of 72h at last log; one unexplained anomaly (LE-45),
                   so PERF-D07-G22 will NOT close on this run, by owner decision
```

**What changed today, in one line each.** Two stranded Stories recovered and reviewed (`STORY-P1-04-05`
Tier 0 composition proof; `STORY-P0-07-03` `grant` fails closed, which found a *reachable* unchecked-
arithmetic defect the row had not described). `EPIC-P2` written — three flavours, tab host, security
doctrine. `goals/index.html` recomposed into four tabs and ten collapsible sections. The pre-commit gate
now validates the **index**, so two sessions can commit independently.

## 2. The agenda

**`FEAT-P1-06` is W1, on the owner's instruction.** It is `EPIC-P1`'s integration exit and the `G-PA-1`
flagship path, and it was amended today ([`280fe6e`](../../goals/features/FEAT-P1-06.md)) because two
things would otherwise have ambushed the session.

| # | Work | Blocked on |
| --- | --- | --- |
| **W1** | **`STORY-P1-06-01`** — the bounded decision-to-actuation path. §3 below is its scope | Nothing, in the shape §3 recommends |
| **W2** | **`LE-23`** — re-record the timing baseline from a CI run. `LE-24` may come free and `LE-42` waits behind it | Nothing. **`origin` was pushed today**, so this is finally askable |
| **W3** | **`LE-46`** — run the soak sweep under `--serial-capture`. The flag exists; CI already uses it | Nothing |
| **W4** | **`LE-42`** — the `D09` accept path at 17.6–39.1× its budgets. Still the most serious *unanalysed* finding | W2 first |
| **W5** | **`EPIC-P2` §6 edit + the ADR**, per `LE-53` | A concurrent session offered the ADR |
| **W6** | **`LE-52`** — generalise the panic gate. Do the explicit half first; it bounds the implicit half | A scope decision |
| **W7** | **The board** — `STORY-P1-07-01` criteria **3 and 4** (per `LE-44`), `-02` clause 2 | An adapter. Six sessions |

## 3. W1's scope — the decision this agenda takes

`FEAT-P1-06`'s exit criteria have three halves, and **they are gated by three different things**:

| Half | Gate | Available today? |
| --- | --- | --- |
| Mechanism + **enforcement firing** + distribution recorded | nothing | **Yes** |
| The same distribution **under hostile load** | `FEAT-P1-05`, `Specified — no Story started` | No |
| `PERF-D03-G04`/`PERF-D05-G04` — the **bound** | hardware **and** a qualification record; zero platforms hold one | No, and not for a long time |

**Recommendation: take the mechanism half now and defer the other two explicitly.**

The argument is that ordering buys nothing. Building `FEAT-P1-05` first — an entire adversarial Feature —
delays the mechanism proof by a Feature and **does not unlock the bound**, because the bound is gated by
`ADR 0005` and hardware, not by load. So the two deferred halves stay deferred either way, and the
mechanism proof is the largest increment actually available.

**What the Story should therefore claim:** the decision-to-actuation path exists, the budget and deadline
are declared, the deadline monitor **enforces** them, a deliberate overrun **trips the declared policy**,
and the distribution is measured and recorded with its margin — under idle load. That is precisely
`G-PA-1`'s wording, *"enforced by the scheduler, not merely observed in testing"*.

**What it must not claim**, and the machine will refuse it if it tries: no `G04` row.
`bound_provenance.rs` rejects a `G04` sourced from Tier 0. Write the Report to say the bound is stated
debt against `LE-09`, in those words.

**Three things to do before writing code**, in order: `TEST-P1-06-01-A` **Red first** (no Test document
exists yet); confirm `STORY-P1-06-01`'s contract row still matches what the Story will actually select
(it is `D03,D05` today); and read `FEAT-P1-06`'s new amendment section rather than only its top half,
because the top half is the version that predates `ADR 0005`.

**The positive control is already an exit criterion** — *"the proof must show the enforcement firing, not
only clean runs"* — written 2026-07-26, two days before `ADR 0005` said the same thing about `Q3`
campaigns. **Do not let a clean run stand in for a demonstrated trip.**

## 4. Two rows this session owes and could not write

Contiguity is enforced, `LE-53` is written-and-uncommitted by a concurrent session, so `LE-54` cannot
exist. **Both of these are owed the moment `LE-53` lands:**

- **Concurrent sessions cannot commit independently — four coupling points, two remaining.** Coupling 2
  (serial id allocation; wants an `xtask register-loose-end` that allocates and appends atomically) and
  coupling 4 (`goals/index.html` as a serialisation point; `LE-30`'s second half — generate the whole
  dashboard, not the tiles only). Diagnosis in [04B](04B-concurrent-sessions-can-commit.md) §1.
- **`FEAT-P1-06`'s two gaps**, now fixed in the document but unregistered: `FEAT-P1-05` appeared only in
  an exit criterion and not in `Depends on`, and the Feature predated `ADR 0005` by two days and cited it
  nowhere. Both are the `LE-43` shape; the second is its **first occurrence found after a gate existed to
  refuse it**, which is why it would have been discovered at Report time.

The inability to file the first of those is itself the strongest evidence for it. **Hitting it twice in
one session is why it is W-listed above as fix ② rather than left as prose.**

## 5. Traps

**All ten in [38A](../hand-2026-07-28/38A-outstanding-actions.md) §6 stand**, and they are not restated
here because that document does it properly. The three most likely to bite W1 specifically:

1. **A clean run is not a demonstrated enforcement.** §3.
2. **Do not reach for `--update-baseline` locally** (`LE-28`), and do not baseline a profiled or
   instrumented run.
3. **Validate with the check that would fail.** `check-spine-files` after every hand edit to a spine TSV;
   `check-assurance-spine` before you commit. The pre-commit hook now checks the **index**, so it will no
   longer fail for another session's reasons — and equally, it no longer notices that *your working tree*
   is broken.

**One more, new today and cheap to forget:** if you ever ran the pre-commit hook from `3e624bc` (the
version that shared `CARGO_TARGET_DIR`), run **`cargo clean -p xtask`**. That version could leave a
cached `xtask` bound to a deleted temp directory, or — worse — make the hook validate the working tree
while reporting that it validated the index.

## 6. The tree you will start in

**A concurrent session has uncommitted work: `LE-53`, `03A-tauri-and-the-tab-host.md`, plus edits to
`goals/index.html` and this folder's `index.html`.** It is complete and green as far as this session can
tell; it is not this session's to commit. **Leave it**, or review and recover it the way
[`e980b9a`](../hand-2026-07-28/45A-the-composed-scenario-under-preemption.md) and `82e3d57` recovered the
two stranded Stories — read it first, per rule 3.

If you need to add a register row while their edits are pending, use the content-staging procedure in
[`CONCURRENT_SESSIONS`](../../agent/CONCURRENT_SESSIONS.md) rule 1 — `git add <path>` on a shared file
takes their lines too.

**And push.** `main` is four ahead of `origin`. `LE-23` (W2) is the one piece of work whose central
question — *do the committed timing ratios survive a Linux CI runner?* — cannot be asked until the push
happens, and `LE-42` waits behind `LE-23`.

## 7. Session letters

This session is **`B`**; a concurrent one is **`A`**. Per the amendment in
[`session/README.md`](../README.md) the letter now identifies the **session**, not the document — claim
one at start and use it all day. The earlier letters in this folder predate the rule (`01` carries none)
and are left alone.
