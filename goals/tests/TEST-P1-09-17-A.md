# TEST-P1-09-17-A — The Verb Table Refuses Everything It Does Not Name

Status: **Specified — the suite is fully specified here and deliberately not yet
written as code: its subject (the `TOS64-CMD/1` classifier and verb table) may not
exist until the owner's S4 sentence lifts the sprint rule for the interaction chain
([`10A`](../../session/hand-2026-08-07/10A-the-first-conversation-from-counted-frames-to-an-answered-command.md) §3 S4), and a committed test that cannot compile gates nothing. The building
session writes these clauses red first, verbatim, before the classifier exists.**
Story: [`STORY-P1-09-17`](../stories/STORY-P1-09-17.md)
Tier: Host unit tests (classifier totality, table denial, refusal taxonomy, rate
bound, host/board vocabulary parity) **plus** a Tier 1 board run witnessed on the wire
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`, `D20`
Security controls: `SEC-18`, `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: `D20` is selected as stated open debt
([`goals/assurance/open-debt.tsv`](../assurance/open-debt.tsv)); two answered verbs
are not a data path and close no guardrail. This Test raises no timing, throughput or
qualification claim. `PD-02`'s reading — why an unauthenticated wire peer bounds the
verb table to answer-only rows — lives in the Story and binds every clause here.

## Specification

### 1. The classifier is total over fixed offsets, and the discipline is load-bearing (`BND-03`, `SEC-19`)

**Given** the `TOS64-CMD/1` classifier as a pure function of the admitted payload,
**then** every byte string maps to exactly one of: a well-formed command (verb id,
fixed-width argument) or a distinct named refusal — wrong magic, undersize, oversize,
unknown verb. No value from the frame is used as an offset, length or address; the
verb id's table lookup is bounded by the table's own length. **Mutation arm:** a
deliberately widened read (one field past the fixed layout) fails a committed test —
the fixed-offset property is asserted, not narrated.

### 2. The table denies by default (`SEC-18`)

**Given** the verb table,
**then** exactly two rows exist (`PING`, `STATUS`); every other id — including
adjacent ids, zero, and the maximum — resolves to `UnknownVerb`; and each row is
read-only by construction, its answer assembled solely from data the board already
broadcasts (the sequence counter; the transcript's verdict line). A test enumerates
the table and fails if a row gains authority: any register write, state change or
capability reach from a verb handler is a red test, not a review comment.

### 3. Every refusal is spoken and distinct (`BND-17`)

**Given** each refusal class from clause 1 plus over-rate,
**then** each produces a distinct named refusal *in the answer channel* — wire-visible,
attributable to the refused frame's own content — and never a silent counter. The
refusal vocabulary is one table shared with the renderer, `LE-80`-style, with the
Ti64Dink half held to parity by test from day one.

### 4. The answer rate is bounded, and the bound fails safe (`SEC-20`, `PD-07`)

**Given** a flood of well-formed `PING`s,
**then** at most one answer leaves the board per park beat; excess admitted commands
are counted and refused as over-rate; and no error path transmits outside the bounded
answer slot. Amplification from an unauthenticated broadcast-capable peer is the
attack; the beat-bounded answer is the containment.

### 5. `-16`'s containment is inherited unchanged (`BND-06`, `BND-07`, `PD-10`)

**Given** the receive path with the command classifier behind it,
**then** the admission filter, the hardware address filter assertion, the size bound,
the one-descriptor ring and the terminal error arms are byte-for-byte the `-16`
discipline — asserted by the existing suite still passing, with no test weakened, and
the answer path proven not to alias `RECEIVE_MEMORY`.

### 6. Board: the first conversation (`BND-17`)

**Given** the board on the cable and `ti64dink` transmitting,
**then** `PING`'s answer names the sequence heard; `STATUS`'s answer replays the boot
verdict line; each host-sendable refusal arm produces its named refusal in the
capture; and the capture parses to its own verdict. `M1` and `M2` in one sitting
(`10A` §4 Sitting B item 4), riding the same boots as the Q3 campaign.

## Test type

Host unit tests written red first by the building session, in the crate the classifier
lands in (the `gem_receive` neighbourhood), plus the Tier 1 board run. Until S4 is
spoken, this document is the suite's normative form.

## Reports

To be filed with the board run.
