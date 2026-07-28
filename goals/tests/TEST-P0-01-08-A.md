# TEST-P0-01-08-A — The Dashboard Stops Being Hand-Maintained

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P0-01-08`](../stories/STORY-P0-01-08.md)
Tier: Host unit tests only — the subject is the agreement between two files in this repository, and no CPU this project lacks is involved
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`, `BND-18`
Protection Domain contracts: `PD-02`, `PD-12`, `PD-13`, `PD-14`
Code admission gates: `RCG-05`, `RCG-06`, `RCG-07`, `RCG-12`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

`LE-30` says [`goals/index.html`](../index.html) is a rendering of data the tooling already has, kept
by hand, and that it drifts. Nine sessions of evidence now sit behind that sentence, and
[Handover 41A](../../session/hand-2026-07-28/41A-the-dashboard-as-a-work-order.md) supplied the
sharpest instance: it re-synced the page and **two of its figures went stale while the sync was being
written.**

The design question this document settles is *what may be generated*. The page is not a report; it
is an argument, with paragraphs explaining why `0 / 59` is the right number and why the reason
usually given for it is wrong. **A generator that owned the whole page would destroy the thing that
makes it worth reading.** So the split below is deliberate and is the specification's main content:
tiles are generated, claims are gated, prose is left alone.

Clause 5 carries forward `STORY-P0-01-07` clause 2 and `ADR 0005`'s trap: once this Story fixes the
tree, every check here passes by construction, and a green `check-assurance-spine` is not evidence
that any of them can reject. The tests are the evidence.

## Specification

### 1. The tiles are generated, and the generator prints the fix

**Given** `cargo run -p xtask -- emit-dashboard`,
**then** it prints the stat-tile block — begin marker, one `<div class="stat">` per spine figure, end
marker — computed from the same walk `check-assurance-spine` performs.

**And** `check-assurance-spine` locates that marked region in the committed page and **byte-compares**
it against what the emitter produces. A mismatch is refused, and the error carries the expected block
verbatim, so the fix is in the message rather than in the reader's memory of which tile moved.

**And** a page carrying no begin marker is refused, naming the command that produces one; a page that
opens the region and never closes it is refused separately, because a truncated region and a stale
region are different defects and a single message for both would misdirect.

**And** a CRLF checkout is not a defect. The comparison normalises line endings before matching; this
repository is worked on Windows and a line-ending difference is not drift.

**And** `emit-dashboard` **does not run the dashboard check**. This is the clause most likely to be
"simplified" away by a later reader: the command exists to print the block that repairs a stale page,
so making it depend on the page being fresh would make it refuse to run in the only situation it is
for. It also **does not write the file** — a command that rewrites the page a reader meets first
should be one someone chose to apply, and the diff is the review.

### 2. The prose is gated, not generated

**Given** the paragraph restating the spine counts,
**then** `check-assurance-spine` requires it to contain the current `N Features / N Stories / N Tests /
N Reports` and `N loose ends (N open)` figures, and refuses the page when either has gone stale.

**And** it changes nothing else in that paragraph, and no check in this Story rewrites any prose.
The argument on that page is written by people; what this Story removes is the ability for its
*numbers* to contradict the register they claim to summarise.

### 3. Badges agree with the Story they label (`LE-44`'s rule, one document along)

**Given** every `<a href="stories/STORY-….md">…</a>` immediately followed by a
`<span class="badge …">` on that page,
**then** the badge text must open with the spelling of that Story's own `Status:` state.

**And** a badge may append a tier — `FUNCTIONALLY VERIFIED (Tier 0 + Host)` — because the tier is
genuinely extra information the header also carries. It may **not** name a different state:
`VERIFIED` where the Story says `Functionally Verified` is refused, which is the overstatement
`LE-44` found in the Feature tables and which this check found seven of on the dashboard.

**And** a Story linked in prose *without* a badge is making no state claim and is not checked. Without
this the page could not mention a Story in a sentence.

**And** a badge naming a Story with no document is refused.

**And** the state-to-badge mapping is explicit rather than an uppercasing of the state string, so that
adding a `Status:` state is a decision about how the dashboard should say it rather than a string
transformation nobody reviewed. Every state in the vocabulary has a spelling, asserted by test.

**And** the check is applied to the committed tree: the seven overstatements are corrected toward the
Story, which is authoritative about its own state. **Grandfathering them would make the gate green on
day one and blind to exactly the drift it exists to catch** — the same decision `STORY-P0-01-07` took
and the same reason.

### 4. `41A`'s reachability count is derived rather than asserted

**Given** the performance catalogue and the set of domains at least one Story contract selects,
**then** the split is computed: release gates **in play**, those **reachable** because their `tier`
names `Host` or `T0`, and those **hardware-only** because it names neither.

**And** `G24`/`G25` are excluded by the `gate` column, being *claim* gates that run only after the
absolute release gates pass.

**And** `HIL` is not treated as reachable. The HIL rigs are CAN/USB hardware-in-the-loop deferred to
Phase 3 and this project has none, so a `Host+T0+HIL` row is reachable on the strength of its
`Host`/`T0` half and a `T1+T2+HIL` row is not reachable at all.

**And** the result is asserted against the hand count in `41A` §2 — **391 in play, 345 reachable, 46
needing a board** — and `reachable + hardware_only == in_play`. From here the figure is derived, and
the `345 / 391` ratio becomes a tile beside `11 / 391`.

### 5. Every refusal is demonstrated

**Given** that this Story's own changes make the committed tree satisfy clauses 1–4,
**then** a passing `check-assurance-spine` is **not** evidence that any of those checks can reject,
and no Report may cite it as such.

**Therefore** host tests drive each refusal with a fabricated input: a stale tile, an absent region,
an unclosed region, a stale count sentence, a stale loose-end count, an overstated badge, a badge for
a nonexistent Story. **And each has an acceptance case beside it** — the emitted block passing its own
check, a CRLF page passing, agreeing badges with and without a tier suffix. A check that only ever
rejects is as uninformative as one that only ever accepts.

## What this test explicitly does not establish

- **That the page is complete or correct as prose.** It establishes that the page's *numbers and
  state claims* cannot contradict the spine. A tile that is accurate and an argument that is wrong
  look identical to every check here.
- **That the per-Story tables are generated.** They are not. Their rows, prose and Report links stay
  hand-written and can still be incomplete; what they can no longer be is contradictory.
- **That `345` gates are easy.** The tile reports that no board is required. `LE-42` is what a
  Tier-0-reachable gate looks like when someone finally measures it.
- **Anything about `README.md`.** `LE-34` is the same failure mode in a third document and stays open.
- **Any performance guardrail.** None closes here and no Story's assurance state moves.

## Reports

- [`REPORT-2026-07-28-11`](../reports/REPORT-2026-07-28-11.md)
