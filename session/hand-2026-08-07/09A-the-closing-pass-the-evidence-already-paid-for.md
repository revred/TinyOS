# 09A — The closing pass the evidence already paid for: STORY-P1-07-01 Verified on the wire, and two forks decided

Same session as [`08A`](08A-the-swot-answered-and-the-tree-is-one-again.md)
(follow-on document, next number, letter `A` per the corrected naming rule).
The owner's instruction, verbatim in spirit: *decide for me, and move the OS
closer to its goals now.* This document records the decisions and what closed.

**The one sentence, if only one survives:** *`STORY-P1-07-01` is Verified —
its two board criteria closed on the committed two-boot wire capture under a
dated channel amendment to `TEST-P1-07-01-A` (the `TEST-P1-07-03-A` precedent),
ending ten days in which the Story was blocked on a serial port that has never
produced a byte while the evidence it needed sat committed on another channel.*

## 1. The amendment, and why it is honest

`TEST-P1-07-01-A` gains a dated amendment (specification untouched, additions
below — the document's own 2026-07-28 precedent):

- **Clause 1** (adapter loopback) is retired with the channel; its purpose —
  never blame the board for a dead instrument — is carried by wire instruments
  that demonstrably return both answers (serve-log digests, `--until` exit 0/1,
  `parse-meas` refusing verdict-less captures).
- **Clause 3** closes on `TOS64-QUAL/1 boot_entry current_el=EL2 raw=0x…8
  now_at=EL1` — both boots, verbatim. The ordering half ("printed before
  anything else") is re-read as **captured at entry, reported when a channel
  exists**, because a wire that trains after boot cannot carry entry-time bytes
  — stated as a re-read, not silently weakened. `now_at=EL1` is the conditional
  drop observed, which is the risk the ordering existed to convert.
- **Clause 4** closes stronger than written: a digest-confirmed, machine-parsed
  envelope with its own verdict (`parse-meas` exit 0, `ok=true`).

What it does not do, stated in the amendment itself: no PL011 claim, no
`SEC-01`/`BND-01` movement, no timing claim, clause 6's hardware half not
claimed. Story, Feature row, dashboard badge, gated sentence, tiles, progress
bar, footnote and feasibility all advanced together; spine green at
**98 rows / 58 badges agreeing, 74/101 Stories functionally verified**.

## 2. The two decided forks

- **`STORY-P1-07-05`:** the amendment route. Criteria 2 and 3's purpose closes
  on the capture path that works (netboot + `ti64dink` + `parse-meas`); the
  serial half retires with `LE-47`. The Story stays In progress for a stated,
  small reason: the wire has demonstrated exit-0-on-verdict and
  exit-1-on-timeout, and criterion 3's remaining arms (reported failure,
  spoke-and-stopped) have not been driven over this channel — one board-session
  piece, plus the `TEST-P1-07-05-A` clause rewrite when they are.
- **`STORY-P1-07-07`:** *not* decided — its header says the disposition is an
  owner call, and that stands. What 2026-08-07 added: `FB=REFUSED` on the
  lit-canvas boot, so the mailbox refusal and a working display are now
  observed together on one boot — the refusal is firmware policy against this
  path, and no scanout debugging changes this Story's answer.

## 3. What was deliberately not advanced

`STORY-P1-07-02` (needs a real induced fault on the board — no 08-07 boot
induced one), `-03`/`-04`/`-06`/`-08`/`-09` (already Verified), and
`STORY-P1-13-01` (its own text forbids taking the containment decision as a
session tail-end). The next board session's cheapest criteria-closing boots,
in order: `-05`'s two remaining outcome arms, `-02`'s induced fault, and the
Q3 campaign specified in `08A` §5 — the last of which unlocks `FEAT-P1-06`'s
bound half and with it the Epic's exit.

## 4. Standing instruction earned

**Evidence keyed to a channel dies with the channel; evidence keyed to a fact
survives it.** Three Stories stalled for ten days because their criteria named
the PL011 rather than the fact it was to carry. When writing acceptance
criteria against instruments, name the fact and let the test document name the
channel — the channel is the amendable part, and the amendment precedent now
exists twice.
