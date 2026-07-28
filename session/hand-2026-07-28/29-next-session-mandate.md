# Handover 29 — Next-Session Mandate: Two Decisions Are Owed Before Any More ARM64 Code

The start-here document written at the close of this session. `main` is at the commit this handover
lands in, pushed. **This session wrote no kernel code.** It ran two external reviews — an
architectural/comparative analysis and an expert code audit — through the register, and it ends with
**nine new loose ends and two open decisions that block work already scheduled.**

**On the folder date.** This repository's document dates run one day ahead of the clock, as
Handover 13 §"A note on dates" records.

## Read these first

1. **This document's §"Two decisions are owed"** — `LE-39` and `LE-41`. Both were raised by outside
   reviewers, both have a recommendation, neither has been decided, and one of them is
   time-sensitive in a way nothing else in this register is.
2. [`28-analysis-response-and-le-33.md`](28-analysis-response-and-le-33.md) — the disposition of the
   comparative analysis, and where `LE-33`, `LE-34`, `LE-35` and `LE-36` came from.
3. [`27-story-p1-07-02-host-half.md`](27-story-p1-07-02-host-half.md) — what actually exists in
   `hal-arm64`, from the session that built it.
4. [`26-next-session-mandate.md`](26-next-session-mandate.md) — **still authoritative for the board
   traps.** Nothing here supersedes its trap list; this document adds to it.

## Where the project stands

```text
main                    pushed, level with origin
assurance spine         23 Features, 57 Stories, 44 Tests, 45 Reports
                        41 loose ends (30 open), 83 status headers
                        10 release gates with dated evidence, of 391
host tests              549 across the workspace; hal-arm64 at 115
STORY-P1-07-01          In progress — blocked on a USB-serial adapter
STORY-P1-07-02          host half complete; clause 2 needs the board
Stories verified        0 / 57
LE-09                   OPEN
```

**The blocker is still one physical object.** That has not changed, and this session did not try to
change it.

## Two decisions are owed

Neither is a coding task. Both block work that is already scheduled, and an agent that starts coding
without settling them will do work that has to be retracted.

### 1. `LE-39` — `ADR 0004`'s premise does not hold, and every ARM64 timing claim waits on it

`ADR 0004` makes ARM64 the real-time tier of record because x86 SMIs are invisible, unmaskable and
unattributable. Its load-bearing sentence is *"Interrupt masking at `EL1` means what it says."*

An external audit answered the question [Handover 28](28-analysis-response-and-le-33.md) put to it,
and **the sentence is not true.** GICv3 Group 0 / Secure Group 1 interrupts routed by `SCR_EL3.FIQ`
preempt NS-EL1 irrespective of `PSTATE.I`, consume cycles `EL1` cannot attribute, and perturb cache
and TLB state. That is structurally the same hole the ADR disqualifies x86_64 for.

**Recommendation: `ADR 0005`, superseding `0004`** — not a silent edit, because Reports already cite
`0004` and editing it rewrites what they cite. `ADR 0005` should apply `ADR 0004`'s *own*
falsifiability test to ARM64: the real-time tier becomes **conditional on per-platform secure-world
qualification** rather than automatic.

Mandating bare-metal `EL3` was considered and rejected: **Pi 5 firmware owns `EL3` and cannot be
displaced**, so mandating it makes the RT claim unreachable on the only board in hand.

Two things worth carrying into that ADR:

- It does not invalidate a single existing measurement. Like `ADR 0004`, it constrains what may be
  *promoted* into a bound.
- **The cost is also the moat.** Per-platform qualification evidence is what commercial RTOS vendors
  charge for. The honest technical position and the defensible commercial one are the same position;
  `ADR 0005` should say so, so nobody later reads it as a retreat.

### 2. `LE-41` — the licence, and the only deadline in this register

`os/Cargo.toml` declares `license = "MIT"`. **There is no `LICENSE` file in the repository.**

The file itself is minutes and should not wait on anything. The strategy is the decision: MIT is
permissive, so anything published under it can be forked and closed by a silicon vendor or a
competitor — the outcome an open-core or dual-licence model exists to prevent. **Relicensing is
possible only while authorship is single-source. That window closes permanently the first time an
outside contribution lands, and nothing in this repository will announce that it has closed.**

Every other row in the register can be deferred at a known cost. This one cannot.

## What to do

**If board time is possible, take it — the mandate has not changed.** [Handover
26](26-next-session-mandate.md) §"If an adapter *is* in your hand" is still exactly right, and one
board session now closes both `STORY-P1-07-01`'s capture and `STORY-P1-07-02`'s clause 2.

**If there is no board time**, in this order:

1. **Settle the two decisions above.** They are cheap in hours and expensive in rework.
2. **`LE-31`** — the audit, whose first pass this session already did (Handover 28). Eight of nine
   Stories are blocked by HIL, not by `LE-09`; `STORY-P0-05-01` is the sole candidate needing no
   hardware and would move `Stories verified` off zero for the first time.
3. **`LE-23`** — re-record the timing baseline from a CI run. `LE-24` may come free with it.

## The nine new rows, and what they are not

`LE-33` through `LE-41`. Three came from an architectural analysis, five from an expert code audit,
one from a commercial review. **None of them is a kernel bug found by running anything**, and that
distinction matters when triaging them:

| Row | What it is |
| --- | --- |
| `LE-33` | `ADR 0004` has no enforcement machine |
| `LE-34` | README target list drifts because it is prose; sibling of `LE-30` |
| `LE-35` | Selecting a `design`-readiness domain needs a rule nobody wrote |
| `LE-36` | This session broke another session's gate; `CONCURRENT_SESSIONS` rule 8 |
| `LE-37` | `CPACR_EL1` never initialised — **severity overstated by the reviewer** |
| `LE-38` | No emergency fault stack on ARM64 — **`LE-04` on a second architecture** |
| `LE-39` | `ADR 0004`'s premise unsound — decision owed |
| `LE-40` | `grant` panics rather than failing closed — **downgraded from the reported TOCTOU** |
| `LE-41` | Declared licence, no licence file — decision owed, time-sensitive |

Two of the audit's findings were **corrected on inspection**, and the next session should know which,
because both were reported with more confidence than the code supports:

- **`LE-37` is not the trap it was reported as.** The claim was that Rust SIMD emitted for zeroing or
  formatting traps immediately at `EL1`. It does not: `targets/aarch64-tinyos.json` sets
  `"abi": "softfloat"` and `"features": "+v8a,+strict-align,-neon"`, so **none is emitted**. The real
  defect is thinner and more interesting — a JSON build flag is standing in for a hardware
  initialisation, and nothing enforces the flag.
- **`LE-40` is not a TOCTOU.** It was reported as a compromised C4 domain modifying payload headers
  mid-validation. The code is page-table translation, not payload parsing; `owner_space` is held
  under a shared borrow on a single core; no C4 mutation path exists. What is real is a `.expect()`
  on a kernel path — against `fail-safe over keep-trying` — resting on an unstated
  no-concurrent-mutation assumption that SMP would invalidate silently.

`LE-22` was independently confirmed by the same audit, which supplied the fix shape the row lacked:
a **dynamic effective priority** function rather than a stored `original_priority` restore. The row
now carries it.

## Traps, named up front

**1. Do not patch `LE-37` or `LE-38` directly.** Both are real; neither is in any clause of
`TEST-P1-07-02-A`, and that document is not ours to extend. Rule 3 is not suspended because a defect
is obvious: **the Red comes first.** `FEAT-P1-07` §6 also says a seventh Story means re-decomposing,
so `LE-38` in particular is a scope decision, not a diff.

**2. An external reviewer's confidence is not evidence.** Two of six code findings this session did
not survive contact with the file they described, and one of those was reported as a security
vulnerability. The findings were still worth having — `LE-39` alone justified the whole review — but
**every one of them was checked against the tree before it was registered, and two changed shape when
it was.** Do that again.

**3. `LE-39` does not invalidate any existing measurement.** It constrains what may be *promoted*
into a worst-case bound. If you find yourself retracting Tier 0 numbers, you have misread it exactly
as `ADR 0004` was misread.

**4. The commercial case rests on the one domain nothing owns.** Every market argument reduces to
*inference does not perturb the RT loop* — that is `D17` and `G19`. **`D17` is selected by zero
Stories** and `LE-35` says the obvious fix is unsafe as written. Do not let a business conversation
create the impression that this is evidenced. It is the least-evidenced claim in the catalogue.

**5. Rule 8 exists now, and this session is why.** Validate a hand-edited machine-checked file before
your next tool call. This session broke another session's gate by dropping a tab and did not notice
for several steps.

## What this session actually changed

No kernel code. What changed is what the next session *believes*:

- `ADR 0004` is no longer safe to build on unamended.
- The README no longer claims an ARM64 CI gate that does not exist.
- `LE-31`'s attribution is now known to be wrong in a way that closing `LE-09` will not fix.
- `CONCURRENT_SESSIONS.md` has a rule for the failure this session caused.
- Nine rows exist that were, a day ago, paragraphs in someone's analysis.

That last one is the pattern worth keeping. **`LE-34`, `LE-35` and `LE-36` all say the same thing
from different directions: a decision without a machine behind it decays, and a finding that stays in
prose stops being read.** This session ended by registering its own findings rather than describing
them in a handover, which is the only reason you are reading them as rows.
