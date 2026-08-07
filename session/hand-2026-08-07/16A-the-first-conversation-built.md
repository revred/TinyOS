# 16A — The first conversation, built: the ear, the tongue, the face and the prompt

The session that executed [`15A`](15A-the-first-conversation-the-workload.md)'s
laptop half, under the scoped sprint-rule lift [`14A`](14A-the-sentence-and-the-door.md)
records. Solo session; the tree was pulled clean at `7de0c15` and no concurrent
commits arrived mid-session.

**The one sentence, if only one survives:** *Every part of `15A` that does not
require a powered board now exists, is Red-first tested and builds for the
board — the descriptor is handed back on the healthy path and on no error arm,
a `TOS64-CMD/1` envelope resolves through a two-row deny-by-default table into
a beat-bounded answer, the canvas has a command row, and `ti64dink --console`
runs a whole scripted conversation with nothing plugged in — so the only thing
left between this project and a human typing `PING` at it is a power cycle.*

## 1. What exists now, by Part of the workload

**Part I — the ear (`15A` §I).** The hand-back is a pure function,
`gem_receive::beat_plan`, not a branch in the aarch64 glue. That placement is
the point: the glue is the one part of this path no host test can reach, and
"no error arm re-arms" is precisely the claim that must not live somewhere
unreachable. A host test exhausts the four status outcomes against all five
descriptor states — twenty combinations, every one classified, both error arms
handing nothing back on every one of them. The hand-back preserves the buffer
address, keeps `WRAP` and returns ownership; `poll_receive` is now a thin
translation of the plan and decides nothing. Filed as `STORY-P1-09-16`
criterion 5 (an amendment, per `15A` §I item 4, because an ear that stays deaf
after one frame fails that Story's own subject) and `TEST-P1-09-16-A` clause 10.

A second, smaller correction rode it: a descriptor refusal was being relabelled
`TooShort` by the glue, which was a small lie about which refusal happened.

**Part II — the tongue (`15A` §II).** `os/src/hal-arm64/src/tos64_cmd.rs`, written
against `TEST-P1-09-17-A` clause by clause, Red confirmed before a line of it
existed. The envelope is 46 payload octets **so that `14 + 46` is exactly the
Ethernet minimum** — a shorter envelope would arrive padded by the sending NIC,
below any software that could be told not to, and the padding would be
indistinguishable at the board from a wrong-width field. That is what lets
"exactly this many octets" be a refusal rather than a hope, and it is why
`Oversize` needed one spare octet in the copy-out buffer to stay reachable at
all.

Two rows, `PING` and `STATUS`, resolved by walking the table's own length; every
one of the 65,536 possible ids is exercised by a test. Five distinct spoken
refusals. One line per park beat. The Feature contract amendment landed
**before** the code, as the Story required: `PD-02` joins `FEAT-P1-09`'s
protection-domain row — and, because the checker holds a Feature's Tests to its
row, all seventeen `TEST-P1-09-*` documents with it.

`#![forbid(unsafe_code)]` on the module is doing real work rather than being
tidy: every register on this board is reached through an `unsafe` volatile
access, and no signature on this path takes a device. `TEST-P1-09-17-A` clause
2's "a row that gains authority is a red test" is therefore enforced by the
compiler instead of by review.

**Part III — the face (`15A` §III).** `TOS64-CMD/1 last=PING answered=1
refused=0`, painted every beat at `canvas::CMD_Y`, beneath the transcript block
rather than beside the RX row it belongs with — the block below `RX_Y` is sized
for full occupancy and squeezing a row in above it would move rows an operator
and three filed captures already know by position. A compile-time claim pins
that it clears the transcript at `MAX_LINES`.

**Part IV — the prompt (`15A` §IV).** `ti64dink --console`. One pcap handle held
open for both directions, because between closing a send handle and opening a
listen handle the board's answer has already gone past. The frame builder and the
answer parser are pure and asserted byte for byte; the loop runs over injected
reader, writer and link seams, so a whole session — answer, refusal, timeout —
is twelve host tests on a bench with nothing plugged in. The three honest-output
rules are each a test: **a timeout prints as a timeout and never as a refusal**,
a refusal prints the name the wire gave it, and a **stale** answer for an older
sequence is skipped rather than printed under the command just typed.

`LE-80`'s discipline from day one rather than after the drift:
`ConsoleParityTests` reads `tos64_cmd.rs` itself — comments stripped first, so
prose cannot satisfy the scan — and holds the C# verb and refusal tables against
it in both directions. Verified to fail: changing `Verb::Status`'s id in the Rust
source turns it red, and the file was restored.

**Also landed, from `15A` §II item 4's list.** `LE-67`'s non-aliasing half stops
being an assumption. `gem_receive::check_grants` refuses to arm when the transmit
staging region and the receive region overlap, and the canvas says
`reason=alias`. The answer transmits from the beacon's staging region through the
existing frame builder — no second grant — and the arithmetic that says so is now
a test at the edges rather than a sentence in a doc comment.

## 2. The finding, and why it is a register row instead of a fix

**`LE-118`: a receive status carrying both a frame and a terminal error throws
the frame away and kills the ear — and on any segment with broadcast traffic
that is the expected first outcome, not an edge case.**

`read_status` tests `OVR` then `BNA` before `REC` and returns `Err` on either.
A status word reading `REC|BNA` — one frame successfully in the ring, plus a
second the MAC had nowhere to put — therefore resolves to a terminal stop,
discarding a whole classifiable frame and leaving the board deaf with
`ACCEPTED=0 REFUSED=0`. That is byte for byte the line `07F` §7c recorded.

The arithmetic is what makes it a finding rather than a curiosity: the ring is
one descriptor polled once per beat at 1 Hz, and the hardware filter admits
broadcast deliberately, so that `-16` criterion 4's broadcast `ping` arm is
accepted. Ordinary broadcast noise from a Windows host on a link-local interface
puts two frames inside one second routinely. **Two frames in one beat is the
median on a shared segment, not a flood — and Part I's hand-back does not help,
because the ear dies before the first hand-back can ever run.**

It is not fixed here on purpose. `15A` §I item 3 commits both error arms to
staying terminal and `14A` §3 says the refusals are the product, so narrowing or
reordering the taxonomy is the owner's call. The row names three candidate
dispositions rather than leaving the choice to drift — count-then-stop, narrow
what reaches the ring with `NCFGR.NBC` and a unicast-only command path, or accept
it and bound the bench to a quiet cable — and names the evidence that decides
between them: **boot 1 of the `12A` manifest**. A fresh boot reading
`STOPPED REASON=NOBUFFER` with no host frame sent at all is a boot-time condition
and points elsewhere; one that reads it only after traffic confirms this
mechanism. `15A` §I item 1 wrote the re-arm against both readings for exactly
this reason, and only the Report's wording now differs.

**A second defect the suite found before any board could.** The first draft of
the answer channel counted over-rate drops and spoke them when the slot next came
free. Under a *sustained* flood the slot is refilled every beat, so the count is
never spoken and the drops vanish — the exact "a refused command that vanishes is
indistinguishable from a dead board" failure the Story forbids, reachable only
under load. The fix gives an owed confession precedence over a new command. It
was found by clause 4's flood arm and would not have been found by a bench.

## 3. What is owed the bench, unchanged from `15A` §V

Nothing in this session touched a boot. The manifest is `12A`'s, absorbed by
`15A` §V, and every image it names builds and lints for the board
(`check-boot-images`, five variants). In order:

1. **The sustained listen** — `--send ping` five times spaced across beats:
   `accepted` climbs 1, 2, 3, 4, 5. Counted, then counted *again*, which is the
   whole difference between an ear and a fluke. Read `LE-118` before reading a
   `NOBUFFER` as a defect in the re-arm.
2. **The five arms** — `-16` criterion 4's predicted verdicts, closing that Story.
3. **`M1`+`M2`** — `PING`: the answer names the sequence heard. `STATUS`: the boot
   verdict line comes back (on a `fixture-measure` image; the featureless build has
   no transcript to replay and answers `status=none`, which is an honest absence
   rather than a fabricated verdict). An unknown verb: `refused=unknown-verb`,
   named. The beacon cadence does not move — the beacon still transmits once per
   beat and the answer is one additional line, never a substitution.
4. **`M4`** — `ti64dink --console`, and a person types. That transcript is the
   deliverable.
5. **Filing, same day** — `-16` Verified, `-17` criteria closed with the capture
   cited, `LE-117`'s tripwire in the preamble, `LE-118` dispositioned in its Report.

## 4. Read next

[`15A`](15A-the-first-conversation-the-workload.md) for the bench half verbatim;
[`STORY-P1-09-17`](../../goals/stories/STORY-P1-09-17.md) for what was built and
what its own suite found; `LE-118` for the one thing most likely to end a
conversation before it starts.
