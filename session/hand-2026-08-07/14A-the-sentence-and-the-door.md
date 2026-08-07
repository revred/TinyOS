# 14A — Cover note: the sentence, and the door it opens

Cover note for [`15A`](15A-the-first-conversation-the-workload.md), the next
session's workload. Written at the owner's ask, in this session's own words:
*give me the next session workload — 10× impactful — that opens the doors to
directly interacting as a human with TinyOS.*

## 1. The sentence, recorded

`10A` §3 S4 said the interaction chain waits on exactly one sentence, the
owner's and nobody else's, and gave its wording. Four handovers held that
line without eroding it by a byte. On 2026-08-07 the owner instructed the
next session to open direct human interaction with TinyOS — and this note
records that instruction as the scoped lift, in `10A`'s own words:

> **Sprint rule lifted for the interaction chain — `-16`'s re-arm, the
> admitted verb, the Ti64Dink console — and nothing else.**

Recorded as a reading of the owner's instruction, openly: if this overstates
it, strike this section and `15A`'s Parts I–II and V revert to S4-gated
exactly as `12A` left them, with nothing else in the workload disturbed. If
it stands, nothing further needs saying — the charter reading is already done
(`STORY-P1-09-17`, written to this boundary and waiting), the suite is
already specified (`TEST-P1-09-17-A`), and the building session starts with
zero derivation owed.

## 2. Why this session is 10×, said against the register rather than as mood

- **Every bench question this project has ever asked cost a power cycle.**
  The unit of interaction with TinyOS today is the *boot*: change a line,
  rebuild, netboot, capture, read. `M2` replaces that unit with the
  *exchange* — ask the running board, get an answer, on the wire, in a
  second. One session converts every future session's cost structure.
- **The OS becomes an OS.** `STORY-P1-09-16` said it first: everything
  downstream of "the board cannot be told anything" — that Ti64Dink cannot
  start, that every interaction costs a rebuild — follows from it. `03B` §5
  called an answered command the one thing standing between this project and
  an operating system rather than an instrumented machine. This is that
  session.
- **The ladder's top rungs are already paid for.** Outbound is
  self-verdicting; inbound is contained with its five verdicts
  machine-predicted; the authority model is `FEAT-P2-01`'s seam reused; the
  charter reading for the verb is filed; the operator's terminal has a test
  project, timestamps, and a parity guard. What remains is the smallest part
  by lines and the largest by consequence: an ear that stays armed, two
  verbs, and a prompt.
- **`M4` is the owner's own definition of arrival** — "launch the OS" on
  real silicon: the Ti64 console with a board behind it instead of a QEMU
  guest (`10A` §1). A human types; TinyOS answers. That is the door this
  workload opens, and nothing less is claimed for it.

## 3. What stays true even at 10×

The refusals are the product. The verb table stays two answer-only rows
because the wire peer has no identity (`PD-02`, read in `-17`); the answer
rate stays beat-bounded because an unauthenticated peer must not make the
board an amplifier; every refusal is spoken, named, on the wire, because a
refused command that vanishes reads as a dead board. `M5` — the remote
desktop — is named as the destination precisely so nobody starts it sideways.
And if Sitting B's qualification boots have not yet run, they ride the same
afternoon: `12A`'s manifest is unchanged and `15A`'s bench half is written to
absorb it, every power cycle still spent twice.

## 4. Read next

[`15A`](15A-the-first-conversation-the-workload.md) — the workload: five
parts, each with its tests named red-first, its wire lines stated, and the
register rows it closes. Laptop parts first (no board, no owner), bench part
last (five boots, the first conversation at the end of them).
