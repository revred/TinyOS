# 22A — The usable OS is on the board

**Aim, stated first, per [`21A`](21A-the-destination-and-the-three-steps-to-it.md) §5
item 4:** *this session advanced §3 steps 1, 2 and 3 — `shell` compiles for the board,
the board image carries it, `TOS64-CMD/1` reaches `TINYCMD`'s verb core, and a human
typed at it over the cable.* It did not open `loose-ends.tsv` to choose its work; it
opened `21A`.

**The one sentence, if only one survives:** *A person typed `FIND /N "cable"
README.TXT` on a laptop and a Raspberry Pi 5 answered with `TINYCMD`'s own numbered
match out of its own RAM volume — `21A`'s destination reached, on silicon, with
`0 exchange(s) went unanswered`, a `DEL` refused and audited against the `WIRE`
session, and a `DIR` byte-identical before and after two denied mutations.*

**Everything below §7 was written before the board was touched and is left as it
stood; §8 onward is the silicon run.** The three steps were delivered in one session
because step 1 turned out to be one line of a manifest (`LE-123`) and step 3 turned
out to be one power cycle.

## 1. Step 1 — measured, then gated, then fixed, in that order

`21A` §4 said the obstacle was the manifest and not the code. It was, and the
measurement reproduced exactly: `cargo check -p shell --lib` against `aarch64-tinyos`
gave **29 errors, every one inside `hal-x86_64`** — `invalid register dx`,
`invalid register al`, `att_syntax is only supported on x86`, `cannot find x86_64 in
arch`. **Not one was in `shell`'s own source.**

The gate went in **first**, as `21A` §4's closing paragraph required, because the
dependency is exactly the kind someone re-adds while fixing an unrelated fixture.
`shell --lib` is now a board-target entry in `check-boot-images` alongside `hal-arm64`,
`kernel` and `pi5-image` — clippy for the target both compiles and lints, so one row is
the whole gate. The list itself is pinned by host tests, per `LE-72`'s standing lesson
that coverage is a property of the list.

Then the fix, which is two lines:

```toml
[target.'cfg(target_arch = "x86_64")'.dependencies]
hal-x86_64 = { path = "../hal-x86_64" }
```

**Target-gated rather than feature-gated**, because `hal-x86_64` *is* x86 inline
assembly — the architecture is the condition, and a feature would only let someone
select it for AArch64 and get the same wall of errors with an extra flag in front.

**`kernel` was left unconditional, and that was measured too rather than assumed.**
`21A` §4 named two dependencies; only one of them ever blocked. `kernel`'s library is
arch-neutral, is already built and linted for the board, and is what `shell`'s
`#[cfg(test)]` spoor include needs on whatever host runs `cargo test` — gating it would
buy nothing on the board and would break the host suite on an AArch64 development
machine. Recorded in the manifest beside the change, so the asymmetry reads as a
decision.

`LE-123` raised and closed in this session.

## 2. Step 2 — the third row, and the argument that now means something

`-17`'s table said, in its own text, that *"a third row is a charter re-read, not an
addition."* This is the re-read. **It did not weaken the sentence it inherited; it
satisfied it a second way.**

The sentence is `PD-02`'s consequence: a peer with no kernel-derived identity may reach
only verbs whose answers disclose what the board already broadcasts and whose execution
changes nothing. `-17` satisfied it by having its rows execute nothing at all. `-18`
satisfies it by construction:

- **Execution changes nothing, in the strongest available sense.** The runner builds
  its `World` from a `const` seed on **every** command and drops it before returning.
  No `static`, no cell, no carried handle anywhere on the path — so no cwd,
  environment variable, file, label or counter survives one wire command into the next.
  The board after any admitted sequence is bit-identical to the board before it. That
  is a property of the shape, not a discipline, which is why the test can assert it
  directly: run `MD`, `COPY`, `SET`, `DEL`, then `DIR`, and the listing is byte-equal to
  the one before.
- **Nothing new is disclosed.** The grant set is the read-only subset of the verb core
  over a volume the image itself seeded, so a peer reads back only bytes that shipped
  in a published artifact.
- **No authority is reachable.** `tos64_cmd` still executes nothing — it classifies,
  reports the pending line, and renders what the caller hands back. `shell` carries
  `#![forbid(unsafe_code)]` exactly as it does, so *"a wire verb cannot reach a
  register"* is still compiler-enforced, now across three crates instead of one.

**What is withheld is written down as a decision, not left as an omission.** Every
mutating verb (`CD`, `COPY`, `MOVE`, `DEL`, `MD`, `RD`, `SET`, `PATH`) and `CLS` —
`CLS` because it emits a real terminal escape, and trusted output that repaints an
operator's screen is authority over a human. And every verb that reads live kernel
state (`MEM`, `TASKMGR`, `SPOOR`) — those are read-only and stateless and satisfy the
first sentence perfectly; they fail the second, and disclosing a task table to a peer
with no identity is a decision worth taking on purpose. Both families wait on the same
thing: the session/authentication story.

### The sentence that had to be retired, retired in writing

`-17` asserted *"no byte of the argument field steers anything"*, and that is now
false. The replacement is narrower and true — *the classification of every row is not a
function of the argument* — and it was made **in the test that used to hold the old
claim**, with the reason recorded there. Retiring a claim silently is the drift this
project keeps catching; the old test's name still points at the new one.

## 3. What it looks like, rendered from the board's own composition

Not a mock-up. These lines came out of the shipped classifier, the shipped grant set
and the real verb core, driven by real `TOS64-CMD/1` envelopes:

```text
TOS64-ANS/1 verb=SHELL seq=1 ok=1 out=\nTinyOS Version 0.2.0 (Tier 0, x86_64)\n\n
TOS64-ANS/1 verb=SHELL seq=2 ok=1 out=\n Volume in drive A is TINYOS\n Volume Serial Number is 5049-3501\n
TOS64-ANS/1 verb=SHELL seq=3 ok=1 out=\n Volume in drive A is TINYOS\n Volume Serial Number is 5049-3501\n\n Directory of A:\\\n\nREADME.TXT           112 07-30-26  12:00p\nVERBS.TXT             49 07-30-26  12:00p\n       2 File(s)      1 more=16
TOS64-ANS/1 verb=SHELL seq=5 ok=1 out=Access denied: verb Delete is not granted to session WIRE [audited]
TOS64-ANS/1 verb=SHELL seq=6 ok=1 out=Bad command or file name\n
```

Four things in there are worth naming. `seq=3` is a **`DIR` listing on one line of
wire** — the escape makes a multi-line transcript survive a line-oriented channel
losslessly, and `more=16` says exactly how many octets did not fit rather than leaving a
short listing to look complete. `seq=5` is the deny-by-default seam **audited into the
transcript the peer receives**, naming the `WIRE` session. `seq=6` is `TINYCMD`'s own
4.0 answer to an unknown command. And `seq=1` is a defect — see §5.

## 4. The bounds, and where each one comes from

- **One frame in, one line out, one line per beat.** The run happens *inside* the
  bounded answer slot, so an admitted command costs exactly the beat its answer was
  always going to cost. Widening the table did not widen the work an unauthenticated
  peer can make the board do per beat (`SEC-20`). A flood of `SHELL` commands emits no
  more lines than there were beats — the same arithmetic `-17` asserted for `PING`.
- **`ANSWER_CAPACITY` 128 → 256**, the whole concession the new row extracted from the
  wire format. It is the largest line the existing text frame can carry
  (`TEXT_FRAME_CAPACITY` is `14 + 256`), so the const assertion that used to be slack
  is now tight. The *rate* did not move.
- **` more=` is reserved before the output is written**, not appended after it — an
  accounting field a long output can push off the end is absent exactly when it
  matters.
- **A labelled prefix is a measurement; an unlabelled one is a forgery.** `status`
  still drops **whole** and says `none`, because it replays a verdict and a partial
  verdict is a plausible lie with no marker. Shell output is a stream, so a counted
  prefix is a true statement about the beginning of one. The divergence is asserted as
  a decision, in a test, so it does not read as an inconsistency later.
- **The stack.** The session is built on the board's 64 KiB stack, so its footprint is
  held against a declared budget of a quarter of it by a `const` assertion — a build
  failure rather than a guard-page fault found on a bench.
- **The image.** Flattened `kernel8.img` **323,400 → 525,624 octets**: 6.3% of the 8 MB
  base-image ceiling (`G-DX-8`). Measured before and after, not estimated.
- **30 octets of command line.** The real limitation of the fixed-width envelope, and
  the console names it before sending rather than letting the operator diagnose it from
  a refusal. Widening it would mean re-making `-17`'s no-padding argument, which is a
  price worth paying deliberately or not at all.

## 5. The defect this work found, which no test was looking for

```text
SHELL VER  →  TinyOS Version 0.2.0 (Tier 0, x86_64)
```

On an AArch64 board. It is a literal in `shell/src/verbs.rs`; it was true of every
context the shell had ever run in; and **it is pinned byte-exact by
`TEST-P2-07-01-A`'s golden transcript** — so the gate that exists to catch shell output
drift is currently the thing requiring the false string.

Raised as **`LE-124`** and deliberately **not** fixed here. The fix is small and its
blast radius is not: the architecture and tier have to become session-supplied facts
rather than literals, which regenerates a golden transcript and touches the one gate in
this tree whose entire value is that it is byte-exact. Doing that in the same session
that widened a hostile-input surface is two reviews wearing one hat.

**The general shape, which is the part worth keeping:** a constant that was true of
every context a component has ever run in is indistinguishable from a fact until the
component is moved — and the first move is exactly when nobody is looking for it.

## 6. Both ends of the cable, kept in one vocabulary

`LE-80`'s discipline did its job before a board was touched. Ti64Dink's
`ConsoleParityTests` **reads `tos64_cmd.rs`** and went red the moment the table grew a
third row — a row the board holds that the console cannot name is a failing test, and
so is the reverse. The console now:

- sends `SHELL <command>` with the line in the fixed field, **explicitly** — routing
  every unrecognised line to the shell would have been convenient and would have
  destroyed rule 3, which is that an operator must be able to watch the board deny an
  unknown verb by default;
- un-escapes the answer back into the lines `TINYCMD` wrote, asserted as the exact
  inverse of the board's two reversible escape classes — and asserted **not** to invert
  the third, because a `?` the board substituted is printed as a `?`. Inventing the
  octet back would be the host fabricating board output;
- prints what did not fit rather than leaving it absent, and names an over-long command
  line before sending it.

## 7. Gates

| Gate | Result |
|---|---|
| `cargo test --workspace` | green — 1378 host tests, 0 failed |
| `check-boot-images` | green — 5 AArch64 variants built, 4 crates linted for the target including `shell --lib` |
| `check-guest-images` | green — 22 x86_64 Tier 0 fixtures compile |
| `check-assurance-spine` | green |
| `check-spine-files`, `check-crate-sizes`, `check-ci-gates` | green |
| `check-metric-labels`, `check-performance-catalogue` | green |
| `check-feasibility` | green after regenerating `goals/feasibility.html` |
| `check-tool-tests` | green — 3 bench-instrument suites |
| `check-lints` | green — 8 packages linted individually |
| `cargo fmt --all --check` | green |
| `dotnet test` (ti64dink) | green — 55 tests |

**Seven hand-edits to `goals/index.html` were needed to make the spine green again** —
the tabstrip count, the progress-bar width, the state-count footnote, both generated
tile blocks, the spine-count sentence and the `EPIC-P0`/`P1` population — plus a
regenerated `feasibility.html`. Every one was a number the gate had already computed
and printed in its own failure message. That is `21A` §5 item 3 measured rather than
asserted, and §8 item 3 is where it goes next.

## 8. Step 3, on silicon — the destination

Filed as [`REPORT-2026-08-08-02`](../../goals/reports/REPORT-2026-08-08-02.md).
`STORY-P1-09-18` criterion 6 and `TEST-P1-09-18-A` clause 7 are Green.

```text
tos64>     sent   : verb=SHELL id=3 seq=5 line="FIND /N "cable" README.TXT"
    ANSWER : verb=SHELL seq=5
    | ---------- README.TXT
    | [2]This session runs over the Ethernet cable.
tos64>     sent   : verb=SHELL id=3 seq=6 line="DEL README.TXT"
    ANSWER : verb=SHELL seq=6
    | Access denied: verb Delete is not granted to session WIRE [audited]
tos64>     sent   : verb=WOBBLE id=0 seq=9
    REFUSED: unknown-verb seq=9

console: 0 exchange(s) went unanswered
```

The bench was already wired: `BOOT_ORDER=0xf12` is network-then-SD, so serving
the image and cycling the plug is the whole deployment. `tos64-netboot` confirmed
the transfer whole (1027 blocks, digest matched at both ends), `tos64-power`
cycled and confirmed by readback, and the board came up
`TOS64-RESULT/1 fixture=measure ok=true`.

**The board's own row corroborates the console independently**, and one number in
it does work the console could not:

```text
TOS64-CMD/1 last=SHELL answered=17 refused=2 lastlen=144
```

`lastlen=144` is exactly `COMMAND_PAYLOAD_BYTES` — so `LE-122`'s MAC-side FCS
strip still holds at the widened envelope, which is the one thing a width change
on this path could plausibly have broken.

### 8a. The envelope widened, and the old argument was one quantifier too strict

Mid-session the command envelope went from 46 to 144 octets, and the reasoning is
worth keeping because it is a correction to `-17` rather than a change of mind.
`-17` made the frame **exactly** the Ethernet minimum so no NIC padding could
reach a fixed-width classifier. The hazard is real; the quantifier was wrong. A
NIC pads *up to* the minimum and never pads a frame already at or above it, so
every width from 46 upward carries the identical guarantee — the property needed
is `>=`, never `==`. Pinning the floor bought nothing and cost the command line
98 octets.

`ARGUMENT_BYTES` is now **128, and derived rather than chosen**: it is exactly
`shell::capacities::MAX_LINE`, the longest line `TINYCMD`'s own front-end will
parse, held against it by a `const` assertion in the composition root. The wire
can now carry every line the shell can read. `FIND /N "cable" README.TXT` is 26
octets and would have fitted at 30; `SHELL FIND /N "quarantine" DOCS\NOTES.TXT`
would not have.

### 8b. The finding: the console's window is shorter than the board's busy period

A first session driven immediately after boot reported `TIMEOUT` on its first
three exchanges and answered normally from the fourth. §8's `answered=17`
accounts for every frame sent **including those three** — so the board answered
them, late, while still running its boot measurement campaign.

Nothing here is a defect and one thing here is a success: the console reported
the condition as a **timeout** and explicitly *not* as a refusal, which is rule 1
of its design and exists so an operator does not go hunting in the verb table
when the board is merely busy. It behaved correctly under the first real
condition that ever exercised it. **Operationally: wait for `TOS64-MEAS/2 END`
before typing, or pass a longer `--timeout`.**

### 8c. `LE-124` confirmed by the board rather than predicted

```text
tos64>  line="VER"  →  TinyOS Version 0.2.0 (Tier 0, x86_64)
```

Raised this morning from a host transcript; this is the Pi 5 saying it. No claim
in `REPORT-2026-08-08-02` rests on that line, and it is now item 2 of §9 rather
than a worry.

### 8d. Bench state left behind

The board is running `d23652c6…` (the widened image) and answers `SHELL VOL`
correctly with the netboot server **stopped** — the image is in RAM and the
server's job was done at boot.

**`tos64-netboot` was deliberately stopped rather than left running**, and UDP 67
and 69 are confirmed free. `LE-87` is exactly about this: a server nobody started
this sitting once silently won the bind and served a wrong image with a complete,
plausible, entirely wrong envelope. Leaving one up for a convenience the next
session did not ask for is how that repeats. `C:/tmp/tftproot` still holds
`d23652c6…` in both its root and `7bf18f79/`, so re-serving it is one command —
but **the next power cycle with no server boots the SD card** (`BOOT_ORDER=0xf12`
is network-*then*-SD), and a session that wants this image must serve it
deliberately.

The plug is ON. The Pi OS role was never booted this session, so `LE-117`'s
tripwire was not owed and runbook §6b's sudoers line is **still uninstalled** —
it remains owed the next time that card boots.

## 9. What the next session does, in order

1. **`21A`'s three steps are done, so the next session picks its own destination.**
   That is the first time in this project's history that sentence has been true of the
   mandate, and `21A` §6 is explicit that what follows is the owner's to choose and
   will be a better-informed choice than any made before a human had used the thing.
2. **`LE-124`.** Now confirmed by the board (§8c), and the first thing a demo
   transcript will be read for. Its fix makes the architecture and tier session-
   supplied rather than literal, which is what `EPIC-P2` §3.2 already wants — the
   defect forces a design improvement rather than a patch. It regenerates
   `golden/parity-smoke.golden.txt`, so it is its own review.
3. **`21A` §5 item 3** — make `emit-dashboard` and `emit-feasibility` *write* rather
   than print. This session paid the filing cost **seven times** and still did not
   automate it, which is the honest state of that item and the strongest brief it will
   ever have.
4. **The withheld verbs are now a live question rather than a hypothetical.** `MEM`,
   `TASKMGR` and `SPOOR` are denied to the wire because they disclose live kernel
   state to a peer with no identity; the write half is denied for the same reason.
   Both wait on the session/authentication story, and a human has now used the
   read-only half enough to say what the authenticated half should be for.
5. `20A` §7's four items — `LE-121`'s silicon run, `LE-117` half (2),
   `STORY-P1-09-17`'s remaining criteria, and the measurement sweep — were *"not
   cancelled and not next"* while a step was in flight. **No step is in flight now**,
   so this is the moment `21A` §5 item 2 describes: they are picked up deliberately,
   not because they were the only open rows.

## 10. Standing instructions earned

**A dependency is a claim about every target a crate will ever be built for, and
nothing checks a claim nobody makes.** `shell`'s source was arch-neutral by
construction and had been for its whole life; its manifest disagreed, and the
disagreement was invisible for exactly as long as no gate compiled it for a second
architecture. The register could not hold this, because a defect register records what
is broken and this was not broken — it was *unasked*. The general form: **a crate's
portability is a property of its build graph, not of its code, and the only thing that
can state it is a gate that builds it somewhere else.**

**Every handshake gets a gate, and the gate gets a mutation.** Owner instruction,
2026-08-08, and this session was the evidence for it. Three parity gates existed for
the command vocabulary — verb table, refusal names, envelope arithmetic — and two
handshakes on the *same path* had none: the escape pair that carries every line of
shell output, and the answer's own field names. Both would have drifted in complete
silence, and the symptom of each is worse than a crash: an operator reading literal
backslashes, or an empty transcript under a command that in fact succeeded. Both now
read the board's source in both directions, **and both were mutated until they failed
before being trusted** — a parity test that has never been shown to fail is exactly the
trap `ci_gates.rs` records, where prose quoting a command satisfied a gate that was
supposed to check the command.

**A quantifier is a design decision and deserves the scrutiny of one.** `-17` fixed the
command envelope at *exactly* the Ethernet minimum to make NIC padding impossible. The
hazard was real and the reasoning was sound; the quantifier was one character too
strict, because padding immunity needs `>=` and not `==`. That single character held
the command line at 30 octets — a keyhole in front of a 128-octet parser — through the
first wire session ever run. The general form: **when a bound is justified by a hazard,
check whether the hazard justifies the bound's *shape* or only its direction.**
