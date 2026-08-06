# 12D — The Domain Label Is Machine-Checked Now, and the Rule the Row Asked For Would Not Have Caught Its Own Instance

Session handover, written 2026-08-06, executing
[`11C`](11C-cover-note-for-the-next-session.md)'s mandate — `LE-91`, item 1 for
three sessions. **No plug, no board run**; everything here ran on a laptop, as
`11C` said it would.

Concurrent with a session working `FEAT-P1-12` and `ADR 0015` in the same tree
(§6). **Nothing is committed.**

**The one sentence, if only one survives:** *`LE-91`'s prescription — declare
each metric's domain beside its owning Story and assert the contract selects it
— is built, mutation-falsified and wired into CI, and it **would not have caught
the defect that produced the row**, because the bent label was consistent with
its contract; what catches it is a sixth rule holding the label against the gate
the number was filed as evidence for.*

---

## 1. What landed

`LE-91` **closed**. The mechanism four handovers deferred with a stated reason.

- **`kernel::measure::MetricLabel`** — `domain`, `story`, `name` in one
  declaration, and `Metric::labelled` as the only sanctioned way to build a
  `METRIC` line from one.
- **All 40 metrics across all six emitting fixtures** now declare themselves in
  a `static METRIC_LABELS` table **the fixture reads its own labels from**. That
  is the part that matters: the declaration `xtask` parses and the bytes on the
  wire are the same array, so they cannot drift the way `LE-80`'s host-side
  mirror did. `collect(&mut collected, 8, samples)` — the domain and the name
  are no longer arguments at the call site, because a name repeated at the call
  site is a second declaration of something already declared.
- **`cargo run -p xtask -- check-metric-labels`**, wired into
  [`ci.yml`](../../.github/workflows/ci.yml) and
  [`.githooks/pre-commit`](../../.githooks/pre-commit).

Six rules, each **falsified by mutation** and each checked for *which* error it
gave — `10C`'s ninth standing instruction, applied:

| mutation | rule that fired |
|---|---|
| relabel the `D11` stamp back to `D07` | 6 — all three `D11` gates, by name |
| name a Story whose contract omits `D11` | 3 — naming the four domains it does select |
| build a `Metric` by struct literal | 5 — file and line of the construction |
| revert the `D09` contract fix | 3 — both `pe-measure` metrics |

## 2. The row's own prescription was not sufficient, and this is the finding

`LE-91` asked for: *declare per metric its domain and its owning Story, parse
the declarations out of the sources, assert each domain is selected by its named
Story's contract.* Built exactly. Then the first mutation — restore the original
defect, `spoor_stamp_park_rung_per_op_of_8` labelled `D07` — **passed**.

It passes because the bent label was never inconsistent with the contract. `D07`
under a Story selecting `D07` is what the defect *was*: a domain chosen from what
the contract already allowed. A rule that holds a label against a contract has
nothing to object to.

What *was* inconsistent is one register along. `PERF-D11-G01`, `G02` and `G03`
were every one of them read from that metric while it said `D07` — a `D11` target
column compared against a number labelled for another domain. That disagreement
is mechanical, and it is **rule 6**: a `guardrail-evidence.tsv` row whose note
names any declared metric must name **at least one of its own domain**.

**Some, not every** — and that quantifier is the whole difference between a gate
and a nuisance. The first cut demanded that every named metric match, and it
flagged `PERF-D04-G23` on the committed tree: that row correctly quotes the `D11`
stamp cost to explain where its 110-cycle delta comes from, while measuring its
own `D04` metric. A gate that cries wolf on correct work gets switched off. The
admitted case is now its own test, with the reasoning in it.

Rules 1–3 remove the *incentive* — a domain is no longer picked at a site where
the contract is the only nearby constraint. Rule 6 catches someone picking wrongly
anyway. **Neither can decide that a domain names a metric's true subject**; that
is a judgement about what the code measures and no text scan holds it. Stated in
the module, in the row, and here, because a gate believed to cover more than it
does is worse than none.

## 3. A second instance, found by the gate on its first run, pointing the other way

`exec::fixture_pe_measure_main` emits two `D09` metrics. `STORY-P0-01-06`'s
contract selected **`D01` alone** — while the Story is titled *"The `D09`
Disposition"*, its criterion 2 measures `exec::pe::parse` at Tier 0, and
`PERF-D09-G20` is closed on that measurement. Every `D09` number that Story
produced was filed against a contract that did not admit `D09`, and nothing had
ever compared the two.

The mirror image of the spoor metrics: there the label was bent to fit the
contract; here the label was right all along and the contract simply never
selected the domain. One defect, two directions, one gate.

Contract and `TEST-P0-01-06-A`'s metadata now carry `D01,D09`. `D09` readiness is
`prototype`, so this opens no debt row.

## 4. `LE-100` — CI runs no tests. At all.

Filed open, **not fixed**, and it is the largest thing this session found.

`ci.yml` has four jobs. The governance one runs `fmt`, `clippy --all-targets`,
three `xtask` checks and `cargo doc`; the other three boot QEMU fixtures. **There
is no `cargo test` step anywhere in the file.** `clippy --all-targets` *compiles*
the test harnesses, which is why it reads as covered: a broken test fails the
build, a **failing** test does not.

~1210 host tests pass locally. Among them is every source-level guard filed as
the closure of a loose end — `LE-99`'s stamp density, `LE-97`'s cadence, the
`G23` pair equivalence, `check-citations`' own tree assertion. Each was written
as *the mechanism that stops this recurring*, and each is invisible to the
runner.

This is `LE-72` and `LE-92` a third time — a gate that exists but is not executed
where it counts — and it is why `check-metric-labels` is a **subcommand in
`ci.yml`** rather than only a `#[test]`. Until `LE-100` lands, treat every
`#[test]`-only closure in the register as locally enforced only.

## 5. Two things the register itself could not do

**The loose-end id grammar stopped at `LE-99`.** Filing `LE-100` failed
validation; the register was full. Widened to `LE-NN` **or** `LE-NNN`, and the
important half was not the validator: `loose_end_tokens` took the *first two
digits*, so `LE-100` in prose resolved to a citation of `LE-10` — a real row
about something else. A decoder that confidently names the wrong record is
`LE-80`'s family and is worse than one that refuses, so four or more digits are
now refused rather than truncated.

**`LE-101` — `.gitattributes` protected only `*.golden.txt`**, so a fresh
worktree on this host checked out `.rs` as CRLF and `board_dispatch`'s
`include_str!` guard failed at `split_once("\n}\n")` on code this session never
touched. **Where it bites is the point:** that is exactly the workflow
`CONCURRENT_SESSIONS` rule 8 *mandates*. A session that correctly refuses to
repair another's broken row is told to verify its subset in a throwaway
worktree — and the throwaway worktree is the one place this fires, producing a
red guard indistinguishable from a real break. `*.rs -text` added beside the
goldens rule, verified by re-checkout. Closed.

The guards are still whitespace-pinned. `10C` §2 already recorded that as failing
*loud* and acceptable; this removes the one environment that made them fail loud
for a false reason.

## 6. The concurrent session, and how this was verified

`FEAT-P1-12` (the RT reserve, split out of `FEAT-P1-05`) and `ADR 0015` landed in
this tree, uncommitted, while this work ran. Both sessions touched
`story-contracts.tsv`, `guardrail-evidence.tsv` and `xtask/src/main.rs`.

Their `xtask` was mid-edit and **non-compiling** for part of this session, so
rule 8 was followed rather than worked around: a detached worktree at clean
`HEAD`, this session's files copied in, every gate run there. When
`spine_tests.rs` was copied wholesale and dragged their edits along, it was
restored to `HEAD` and only this session's hunk re-applied. Same for `main.rs`.

**One coherence check worth recording:** their change added four columns to
`guardrail-evidence.tsv` mid-session. Rule 6's reader locates `guardrail_id`,
`domain` and `note` **by header name, not by index** — verified green against
both the 7-column and the 11-column shapes. A positional reader would have
compared metric names against a column that never holds them.

Their framing is right and worth carrying forward: `LE-91` asks *is this number
labelled with the domain of the thing it measures?* and `ADR 0015` asks *was it
measured under conditions that support what it claims?* Same family, two layers.

## 7. State at close

- **Nothing committed.** Two sessions' work is interleaved in
  `story-contracts.tsv`, `guardrail-evidence.tsv`, `main.rs` and
  `spine_tests.rs`. `CONCURRENT_SESSIONS` rule 1 is live and path-level staging
  is file-level staging — read `git diff --cached` **before** committing.
- **`goals/index.html` is stale** by this session's two new loose-end rows and
  must be regenerated **once, over the combined tree**, after both sessions'
  changes settle: `cargo run -p xtask -- emit-dashboard`. It is the one gate
  left red and it is red for a reason that is nobody's mistake.
- **Gates, in the isolated worktree over clean `HEAD` + this session's files:**
  spine green (4000 selected contracts), `check-metric-labels` 40 metrics / 6
  fixtures / 8 domains / 9 Stories, `check-spine-files`, `check-citations` 730
  citations, `check-lints` 8 packages, `check-boot-images` 3 variants,
  `check-guest-images` 22 binaries, `check-crate-sizes`,
  `check-performance-catalogue`, `fmt`, workspace suite **1210 passing**.
- **`check-timing-regression`** untouched and still `LE-23` — see `10C` §4.
- **CI:** still no run for `b4a7010`/`cb9b27b` at the time of writing;
  `10C` §3 is the diagnosis and re-running `gh run list` is still item 2.
- **Bench:** board powered and beaconing, untouched. **No plug on the desk.**

## 8. Next session

1. **Regenerate the dashboard and commit**, once both sessions settle (§7).
2. **`LE-100`** — a host test job in `ci.yml`. Not a one-line addition to the
   governance job: it needs the pinned toolchain and belongs beside the QEMU
   jobs, and the first run may surface Windows-vs-Linux differences, which is
   `LE-64`'s family and must not be met mid-merge.
3. **Check `b4a7010`'s CI run** once Actions recovers (`10C` §3). Expect red,
   expect three unbaselined metrics, expect that to be `LE-23`.
4. **`LE-98`'s remaining half** — the device-tree parse that makes
   `SIMPLEFB_BASE` evidence rather than folklore.
5. **When the board next runs**, `10C` §5 item 4's checklist, unchanged.

**Do not start:** `FEAT-P1-12` (the RT reserve — **it has a name now**; four
handovers called it "`FEAT-P1-05`'s RT reserve" and the ambiguity is what let it
be started twice), `G09`/`LE-86`, `06A` §4.3. The hardware-evidence sprint rule
from 2026-07-30 has not been lifted.

**The standing instructions, all holding**, stated in full at the end of
[`10C`](10C-the-work-is-on-main-and-the-runner-is-in-an-outage.md).

**And a tenth, from this session:** *a gate that would not have caught the defect
it was written for is a gate that has not been checked against its own instance.*
The mutation that mattered was not one that made the gate fire — it was the one
that made it stay silent, run against the exact defect the row was filed for.
