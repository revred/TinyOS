# 06A — Nothing Is Verified, and the Reason Is Not Velocity

Session handover, written 2026-08-05 in answer to the owner's observation: *"I don't see the
stories moving to completion — especially `EPIC-P1`, Determinism Proof, which has a mandate for
determinism proof."*

**The observation is correct and the cause is not what a productivity handover would assume.**
This document is a diagnosis first and a plan second, because the plan is wrong if the
diagnosis is.

---

## 1. The numbers, before any interpretation

```
Story states across the spine        Assurance states (story-contracts.tsv)
  46  Verified                          55  baseline-debt
  46  In progress                       42  specified
  24  Specified                          0  verified          ← every row, all time
  12  Functionally Verified
```

And every in-flight `EPIC-P1` Story, without exception:

```
STORY-P1-07-01 … -10   In progress   (10)
STORY-P1-09-01 … -15   In progress   (15)
STORY-P1-10-01 … -05   In progress   ( 5)
STORY-P1-11-01         In progress   ( 1)
```

**Thirty-one Stories in flight under `EPIC-P1`. Not one has ever advanced.** And **zero of 97
contract rows have ever reached assurance `verified`** — not this month, not ever.

## 2. There are two ceilings, and they are different problems

Reading the lifecycle in [`goals/assurance/README.md`](../../goals/assurance/README.md)
carefully — which I had not done until asked this question — the ladder has two rungs that this
project has been treating as one.

### Ceiling A — functional `Verified`. Reachable today. Not being claimed.

> *"**Green.** Functional tests pass. This can earn functional `Verified`, but not assurance
> `verified`."*

That is the `Status:` header. It needs passing tests and evidence — **nothing else.** And it is
demonstrably being left on the table:

- `hand-2026-08-04/05A` records `FEAT-P1-07`'s ladder as *"effectively done"*, with
  `STORY-P1-07-02` criterion 2, **`-03` every criterion**, **`-04` all five**, and `-06`'s
  board half Green on silicon.
- `REPORT-2026-08-04-01` was filed and closed four loose ends including release-blocking
  `LE-09`.
- **All ten `STORY-P1-07-*` still read `In progress`.**

This is `LE-65`'s class exactly — *"no gate cross-checks a Story's `Status:` header against its
own filed Reports"* — and `STORY-P0-01-10` closed only the half that refuses a `Specified`
header on a Story with a passing Report. **Nothing refuses an `In progress` header on a Story
whose every criterion is met.** So work accumulates at the front and is never retired at the
back, which is precisely what the owner is seeing.

### Ceiling B — assurance `verified`. **Currently unreachable for every Story in the project.**

> *"`verified` — dated raw evidence and Reports close **every applicable** mapped release gate."*
> *"**Measure.** Reports execute applicable `PERF-Dnn-G01..G23`, `SEC-*` and `BND-*` release
> gates **in the declared deployment profile**."*

And `ADR 0005` holds that a bound is quotable only from a platform carrying a secure-world
qualification record. The register:

```
qualified-platforms.tsv   5 platforms,  0 qualified
guardrail-evidence.tsv    11 rows of 460 release gates
```

`REPORT-2026-08-04-01` states the position itself: *"this Report's metadata **is** `Q1` for this
board; `Q2` and any `Q3` campaign remain unstarted, and the platform register remains empty."*

**So assurance `verified` is not slow. It is closed.** Zero Stories can reach it, by design,
until a platform is qualified — and no amount of building changes that. **This is the finding
that most directly answers the owner's question about `EPIC-P1`'s mandate**: the Epic cannot
discharge *Determinism Proof* through Stories, because the state that would represent proof is
gated on a campaign nobody has started.

## 3. What the last three sessions actually optimised

Honestly: **the loop, not the ledger.** Netboot, `--until`, `check-lints`, `check-boot-images`,
the spoor substrate, the cost metrics — every one of those makes the *next* piece of work
cheaper. None of them retires a Story. A 10× loop attached to a queue that never drains
produces a longer queue, faster, and that is what the register now shows: `FEAT-P1-10` gained
five Stories in two days and closed none of them.

That is not an argument against the tooling — the tooling is why three board verdicts fit in an
afternoon. It is an argument that **the next session must be a closing session**, and that
closing needs a mechanism rather than an intention, because intention is what the last four
handovers already supplied.

## 4. The next session — a closing pass, in this order

### 4.1 Advance every Story whose criteria are already met (hours, not days)

For each of the 31, read its criteria against filed evidence and do exactly one of:

- **All met** → advance the header to `Verified`, cite the Report or `BOARD VERDICT`, done.
- **Not met** → write the **one** missing thing into the Status header as a single sentence.

There is no third option, and "still in progress" without naming the missing item is not one.
Candidates with evidence already filed and headers not moved:

| Story | Evidence on record |
|---|---|
| `STORY-P1-07-03` | *every* criterion Green on silicon (`BOARD VERDICT 5`, `8`) |
| `STORY-P1-07-04` | all five criteria (`count=1816 rmin=999 rmax=1000`) |
| `STORY-P1-07-02` | criterion 2 — `far=0x20_0000_0000`, `HALTED REASON=NO-RESUME-PATH` |
| `STORY-P1-07-06` | envelope parsed off the wire 2026-08-05, criterion 1's strongest form |
| `STORY-P1-10-01` | 13 tests + board-decoded frames |
| `STORY-P1-10-04` | criteria 1–5, 7 Green (`BOARD VERDICT 11`–`13`) |
| `STORY-P1-09-*` | 15 Stories against a board that has been beaconing since 2026-08-04 |

**Expect this to move 10–20 Stories.** If it moves two, the criteria were never satisfiable and
*that* is the finding.

### 4.2 Close the gate that let it happen (`LE-65`'s other half)

`STORY-P0-01-10` refuses a `Specified` header on a Story with a passing Report. Extend it: **an
`In progress` Story whose Test document has no unmet clause and whose Report records a pass must
fail the spine.** Then a satisfied Story cannot sit unclaimed, and this handover cannot need
writing twice.

### 4.3 Start the qualification campaign, or restate the Epic's mandate

This is the owner's decision and it should be made deliberately:

- **Start `Q2`** — `ADR 0005`'s qualification campaign for `rpi5-bcm2712`, which is the only
  route to a non-zero `qualified-platforms.tsv` and therefore the only route to any assurance
  `verified` row, ever.
- **Or restate what `EPIC-P1` discharges without it**: functional `Verified` plus
  `guardrail-evidence.tsv` rows (11 of 460 today), with *Determinism Proof* explicitly meaning
  *measured and stated*, not *bounded and qualified*.

**Doing neither leaves `EPIC-P1` unable to complete by its own definition**, which is the
structural version of the owner's observation and the thing most worth fixing this week.

## 5. What is genuinely done and should not be re-litigated

- **TinyOS boots over Ethernet.** Card swap over. `BOOT_ORDER=0xf12`, SD fallback proven twice.
- **A hardware tier exists.** `REPORT-2026-08-04-01`; `LE-09`, `LE-15`, `LE-24`, `LE-27` closed.
- **The spoor substrate is board-proven and self-measuring** — stamp 138 cycles/op, announce
  3101, drain 121955.
- **`FEAT-P1-11` is implemented and one power cycle from evidence** — image `f8133b0958d3`
  staged and served.
- **SharCrust is the RT proving ground** ([`docs/tinydb-rt-scope.md`](../../docs/tinydb-rt-scope.md)),
  staged, with the licence contradiction that blocks stage 1 named.

## 6. The measure this session should be judged by

Not commits, not board verdicts, not tooling. **The count of Stories at `Verified` under
`EPIC-P1`, which is currently zero**, and the count of `guardrail-evidence.tsv` rows, currently
11 of 460.

If the next handover reports new capabilities and those two numbers are unchanged, the loop got
faster and the project did not.

## 7. Bench facts at close

Unchanged from [`05A`](05A-one-power-cycle-from-the-kernel-driving-the-machine.md) §6 — board
powered on the previous netbooted image, card in the Pi in Pi OS role as fallback, `f8133b0958d3`
staged and served, `sudo -n tos64-probe` passwordless, and do not read the AVS debugfs regmap.
