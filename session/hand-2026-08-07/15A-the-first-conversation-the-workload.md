# 15A — The workload: one session from silence to conversation

The next session's work, written so it executes instead of derives. Cover
note: [`14A`](14A-the-sentence-and-the-door.md), which records the scoped
sprint-rule lift this workload runs under. Everything here is the interaction
chain and nothing else: **the ear** (`-16`'s re-arm), **the tongue** (the
admitted verb), **the face** (the exchange on glass), **the prompt** (the
Ti64Dink console), and the bench half that makes a human's first exchange
with TinyOS a matter of typing.

**The one sentence, if only one survives:** *At this session's close a person
types `PING` at a prompt and TinyOS — on silicon, over the cable, through a
deny-by-default table — answers with the sequence it heard; every part below
exists to make that sentence true without weakening one committed refusal.*

## Part I — The ear: `-16`'s re-arm (laptop first, board later)

`TOS64-RX/1 STATE=STOPPED REASON=NOBUFFER ACCEPTED=0 REFUSED=0` (`07F` §7c)
is the wire's proof that a ring of one never-re-armed descriptor is a
doorbell, not an ear. No conversation survives it, so this lands **before**
`M1` — an echo that works once is indistinguishable from a fluke.

1. **Diagnose before designing.** If `12A` boot 1 has run, its capture says
   whether `NOBUFFER` is a boot-time condition (status read before the ring
   was armed) or first-frame exhaustion. If it has not run, write the re-arm
   against both readings and let the first board listen say which was live —
   the fix below is the same either way; only the Report's wording differs.
2. **The discipline, red-first over the scripted seam** (`gem_receive`'s
   existing double): after a frame is classified and counted, the descriptor
   is handed back — address preserved, `WRAP` kept, ownership returned to
   the MAC — and stale status is cleared, **at most once per park beat**.
   Bounded: one frame classified, one descriptor re-armed, per beat; a
   second frame arriving inside a beat waits in the MAC until the next
   hand-back, which is the same bounded-poll discipline every channel on
   this board already keeps.
3. **What does not change, asserted not assumed:** the enable order stays
   pinned with `RE` strictly last; overrun and buffer-not-available stay
   *terminal* — the re-arm is for the healthy path only, and a test proves
   no error arm re-arms anything, on that pass or any later one. The
   containment argument's four parts are untouched: same region, same
   filter, same size bound, same total classifier.
4. **Story mechanics:** the re-arm is a criterion amendment to
   [`STORY-P1-09-16`](../../goals/stories/STORY-P1-09-16.md) (its §"Named
   debt" already owns the gap), not a new Story — the Story's subject was
   always "the board can be reached, fail-closed"; an ear that stays deaf
   after one frame fails that subject's own sentence.

## Part II — The tongue: `STORY-P1-09-17` built exactly as written

The Story and suite are filed; the building session's job is to make the
specification true without widening it.

1. **`TEST-P1-09-17-A`'s clauses become code, red first, verbatim** — wrong
   magic, unknown verb, undersize, oversize, flood/over-rate, the mutation
   arm proving fixed-offset discipline is load-bearing, the two positive
   arms. The suite's normative form is the Test document; a divergence
   between it and the tests-as-written is a defect in the session, not a
   liberty.
2. **The classifier** extends `gem_receive::admit`'s discipline by exactly
   one field: fixed offsets, total, the verb id's table lookup bounded by
   the table's own length — the one input-derived selection this chain
   introduces, exactly as wide as the deny-by-default table.
3. **The answers ride the existing text channel, beat-bounded.** One answer
   per park beat leaves the board; `PING`'s answer names the sequence heard,
   `STATUS`'s replays the boot verdict line; excess admitted commands are
   counted and refused as over-rate on the next slot. No new transmit
   machinery: the answer is one more line in the transcript cycle's
   priority, and a test proves the beacon cadence a capture window is sized
   against does not move (`03B` §3a's constraint, kept).
4. **The Feature contract amendment lands before the code** (the Story's own
   "What landing this Story must also do"): `FEAT-P1-09`'s authority posture
   gains the verb table and the bounded answer rate; `PD-02` joins its
   protection-domain row; `LE-67` is re-read against the answer path (the
   answer buffer must not alias `RECEIVE_MEMORY`, stated and tested).
5. **The absence argument is retired in writing** (criterion 5): `-16`'s
   Story gains the dated note that its expiring argument expired here,
   superseded by `-17`'s charter reading — no future reader inherits an
   absence that no longer holds.

## Part III — The face: the exchange on glass (`M3`)

One canvas row: the command channel's state — last verb answered (by name),
answers sent, refusals spoken — painted the way `TOS64-RX/1` already is.
Small by design: the canvas is UX, the wire is evidence, and the row exists
so an operator at the bench sees the OS respond without opening a capture.
Rides `07F` §7b's lit-canvas procedure until `FEAT-P1-13`'s consumption
increment lands; a dark canvas costs nothing (`Canvas::is_dark` already
speaks for it).

## Part IV — The prompt: Ti64Dink console mode (`M4`'s host half)

`ti64dink --console`: read a verb from stdin, build the `TOS64-CMD/1` frame,
transmit, capture until the answer or a stated timeout, print the answer —
loop. The first interactive terminal this OS has ever had.

- **TDD in `ti64dink.tests`, seams first:** the frame builder and the
  answer/refusal parser are pure and exact-byte-tested (the `Send.Frame`
  pattern); the console loop is driven over injected reader/writer seams so
  a scripted session — prompt in, answer out, refusal named, timeout named —
  runs green on the bench with no board and no Npcap.
- **The `LE-80` discipline from day one:** the verb list and the refusal
  vocabulary are single tables shared with (or parity-tested against) the
  Rust side — the rung-table lesson applied before the drift, not after.
- **Honest output rules carried over:** a timeout prints as a timeout, never
  as a refusal; a refusal prints its wire-given name; nothing is smoothed.
- **Not in scope:** command history, scripting, multi-board — `M5`'s
  gravity, refused here by name.

## Part V — The bench half: the first conversation

Boots, in order, each double-spent per `12A`'s standing rule (if Sitting B's
qualification boots have not yet run, its manifest executes first and these
ride the same afternoon):

1. **The sustained listen** — the re-arm image, then `--send ping` five
   times spaced across beats: `accepted` climbs 1, 2, 3, 4, 5. The ear is
   proven — counted, then counted *again*, which is the whole difference
   between an ear and a fluke. `S1` closes on this capture.
2. **The five arms** — `-16` criterion 4's predicted verdicts, closing that
   Story (its host tests already pin every expectation).
3. **`M1`+`M2`** — `--send`-style `PING`: the answer frame names the
   sequence heard. `STATUS`: the boot verdict line comes back. An unknown
   verb: the refusal comes back, *named*, and the flood arm shows over-rate
   refusals while the beacon cadence holds.
4. **`M4`** — the operator opens `ti64dink --console` and **types**. `PING`.
   Answer. `STATUS`. Answer. Nonsense. A spoken refusal. That transcript —
   captured, parsed to its own verdict like every capture since `07F` — is
   the session's deliverable and the project's first conversation.
5. **Filing, same day:** `-16` Verified; `-17` criteria closed with the
   capture cited; the M-ladder's state recorded in the handover; `LE-117`'s
   tripwire in the preamble; any `NOBUFFER` finding dispositioned in its
   Report. The register should be smaller on the far side — this workload
   opens no new surface beyond the three items the lift names.

## What this workload refuses, so the door opens without the frame bending

No third verb (a charter re-read, per `-17`'s own text). No session, no
authentication, no authority-bearing verb — the wire peer still has no
identity and the table's two rows are the honest maximum. No TCP/IP; counted
raw frames with fixed-width envelopes remain the transport. No remote
desktop; `M5` stays named-and-shut. No DTB chunking (the shape-2 consumption
increment queues behind the conversation, not inside it). And no timing
claim from any of it: an answered `PING` is a *functional* fact; latency
numbers wait for their own instrumented arm and their own honest tier.

## The arithmetic, restated once

`12A` closed with the register's lock and the product's first conversation
opening on the same day. Sitting B's script made the lock's half cheap;
`13A` banked its instruments. This workload is the other half: after it, the
question "what is the OS doing?" costs a keystroke instead of a power cycle
— and every Story, every deploy path, every Ti64Dink feature after it is
built against a machine you can *ask*.
