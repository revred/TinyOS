# The Relay — Two Agents, One Baton

Binding, like [`CODING_STANDARDS.md`](CODING_STANDARDS.md) and
[`CONCURRENT_SESSIONS.md`](CONCURRENT_SESSIONS.md). Short on purpose.

## Why this exists

[`CONCURRENT_SESSIONS.md`](CONCURRENT_SESSIONS.md) ends by naming what it cannot
fix: *"Genuine concurrent editing of one file still needs the sessions to talk,
and there is no mechanism in this repository for that."* The relay does not
build that mechanism. It **removes the need for one**, by guaranteeing that only
one agent ever writes.

The problem the relay actually solves is a different one, and it is the one that
ends every session in this repository: **context exhaustion**. Work stops not
when the work is done but when the agent runs out of room — and the handover
that carries the state forward is therefore written by the least capable version
of the agent that did the work, at the exact moment it has least room to be
careful. Every failure this register keeps recording has the same shape: *a write
acknowledged, a readback never taken*. A handover flung over the wall at 95%
context is that shape applied to knowledge.

So: two agents, one baton, and the receiver verifies before the sender forgets.

## The invariants

These do not bend, in this order (`CODING_STANDARDS.md`'s priority ordering
applies here exactly as it does to code).

1. **One baton. One writer.** The Standby agent does not write to the shared
   tree — not a file, not a commit, not a register row. Every incident in
   `CONCURRENT_SESSIONS.md` becomes structurally impossible rather than merely
   discouraged.
2. **The tree is never handed over red.** Gates green at the seal, or the red is
   named in the handover *as the next action*. A handoff mid-red hands over a
   defect wearing progress's clothes. This is the board's "never leave it off",
   applied to the tree.
3. **Reset is gated on ACCEPT, never on send.** The Holder keeps its context
   until the receiver has verified the handover and said so. A sender that
   resets on send has destroyed the only thing that could answer the receiver's
   questions — the same defect class as believing a command's own response
   instead of a readback (`LE-87`).
4. **The handover is written continuously, from the first tool call.** It is a
   living document that is finalized at the seal, never composed at it.
5. **Prose proves nothing.** Every state claim in a handover must be
   re-derivable by a command against the repo. Where prose and the machine
   disagree, the machine wins and the prose is repaired.
6. **Findings go to the register, not the prose.** Prose is summarized away
   across cycles; `LE-*` rows are not. Anything discovered and not fixed is a
   row *before* the handoff, not a paragraph someone will lose in four cycles.

## The cycle — two context lifetimes per agent, per turn

At any instant one agent is the **Holder** (owns the baton, is the only writer)
and the other is the **Standby** (empty context, no writes). Roles are positional
and swap every pass. Neither agent is senior.

### Context 1 — AUDIT (small, read-mostly, capped)

The receiving agent wakes fresh and reads *only* the incoming handover,
[`agent.md`](../agent.md), and the repository.

1. **Re-derive the claims.** `git log`, `git status`,
   `cargo run -p xtask -- list-status`, `check-spine-files`,
   `check-assurance-spine`, plus whichever of `check-boot-images`,
   `check-guest-images` and `check-tool-tests` the incoming diff touched. The
   handover said the tree is green; find out.
2. **Apply the three acceptance tests** (below).
3. **Repair the handover in place.** The *receiver* owns making the document
   sufficient, because the receiver is the only one who can tell whether it is.
   A sender cannot audit its own handover — it knows what it meant.
4. **Commit the repaired handover**, then emit the verdict:
   - **ACCEPT** — the sender may now reset.
   - **REJECT**, with specific questions. The sender still holds its context and
     answers them; the audit runs once more. **Two rejections escalate to the
     owner** rather than a third round — fail-safe over keep-trying, and a
     handover that cannot survive two audits is a scope problem, not a writing
     problem.
5. **Reset.**

### Context 2 — WORK (large, fresh)

The same agent wakes again with nothing carried over, and reads only the
*repaired* handover, `agent.md`, and the repository.

This reset is the point, not an overhead. It does three things:

- **It proves the document.** If the work context has to ask the previous agent
  a question, the handover was insufficient — and that is a defect recorded
  against the *audit* pass, which accepted it, not against the sender.
- **It maximizes the budget** that actually matters. The audit context is full of
  the previous session's narrative, half-read files and the auditor's own
  discarded hypotheses; carrying that into the work halves the usable room.
- **It breaks inherited framing.** An auditor who spent a context reconstructing
  someone else's plan is biased toward continuing it. The work context starts
  from the stated next action, not from the reconstruction.

Then: work, maintaining the running handover from the first tool call, and seal
at the budget threshold. The baton passes and the roles swap.

## The three acceptance tests

A handover is accepted only if all three hold. Anything unresolved that is not
the stated next action becomes an `LE-*` row before ACCEPT.

1. **Re-derivable.** Every state claim matches the machine — Story headers, gate
   results, register rows, commit graph.
2. **Executable.** The next action can be *started* without asking the sender
   anything: named files, named commands, named Story and criterion.
3. **Bounded.** It states what it refuses, so the work context cannot drift into
   new surface. The sprint rule and the owner's sentence govern here unchanged.

## The seal — what the Holder does before handing off

- Finish the unit of work, or **park it explicitly**: what state it is in, why it
  stopped, and what the next agent must not assume about it.
- Gates green, or the red named as the next action (invariant 2).
- Committed, staged narrowly — `CONCURRENT_SESSIONS.md` rule 1 still applies and
  is now cheap, because nobody else is writing.
- The running handover finalized with: the one sentence; what changed; what is
  owed; **what it refuses**; the register delta; and each gate with the command
  that produced its result.
- `session/RELAY.md` updated (below).

## Budget discipline

"Bandwidth" here means the context budget, and it is spent in a fixed order:
**seal first, work second.** Concretely:

- **Declare the handoff at roughly 60–70% consumed, not at exhaustion.** Sealing
  is not free: finishing the unit, running the gates (whose output is itself
  large), writing the document and answering an auditor's questions all happen
  *after* the declaration.
- **Refuse rather than clamp.** If you find yourself past the threshold with the
  unit unfinished, seal *immediately* around a smaller unit. Do not press on and
  compress the handover — the compressed handover is the failure this whole
  protocol exists to prevent.
- **The audit context is capped deliberately.** It is read-mostly and should end
  well under its limit; an audit that needs a large context is reporting that the
  handover was unclear, and the correct output is REJECT.

## Visibility for the owner

Four artifacts, no dashboard to build:

- **`session/RELAY.md`** — one live file, always current, rewritten at every
  phase transition. It is the process's own `TOS64-RX/1` row: a state a reader
  can check at a glance rather than infer.

  ```markdown
  # Relay state
  Baton:        Agent Beta            (Alpha: Standby, reset 14:02)
  Phase:        WORK                  (AUDIT accepted 14:05)
  Since:        2026-08-08 14:07
  Budget:       ~35% consumed
  Handover:     session/hand-2026-08-08/03A-...md
  Story:        STORY-P1-09-17, criterion 4
  Next action:  <one line, executable>
  Gates:        spine ok · boot-images ok · tool-tests ok  (14:05)
  Owed:         LE-118 disposition (owner)
  ```

- **A commit per phase transition**, with a conventional subject — `relay: seal`,
  `relay: accept`, `relay: reject`, `relay: park` — so `git log` is the audit
  trail and no phase change is invisible.
- **One git worktree per agent**, so two agents' diffs can never interleave even
  by accident.
- **The dated `session/hand-*/index.html`**, unchanged, as the human history.

## Naming — the trap worth naming

**The handover letter marks concurrency at a number and nothing else** (the
owner's 2026-08-07 amendment; see [`session/README.md`](../session/README.md)).
A relay has exactly one writer at a time, so its handovers are **sequential**:
`17A`, `18A`, `19A`. Never `17A`/`17B` for "the two agents", and never a letter
carried as an agent's identity — that is precisely the mistake that produced
`08G` and had to be corrected the same day.

Agent identity belongs in a header field of the handover (`Held by:`), never in
the filename.

## Why this is sustainable

Each failure mode and the refusal that holds it:

- **Telephone-game drift**, the thing that kills every relay → prose proves
  nothing; the audit re-derives from the machine every single pass.
- **Review theatre** → ACCEPT requires re-run gates *and* a repaired document. An
  audit that claimed to verify and changed nothing is visible in the diff.
- **Infinite ping-pong** → two rejections escalate to the owner.
- **Scope creep compounding across cycles** → every handover names its refusals,
  and new surface still needs the owner's sentence.
- **Knowledge lost at reset** → the register carries findings, not the prose.
- **Cost** → the expensive context is the work one, and it is maximal *because*
  the audit was reset away. The audit itself is read-mostly and short.

## What the relay does not do

- **It does not make work parallel.** Throughput is roughly one agent's, plus a
  quality gate. What it buys is *continuity across context limits* and an
  independent reader of every handover — not speed.
- **It does not replace the owner's sentence, the sprint rule, or the gates.**
- **It adds no new machine gate on day one.** The natural mechanization, once the
  loop has run enough times to know its own shape, is
  `cargo run -p xtask -- check-relay`: the handover carries its required
  sections, its claims agree with the spine, and `session/RELAY.md` is not stale.
  Folklore becomes a gate — this project's standard move, and deliberately not
  taken before the folklore exists.

## Alternatives rejected, and why

1. **Both agents working at once on different Stories.** Rejected on this
   repository's own evidence: `CONCURRENT_SESSIONS.md` records what one day of
   that cost. The relay's value is that the second agent is **fresh**, not that
   it is parallel.
2. **Sender writes the handover and resets immediately on send.** Rejected: it
   destroys the only context that can answer the receiver's questions, and it
   reports a handoff as complete on the strength of having sent it. That is
   `LE-87` with knowledge in place of a relay state.
3. **No reset between the audit and the work.** Rejected: it carries the
   auditor's reconstruction into the work, halves the usable budget, and — worst
   — removes the only test that the handover was ever sufficient.
4. **One agent with periodic self-summarization.** Rejected: a summary written by
   the agent that is running out of room is written by the least capable version
   of it, and nothing independent ever checks it. This is the status quo, and it
   is what the relay replaces.
