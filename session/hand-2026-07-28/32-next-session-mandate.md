# Handover 32 — Next-Session Mandate: The Fallback Ran Out, and That Is Good News

The start-here document. `main` is at `2a5a867`, pushed, level with `origin`. This supersedes
[Handover 29](29-next-session-mandate.md) as the mandate; **29's two decisions are still owed and are
restated here in full**, so you do not need to open it unless you want the reasoning.

**On the folder date.** This repository's document dates run one day ahead of the clock, as
Handover 13 §"A note on dates" records. Do not read a date here as evidence of when anything happened.

## First, two minutes of setup

```text
git config core.hooksPath .githooks
```

Per-clone, so a fresh clone does not have it. Then read
[`agent/CONCURRENT_SESSIONS.md`](../../agent/CONCURRENT_SESSIONS.md) — **eight rules now**, it is
binding, and it is short. Rule 8 was added because a session dropped a tab in a machine-checked TSV
and broke a *different* session's gate for several tool calls. Slot 33 is free.

## Where the project stands

```text
main                    2a5a867 (green, pushed)
assurance spine         23 Features, 58 Stories, 45 Tests, 46 Reports
                        42 loose ends (31 open), 84 status headers
                        11 release gates with dated evidence, of 391
host tests              549 across the workspace; hal-arm64 at 115
Stories verified        0 / 58
STORY-P1-07-01          In progress — criteria 3 and 4 need a board + adapter
STORY-P1-07-02          In progress — criterion 2 needs a board; no version passes without it
STORY-P1-03-02          In progress — criteria hardened after pre-implementation review
LE-09                   OPEN
```

**The blocker is still one physical object: a loopback-tested USB-serial adapter.** That has not
changed for four sessions. What *has* changed is everything around it.

## The one thing to understand before choosing work

**Handover 26's fallback list is spent, and Handover 29's has one item left.** That is not a problem;
it is the result of three sessions doing exactly what they were told. But it means the honest
statement of this session's options is different from the last four:

- **`LE-31` is no longer "the clearest non-hardware work in the project."** It was, for three
  sessions. `STORY-P0-01-06` finished the one slice that could be finished, and the audit's own
  conclusion is that **the remaining eight Stories are the HIL-blocked ones** — they need a
  hardware-in-the-loop rig this project does not have and will not have before Phase 3. More auditing
  will not move them.
- **`LE-23`** (re-record the timing baseline from a CI run) is still real, still unowned, and is now
  the *only* item on Handover 29's fallback list that has not been overtaken.
- **What replaced them is better work**, and it is in §4.

## The two decisions, still owed

Neither is a coding task. Both block work that is already scheduled, and an agent that starts coding
without settling them will do work that has to be retracted.

### 1. `LE-39` — `ADR 0004`'s premise does not hold

`ADR 0004` makes ARM64 the real-time tier because x86 SMIs are invisible, unmaskable and
unattributable. Its load-bearing sentence is *"Interrupt masking at `EL1` means what it says."*

Against GICv3 Group 0 / Secure Group 1 interrupts routed by `SCR_EL3.FIQ`, **it is not true** —
secure-world interrupts preempt NS-EL1 irrespective of `PSTATE.I`, consume cycles `EL1` cannot
attribute, and perturb cache and TLB state. That is structurally the same hole the ADR disqualifies
x86_64 for.

**Recommendation on file: `ADR 0005` superseding `0004`** — not a silent edit, because Reports
already cite `0004` and editing it rewrites what they cite. `ADR 0005` applies `ADR 0004`'s *own*
falsifiability test to ARM64: the real-time tier becomes **conditional on per-platform secure-world
qualification** rather than automatic. Bare-metal `EL3` was considered and rejected — **Pi 5 firmware
owns `EL3` and cannot be displaced**, so mandating it makes the RT claim unreachable on the only
board in hand.

Two things to carry into that ADR:

- **It invalidates no existing measurement.** Like `ADR 0004`, it constrains what may be *promoted*
  into a bound. If you find yourself retracting Tier 0 numbers you have misread it.
- **The cost is also the moat.** Per-platform qualification evidence is what commercial RTOS vendors
  charge for. Say so in the ADR, so nobody later reads it as a retreat.

### 2. `LE-41` — the licensing model

The `LICENSE` file now exists (`b663376`), so the diligence half is closed. **The strategy is not.**
`os/Cargo.toml` declares MIT; MIT is permissive, so anything published under it can be forked and
closed by a silicon vendor or a competitor — the outcome an open-core or dual-licence model exists to
prevent.

**This is the only row in the register with a window that closes silently.** Relicensing is possible
while authorship is single-source, and nothing announces the first outside contribution. Every other
row can be deferred at a known cost. This one cannot.

## What to do

**If a serial adapter is in your hand, take the board.** Handover 26 §"If an adapter *is* in your
hand" is still exactly right and is not superseded: loopback-test the adapter first, `os_check=0` in
`config.txt`, read the `current_el=` line, quote the capture verbatim. **One board session now closes
both `STORY-P1-07-01`'s capture and `STORY-P1-07-02`'s clause 2**, because both host halves are done.
That is the highest-value session available to this project by a wide margin.

**If there is no board time**, in this order:

1. **Settle the two decisions above.** Cheap in hours, expensive in rework, and `LE-39` blocks every
   future ARM64 timing claim — including the ones the board session will produce.
2. **Take the `-M virt` fixture.** [Handover 31](31-qemu-virt-fixture-scoping.md) is the scoping;
   §7 lists four decisions to settle before writing anything. This is the work that replaced
   `LE-31`, and it is better than what it replaced: it finds, on a host in seconds, the defect class
   that otherwise costs a whole board session and produces no diagnostic at all.
3. **`LE-23`** — re-record the timing baseline from a CI run to remove the confirmed 23–53%
   Windows-vs-Linux offset. `LE-24` may come free with it. One fix, two rows.
4. **`LE-30`** — generate the dashboard's status tables from `list-status`. `goals/index.html` was
   hand-re-synced again in `0fdc154`; that is the fourth time, and the data is already emitted as TSV.

**Do not take a fallback if board work is possible.**

## The `-M virt` fixture, in one paragraph

Because it will be the most tempting item and the easiest to get wrong: it runs TinyOS AArch64 code
under `qemu-system-aarch64 -M virt` so mechanism defects are found on a host instead of a board.
**It produces no timing evidence, closes no release gate, and does not touch `LE-09`.** There are
zero ARM64 fixtures today — all 23 are x86_64 — while the next four Stories are all AArch64. The
dependency that scoping found: **nothing in this workspace produces an AArch64 executable**
(`hal-arm64` is `[lib]` only, `kernel`'s one binary is x86_64), so it needs a binary crate currently
inside `STORY-P1-07-05`'s scope. That separates cleanly — `virt` takes an ELF and needs no SD image —
and building it first *de-risks* `-05`. Read Handover 31 before touching it; do not reconstruct its
reasoning from this paragraph.

## Traps, named up front

**1. A green ARM64 fixture is not ARM64 coverage.** This is trap 1 because it is the one this session
has already been caught by once. The README claimed *"QEMU x86_64 + QEMU ARM64 — CI gate, every
commit"* until `7f26a3b`, an external reviewer read it in good faith, and it came back to us as
confirmation of a pipeline that does not exist. If you build the `virt` fixture, the sentence in
Handover 31 §1 goes in four places: the Story, the Test, the Report, and the fixture's own
description.

**2. Do not patch `LE-37` or `LE-38` directly.** Both are verified defects. Neither is in any clause
of `TEST-P1-07-02-A`, and that document is not yours to extend. **Rule 3 is not suspended because a
defect is obvious** — the Red comes first. `FEAT-P1-07` §6 says a seventh Story means re-decomposing,
so `LE-38` in particular is a scope decision, not a diff.

**3. An external reviewer's confidence is not evidence — and neither is your own.** Two of six code
findings from the expert audit did not survive contact with the file they described, one of them
reported as a security vulnerability. Every one was checked against the tree before registration and
two changed shape when it was. The narrower, more useful form of this, from the session that verified
the corrections: **a detector that returns nothing is exercised before its nothing is believed.**

**4. `LE-35` is now load-bearing, not theoretical.** Any new Story that selects a performance domain
pulls all 25 of its guardrails into its contract. If the domain's readiness is `design`, none of them
can be closed, and `LE-35` says the rule for initialising them as stated open debt **has never been
written down**. The `-M virt` Story hits this immediately. Writing the rule is part of the work, not
a distraction from it.

**5. Do not reach for `--update-baseline` locally.** It rewrites every measured row with whatever your
host produced. That is `LE-28`, and it is one command from turning the confirmed cross-host offset
into a false green.

**6. Validate a hand-edited machine-checked file before your next tool call.** Rule 8. The edit is
not finished until the file parses.

## What not to be misled by

- **`11 / 391` gates with evidence is not 3% of the way to release.** It is eleven gates that have
  evidence, against `0 / 58` Stories assurance-verified, which is the number that matters and has not
  moved.
- **`LE-09` is necessary and nowhere near sufficient.** `STORY-P0-01-06` found it to be the correct
  blocker for exactly one of `D09`'s 25 gates. The dashboard used to say it was "the single blocker"
  and that claim has been retracted twice.
- **The commercial case rests on `D17`, which zero Stories select.** Every market argument reduces to
  *inference does not perturb the RT loop*. That is the least-evidenced claim in the catalogue, and
  `LE-35` says the obvious fix is unsafe as written.
- **549 host tests, all green, establish almost nothing about behaviour on hardware.** The interesting
  question is not how many but which claims have no test at all.

## State at the close

```text
main                    2a5a867 — pushed, level with origin
last five commits       2a5a867 scope the -M virt fixture
                        52a5d20 pay down three loose ends by their cheap half
                        0fdc154 re-sync the goals dashboard
                        2531e1d file Handover 30
                        b663376 D09 measured; LE-09 wrong blocker for 24 of 25
open decisions          LE-39 (ADR 0005), LE-41 (licensing model)
best available work     a board session, if an adapter exists
next best               the two decisions, then the -M virt fixture
```

`goals/reports/_soak-p0-03-01.log` is dirty in the working tree and has been for several sessions. It
belongs to whoever is running that soak; leave it.
