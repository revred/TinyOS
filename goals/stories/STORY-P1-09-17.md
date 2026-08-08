# STORY-P1-09-17 — The Admitted Verb: One Command Answered End to End, Deny-by-Default

Status: **In progress — the sentence was spoken on 2026-08-07 ([`14A`](../../session/hand-2026-08-07/14A-the-sentence-and-the-door.md) §1, the owner's scoped lift of the sprint rule for the interaction chain) and the Story was built the same day ([`16A`](../../session/hand-2026-08-07/16A-the-first-conversation-built.md)). Criteria 1, 2 and 3 host-Green; criterion 5 done in writing. **Criterion 4's REFUSAL half closed on silicon 2026-08-08** ([`19A`](../../session/hand-2026-08-08/19A-the-ear-is-deaf-on-arrival.md)): `ti64dink --console` sent `PING`, `STATUS` and an unknown verb to a running board and each was answered with a distinct, named, sequence-matched refusal over the cable — `M1`'s round trip, and the first time this OS did anything because a human asked. Its **accept half is blocked on `LE-122`**, measured the same session: the board receives exactly four octets more than the host sends because the GEM descriptor's frame length includes the FCS, so `PING` and `STATUS` are refused as `oversize` before the verb table is consulted. One register bit and one boot. Assurance state `specified`.**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: [`session/hand-2026-08-07/10A-the-first-conversation-from-counted-frames-to-an-answered-command.md`](../../session/hand-2026-08-07/10A-the-first-conversation-from-counted-frames-to-an-answered-command.md) (proposal) and [`11A`](../../session/hand-2026-08-07/11A-sitting-a-executed.md) (this document)

## Description

[`STORY-P1-09-16`](STORY-P1-09-16.md) made the board reachable and forbade the reach
from meaning anything: *"no value from a frame selects a branch, an address, an offset
or a size anywhere in the image"* — and said, in its own words, that the moment a frame
means something *"the argument above stops being sufficient and has to be made again —
and it should be made again, not cited."* This Story is that re-making. It is the first
moment TinyOS does something because a human asked: `M2` of the interaction ladder,
the answered command.

**The shape, fixed here so the building session inherits decisions rather than
options:**

- **A fixed-width command envelope, `TOS64-CMD/1`,** carried on the already-admitted
  path (EtherType `0x88B5`, `TOS64-` prefix, hardware address filter, MAC-enforced
  size bound — all of `-16`'s containment, untouched). After the envelope tag: a
  four-byte magic, a verb id as one big-endian integer from a **closed allowlist**, a
  fixed-width argument field, nothing else. **No parser.** Classification is a total
  function over fixed offsets — `gem_receive::admit`'s discipline extended by exactly
  one field, and deliberately on the *admissible* side of the line
  [`STORY-P1-13-01`](STORY-P1-13-01.md)'s containment decision drew the same day from
  the other direction: fixed-offset total functions in C1, input-steered reads never.
- **Verb ids resolve through a deny-by-default table** — the
  [`FEAT-P2-01`](../features/FEAT-P2-01.md) policy-seam *shape* (every verb through
  one seam, resolution before execution, denial attributable), not a new authority
  model and not a call into the x86_64 shell. The first table holds two rows, both
  read-only, both answer-only:
  - **`PING`** — the answer names the sequence heard, proving round-trip (`M1` rides
    the same table: an echo is `PING` with nothing else in the table yet exercised).
  - **`STATUS`** — the answer replays the transcript's own boot verdict line, already
    public on the wire every cycle.
- **Every refusal is spoken, attributably, on the wire** — wrong magic, unknown verb,
  oversize, undersize and over-rate are distinct named refusals in the answer channel,
  not silent counter increments. A refused command that vanishes is indistinguishable
  from a dead board, which is the diagnosis failure `LE-80`'s family keeps producing.
- **The answer rate is bounded, fail-safe.** An unauthenticated broadcast-capable peer
  must not be able to make the board transmit at line rate: at most one answer per
  park beat leaves the board; excess admitted commands are counted and refused as
  over-rate on the next answer. Amplification is a hostile-load failure (`SEC-20`),
  and the bound is part of the design, not a tuning.

## The charter, read again — not cited

Each obligation below is quoted from its register text and answered for *this* Story's
shape. The building session re-reads these against the code it writes; a drifted
answer is a defect.

- **`BND-02`** — *"C0 exposes no reusable runtime authority."* Untouched: the command
  path begins at the GEM ring, not at boot handoff state.
- **`BND-03`** — *"C1 contains no complex hostile-format parser … Privileged
  hostile-parser entry points and their linked executable bytes equal zero."* The
  fixed-width classifier must stay on the admissible side of the same line
  `STORY-P1-13-01` argued: fixed offsets only, no input value ever used as an offset,
  length, or address; the verb id selects a **table row bounded by the table's own
  length check**, which is the one input-derived selection this Story introduces, and
  it is exactly as wide as the deny-by-default table and no wider. The suite's
  mutation arm asserts that widening the classifier past fixed offsets fails a test.
- **Matrix `C1 → C1` row** — *"Fixed-format kernel object and architecture operations
  only; fail closed without invoking a complex hostile parser."* Fixed width **is**
  the containment: the envelope is a fixed-format object by construction, and every
  malformed instance resolves to a named refusal, never a partial read.
- **`PD-02`** — *"Kernel-derived caller identity … never from caller-supplied
  identifiers."* The wire peer has **no** authenticated identity, and this Story must
  not pretend otherwise: no caller-supplied field is ever treated as an identity, and
  the verb table may therefore hold **only** verbs whose answers disclose what the
  board already broadcasts and whose execution changes nothing. `PING` and `STATUS`
  qualify; nothing else does until a session/authentication story (the
  deploy-protocol/WCI model) exists. This is why the table's first cut is two rows
  and why a third row is a charter re-read, not an addition.
- **`PD-14`** — *"No ambient namespace or class-derived authority … class and
  priority grant none."* The verb table **is** the policy: a verb absent from it does
  not exist, and no verb reaches a capability, a name, a file, a register write or a
  state change. Empty ambient authority is trivially satisfied because the verbs own
  nothing to leak — and must be *kept* satisfied verb by verb.
- **`RCG-*`** — untouched, asserted not inherited: no byte from the wire is ever
  code, maps nothing, launches nothing, patches nothing. The argument field is
  compared and echoed at fixed width; it is never dereferenced.
- **`PD-07` / fail-closed (`-16`'s discipline carried forward)** — a receive error
  still disables receive terminally; the command path adds no re-arm and no retry.

## Adversarial suite (specified now, written red by the building session)

[`TEST-P1-09-17-A`](../tests/TEST-P1-09-17-A.md) clause by clause: wrong magic,
unknown verb, undersize, oversize, flood/over-rate, argument-field hostility, and the
two positive arms — each a **host test against the classifier and table before any
board is powered**, each refusal distinct, and the two-directions parity discipline
(`LE-80`'s lesson) applied to the Ti64Dink half from day one.

## Depends on

- `STORY-P1-09-16` — the admission filter and its containment argument; criterion 4's
  board half (five `--send` arms) should close in the same sitting that first answers
  a `PING` (`10A` §4 Sitting B).
- **`S1` — the receive re-arm.** `TOS64-RX/1 STATE=STOPPED REASON=NOBUFFER` is the
  wire's own proof that a ring of one never-re-armed descriptor cannot hold a
  conversation. The re-arm discipline is `-16`'s owner's disposition to write
  (`08A` §7), and it must land before `M1`: an echo that works once is
  indistinguishable from a fluke.
- The owner's S4 sentence, which this Story waits on by design.

## Acceptance criteria (first cut — the building session may sharpen, not weaken)

1. The `TOS64-CMD/1` classifier is a total function over fixed offsets; every
   malformed envelope maps to a distinct named refusal; the mutation arm proves the
   fixed-offset discipline is load-bearing (a widened read fails a test).
2. Verb resolution is deny-by-default through one table; `PING` and `STATUS` are its
   only rows; each row's answer is assembled from data the board already broadcasts;
   a reviewer can check every row against the `PD-02` reading above.
3. Refusals are spoken on the wire with the refusal named; the answer rate is bounded
   at one per park beat with over-rate itself a spoken refusal; no path transmits in
   response to a frame outside the bounded answer slot.
4. Board: `ti64dink` sends `PING`, the board's answer names the sequence heard
   (`M1`+`M2` in one sitting, `10A` §4); `STATUS` replays the boot verdict line; each
   refusal arm sent from the host produces its named refusal in the capture.
5. The `-16` absence argument is retired **in writing**: its Story text gains a dated
   note that the expiring argument expired here, superseded by this Story's charter
   reading — so no future reader inherits an absence that no longer holds.

## Progress, 2026-08-07

| Criterion | State |
|---|---|
| 1 — total classifier over fixed offsets, distinct refusals, mutation arm | **Green (host).** The envelope is 46 octets so that `14 + payload` is exactly the Ethernet minimum and no NIC's padding can reach the classifier; the field ranges are asserted to tile the payload with no gap and no overlap, which is the mutation arm — widening or adding a field moves a later offset and fails. Every 16-bit verb id is exercised; the argument field is filled across its range and may not move the verdict. |
| 2 — deny-by-default, two answer-only rows | **Green (host).** `VERB_TABLE` holds `PING` and `STATUS`; `resolve` walks the table's own length; zero is not a verb, so an all-zero payload — the likeliest accident on a wire — is `UnknownVerb`. The module carries `#![forbid(unsafe_code)]` and no signature on the path takes a device, so a row **cannot** reach a register: clause 2's "a row that gains authority is a red test" is enforced by the compiler rather than by review. |
| 3 — refusals spoken, rate beat-bounded | **Green (host).** Five distinct wire names; one line per beat; a flood of 10,000 commands is asserted to emit no more lines than there were beats. |
| 4 — the board answers | **Blocked on hardware only.** The host half exists: `ti64dink --console` builds the frame, parses the answer, and runs a whole scripted session — answer, refusal, timeout — with no board and no Npcap. |
| 5 — the `-16` absence argument retired in writing | **Done.** [`STORY-P1-09-16`](STORY-P1-09-16.md) gains a dated section recording that its expiring argument expired here. |

**One defect the suite found before any board did.** The first draft counted
over-rate drops in a saturating counter and spoke them when the answer slot next
came free. Under a *sustained* flood the pending slot is refilled every beat, so
the count is never spoken and the drops vanish — the exact "a refused command
that vanishes is indistinguishable from a dead board" failure this Story
forbids, reachable only under load and invisible to any single-command test. The
fix gives an owed confession precedence over a new command: while a drop is
unspoken the channel accepts nothing new, so the over-rate refusal always
reaches its slot. Recorded because the failure was found by clause 4's flood arm
and would not have been found by a bench.

## What landing this Story must also do (recorded so it is not discovered late)

- Amend [`FEAT-P1-09`](../features/FEAT-P1-09.md)'s contract row: the authority
  posture must name the deny-by-default verb table and the bounded answer rate before
  implementation starts — the current posture text describes receive-and-count only.
- Re-read `LE-67` (updated, not closed, by `-16`) against the answer path: transmit
  from the command handler shares the beacon's single-buffer discipline and must not
  alias `RECEIVE_MEMORY`.
- State the Ti64Dink console half (`M4` is Sitting C's headline; this Story only
  needs `--send`-style verbs and capture assertions).
