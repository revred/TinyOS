# 10A — The First Conversation: from counted frames to an answered command, and what actually stands between us and using this OS

Cover note for the next sessions, written at the owner's ask: *what takes us
beyond the current blockers and showstoppers to actually interacting with the
TinyOS build.* Same session as [`08A`](08A-the-swot-answered-and-the-tree-is-one-again.md)/[`09A`](09A-the-closing-pass-the-evidence-already-paid-for.md).

**The one sentence, if only one survives:** *Every instrument, channel and
charter argument needed for a human to send TinyOS a byte and watch it answer
now exists in the tree — what stands between us and the first conversation is
one sustained-receive fix, one Story that lets a received frame mean exactly
one thing, and the owner's sentence lifting the sprint rule for the
interaction chain the way it was lifted for `FEAT-P1-13`.*

## 1. Name the destination precisely, because "interacting" hides a ladder

The owner's first priority has been bidirectional interaction since 2026-08-03
(peer-to-peer Ethernet remote desktop, host app **Ti64Dink**). "Interacting
with the TinyOS build" decomposes into five milestones, each one bench-provable
on its own:

- **M1 — the echo.** Ti64Dink sends a frame; the board's *reply* names what it
  heard. The wire proves round-trip, nothing is executed.
- **M2 — the answered command.** A received verb resolves through a
  deny-by-default table; an allowed one answers (`STATUS` returning the
  transcript's own lines), a refused one refuses *on the wire, attributably*.
  This is the first moment TinyOS **does something because a human asked**.
- **M3 — the visible answer.** The same exchange paints on the canvas — the
  operator sees the OS respond on glass, not only in a capture.
- **M4 — the session.** Ti64Dink gains an interactive console mode: prompt in,
  reply out, continuously. This is the owner's "launch the OS" moment on real
  silicon — the Ti64 console with a board behind it instead of a QEMU guest.
- **M5 — the remote desktop** (`FEAT-P2-10`'s destination). Out of scope for
  this note beyond saying M1–M4 are its exact prerequisites.

## 2. What already exists — the inventory that makes this cheap

- **Outbound is done and self-verdicting**: TOS64 envelopes, beacons,
  `TOS64-RESULT/1`, `parse-meas` exit 0/1 — every reply channel M1–M4 needs.
- **Inbound exists and is contained**: `STORY-P1-09-16`'s ring, hardware
  address filter, MAC-enforced size bound, enable-last discipline, and a
  **total classifier that interprets nothing** — criteria 1–3 host-Green;
  criterion 4 is *one power cycle and five `ti64dink --send` commands*, every
  expected verdict already machine-checked against `gem_receive::admit`.
- **The authority model already exists at Tier 0**: `EPIC-P2`'s shell resolves
  every verb through one deny-by-default policy seam with session identity and
  empty ambient authority (`FEAT-P2-01`, board-agnostic by design). M2 does
  not invent an authorisation story — it *reuses* the one the charter already
  approved, with the verb table cut to almost nothing.
- **The display path is decided**: `FEAT-P1-13` (owner-ordered, sprint rule
  lifted) justifies the framebuffer from the boot's own device tree;
  `STORY-P1-13-01`'s containment decision is queued as a fresh session's
  first-class work. M3 needs nothing more than what `-09`'s canvas already
  paints.
- **The bench loop runs today**: owner power cycles work (`07F`), captures
  carry verdicts, and the qualification record is one Q3 campaign from done.

## 3. The actual showstoppers, each with its exit — there are only four

**S1 — Receive stops after one frame, and we have the wire line proving it.**
`TOS64-RX/1 STATE=STOPPED REASON=NOBUFFER ACCEPTED=0 REFUSED=0` (first
observation, lit-canvas boot, `07F` §7c). A ring of one wrapped descriptor
that is never re-armed is a doorbell, not an ear; **no conversation survives
it**. The exit is `-16`'s owner giving the stopped state a re-arm discipline —
bounded, fail-safe, refusal-spoken — and it must land *before* M1, because an
echo that works once is indistinguishable from a fluke. This is the single
highest-leverage piece of board code in the project right now.

**S2 — No received byte is allowed to mean anything, by explicit design.**
`-16`'s containment argument is *satisfied by absence*: no value from a frame
selects a branch, address, offset or size anywhere in the image — and the
Story itself says the argument **expires the moment a frame means something**,
which is why "one command answered end to end is a separate Story with the
charter read again rather than cited." That Story is the heart of this note:

> **STORY (proposed): the admitted verb.** A fixed-width command envelope
> (`TOS64-CMD/1`) over the existing filter: magic, verb id as an integer from
> a **closed allowlist**, fixed-width argument field, everything else refused
> with the refusal named on the wire. **No parser** — a total function over
> fixed offsets, exactly `gem_receive::admit`'s discipline extended one field.
> Verb ids resolve through a deny-by-default table (the `FEAT-P2-01` seam's
> shape, not a new one); the first table holds two rows: `PING` (answer names
> the sequence heard) and `STATUS` (answer replays the boot verdict line).
> Charter obligations read again, not cited: `BND-02`/`BND-03` (hostile
> input, no complex format — fixed width **is** the containment), `PD-02`
> (no ambient authority; the verb table *is* the policy), `RCG-*` untouched
> (no byte from the wire is ever code, maps nothing, launches nothing),
> matrix C1 row argued in the Story against its own text. Adversarial suite
> red first: wrong magic, unknown verb, oversize, undersize, flood — each a
> distinct spoken refusal, each a host test before the board sees one frame.

**S3 — The operator's terminal is the least-tested tool we own.** Ti64Dink
gates nothing (`LE-114`), has no arrival timestamps (`LE-115`), and its defect
history (`LE-80`) is the project's most expensive. Before M4 makes it *the*
interface, land the `dotnet test` job (alone, as `LE-114` says), put the
console/`--send` paths under test, and give frames timestamps — which also
unlocks the free beat-cadence measurement.

**S4 — The sprint rule, which is the owner's sentence and nobody else's.**
The 2026-07-30 rule ("no new design surface for two sprints") was written to
force silicon evidence before architecture. The evidence arrived: first light,
first envelope, first listen, Q1+Q2+Q4, a green main. S2's Story *is* new
design surface. The precedent is one sentence, already used once:
**"Sprint rule lifted for the interaction chain (`-16`'s re-arm, the admitted
verb, Ti64Dink console) and nothing else"** — the same scoped lift
`FEAT-P1-13` received on 2026-08-07. Until the owner says it, S2 stays a
proposal; nothing in this note pre-empts that call.

## 4. The session plan that gets there — two sittings, one of them ownerless

**Sitting A (laptop, no board, no owner):** the `STORY-P1-13-01` containment
decision (already sanctioned); the admitted-verb Story *written* with its
adversarial suite red (writing it violates no rule — building it awaits S4);
`LE-114`'s dotnet job; `LE-115`'s timestamps; ti64dink console-mode host
tests. Everything here is TDD-able against doubles today.

**Sitting B (bench, owner's hand or the relay):** boots double-spent, `07F`'s
lesson — every power cycle carries qualification *and* interaction payloads:
1. `-16` criterion 4: five `--send` arms, five predicted verdicts (closes the
   Story's board half).
2. The re-arm fix's first sustained listen (S1's exit, watched on the wire).
3. **The Q3 campaign** (`08A` §5: SMC positive control, then the stated-
   duration run) — rides the same boots, unlocks every `G04` and the
   assurance-verified tile's ceiling.
4. If S4 is spoken by then: `PING` answered — **M1 and M2 in the same
   sitting**, because the verb table's second row costs one more frame.
5. `LE-117`'s tripwire in the preamble (bootloader hash vs the Report's pin).

M3 follows wherever the canvas is lit (bench procedure in `07F` §7b until
`FEAT-P1-13` lands); M4 is Sitting C's headline once M2's exchange exists.

## 5. What this note deliberately does not open

No TCP/IP stack — counted raw frames with fixed-width envelopes are the
transport until a Story argues otherwise (`STORY-P1-09-03`'s open debt is not
this note's to spend). No DT generalisation past `FEAT-P1-13`'s one fact. No
serial revival (`LE-47` stands; the wire won). No remote-desktop design — M5
is named as the destination precisely so nobody starts it sideways. And no
softening of the expiring absence-argument: until the admitted-verb Story's
charter reading is written and reviewed, **the board keeps refusing meaning**,
which is the correct state for it to wait in.

## 6. Why this is the highest-impact path, said against the register

The assurance spine's own arithmetic says the locked tiles open on Q3 (one
sitting, already specified). The *product's* arithmetic says the owner's first
priority opens on S1+S2 (one re-arm, one two-verb Story). Sitting B carries
both at once — the same power cycles, the same captures, the same afternoon.
There is no ordering conflict and no idle work in either chain: the next bench
session can move the OS from "measured" to "answers when spoken to" while
taking qualified platforms from 0 to 1. That is the whole gap between the
register we have and the OS the owner asked for.
