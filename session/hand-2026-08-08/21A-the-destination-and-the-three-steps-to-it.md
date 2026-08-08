# 21A — The destination, and why a week of good work did not approach it

Follows [`20A`](20A-the-board-answered-the-question-it-was-asked.md), same date,
written at the owner's finding rather than at a unit of work. **This is a
mandate, not a record**: it contains no code, closes no row, and files no
evidence. It exists because the previous handover was the seventh in a week that
each closed real defects and left the project, in the owner's words, *staring at
an OS with a splash screen*.

**The one sentence, if only one survives:** *The human-usable OS already exists —
`TINYCMD`, a labelled volume, a DOS front-end, a byte-exact parity gate — and it
runs on **x86_64 under QEMU**, while the Pi 5 board runs a different image whose
entire payload is instruments; nothing in the tree joins the two, so no amount of
good board work approaches a human using TinyOS, and the fix is three steps
rather than a phase.*

## 1. The finding, in the owner's terms

> The sad state of play is that even after a week of development, I am staring at
> an OS with a splash screen. You failed to create a sustained long life session
> with clearing the bandwidth built into it so as to complete the OS to a human
> usable state.

Both halves are recorded because both are right, and they are different faults.
The second is about how sessions were run. The first is about what they were
aimed at, and it is the one this document is mostly for — because it would have
held at any session length.

## 2. The diagnosis: two tracks, and nothing joins them

The week's board sessions produced `LE-118`, `-119`, `-120`, `-121`, `-122` and
`-117`'s reading: every one a real defect, each closed properly, each filed with
evidence. The sum is **an instrument that can now prove things about an OS
nobody can use.**

That is not a failure of care. It is a failure of aim, and it has a structural
cause that is visible in one line of a manifest:

```text
src/pi5-image/Cargo.toml:  hal-arm64, kernel        <- the board's whole payload
```

`shell` and `exec` are **not in the board image at all**. The usable surface —
`TINYCMD`'s verb core, the deny-by-default policy seam, the labelled RAM volume,
the DOS front-end, the `.TCB` batch runner, and `TEST-P2-07-01-A`'s
golden-transcript parity gate that holds it byte-exact — exists, is tested, and
lives on the **x86_64** side. The board's image carries `boot`, `measure`,
`fault`, `qual-control`, `qual-campaign`, and since `20A` a two-verb command
channel.

So the board can be *proven* and cannot be *used*, and the two facts are
unrelated by construction. **Every board session adds evidence; none of them
could have added usability, because the thing a human would use was never
compiled into the thing a human would boot.** A week of closing board defects was
always going to end at a splash screen, and the register — which is what chose
the work — had no way to say so.

## 3. The chain: three steps to a human typing at TinyOS

Short, and each step reuses what the one before it proved. This is the whole
plan; it is not a phase and it should not become one.

**Step 1 — `shell` compiles for the board.** Its source is already
arch-neutral: `#![cfg_attr(not(test), no_std)]`, `#![forbid(unsafe_code)]`,
heap-free, and every byte of output goes through a `core::fmt::Write` sink
precisely so a QEMU fixture, a host test and some future host render
identically. What blocks it is the **manifest, not the code** — see §4, which
measured it rather than assuming it.

**Step 2 — the board image carries it, and the command channel reaches it.**
`20A` proved the seam: a fixed-width envelope, classified by a total function
over fixed offsets, answered over the cable within the beat, with a two-row
deny-by-default verb table. Widening that table from `PING`/`STATUS` to a
request/response surface over `TINYCMD`'s verb core is the work — and note the
containment argument moves with it, because an admitted frame would then select
a verb *and its arguments*. `STORY-P1-09-17`'s expiring absence argument is the
one that must be re-made, exactly as `-17` re-made `-16`'s.

**Step 3 — Ti64Dink's console points at the shell.** `ti64dink --console`
already types at the board and matches answers to sequences. Pointing it at the
verb core instead of the two-verb table **is** a human at a TinyOS prompt over
Ethernet — the owner's standing priority (1), reached with the components that
already exist rather than with new ones.

Step 3 is the demo. Steps 1 and 2 are the only work.

## 4. Step 1's obstacle, measured

Not inferred. `cargo check -p shell --lib` against the board target:

```text
error[E0433]: cannot find `x86_64` in `arch`   --> src/hal-x86_64/src/tsc.rs:135
error: invalid register `dx`: unknown register --> src/hal-x86_64/src/actuation.rs:57
error: the `att_syntax` option is only supported on x86
                                                --> src/hal-x86_64/src/boot.rs:83
error: invalid register `ecx`: unknown register --> src/hal-x86_64/src/interrupts.rs:515
```

**Every error is inside `hal-x86_64`. Not one is in `shell`'s own source.** The
crate declares `hal-x86_64` and `kernel` as unconditional path dependencies for
the benefit of its *fixture binary* (`shell-batch-fixture`, the Tier 0 QEMU
target), and Cargo dependencies are crate-wide, so the library drags an x86 HAL
onto an AArch64 target it never calls into.

The first task is therefore small and precisely stated: **make those two
dependencies belong to the binary that needs them**, target-gated or
feature-gated, so `shell`'s library target builds for `aarch64-tinyos` — then
prove it stays that way by adding `shell --lib` for the board target to
`check-boot-images`, which is the only gate that compiles anything for AArch64
and the reason three pushes once went green locally and red on the runner
(`LE-72`).

Write the gate first. A crate that compiles for the board today and silently
stops tomorrow is exactly the failure `check-boot-images` exists to catch, and
this dependency is the kind that gets re-added by someone fixing an unrelated
fixture.

## 5. What changes about how sessions choose work

The second half of the owner's finding, answered concretely. The fix is **not** a
longer session — the harness already carries work across a summarised context,
and the same drift would happen at any length. The fix is where a session gets
its goal.

1. **The destination selects the work; the register does not.** Sessions have
   been opening `loose-ends.tsv`, sorting by what is broken, and closing it.
   That is how a week of correct decisions summed to no progress toward use.
   From here a session opens **this document** and asks which of §3's three
   steps it is advancing.
2. **Loose ends get triaged against the chain, not just filed.** A row is either
   *blocking a step* or *not*, and the not-blocking ones stay open **on purpose
   and in writing**. `LE-121`'s silicon run and `LE-117`'s half (2) are real and
   neither blocks a human typing at the board; they should not be next simply
   because they are open. Nothing here weakens the no-waiver rule — an open row
   stays open and visible; what changes is that it stops silently setting the
   agenda.
3. **The filing overhead gets automated rather than absorbed.** `20A` spent a
   material fraction of its session hand-editing dashboard counts,
   `feasibility.html`, status-header phrasing, and working around a criterion
   parser that stops at an apostrophe. `emit-dashboard` and `emit-feasibility`
   already *print* what was edited by hand; making them **write** turns that
   cost into a command. This is bandwidth returned to the OS, and it is the
   cheapest item in this document.
4. **Sessions state their aim in the first line of the handover**, so a mandate
   drifting back into register-clearing is visible in the record instead of only
   in hindsight.

## 6. What this mandate is not

- **Not a criticism of the week's evidence work, and not a licence to skip it.**
  `LE-118` had the board deaf on arrival on any real segment; `LE-122` had every
  command refused before the verb table was read. Neither step 2 nor step 3
  would run today without them. The fault was never that the work was wrong —
  it is that nothing pointed the next one at a human.
- **Not a request to bypass the assurance spine.** Steps 1–3 decompose into
  Stories with contracts like everything else, just-in-time, and step 2 in
  particular widens a hostile-input surface and owes its containment argument
  in full.
- **Not a claim that three steps make a finished OS.** They make **one human
  typing at TinyOS over the cable, on real silicon**, which is the first point
  at which the thing can be used at all. What follows that is the owner's to
  choose, and it will be a much better-informed choice than any made from here.
- **Not a re-plan of Phase 2.** `TINYCMD` is built. This document moves it onto
  the board; it does not redesign it.

## 7. What the next session does, in order

1. **§4's first task**: `shell --lib` builds for `aarch64-tinyos`, with the gate
   written first and wired into `check-boot-images`.
2. **Step 2**: the board image carries `shell`, and the `TOS64-CMD/1` verb table
   widens onto `TINYCMD`'s verb core with its containment argument re-made.
3. **Step 3**: `ti64dink --console` at the verb core — the demo, on silicon.
4. **§5 item 3** whenever a session is already paying the filing cost: make
   `emit-dashboard` and `emit-feasibility` write.

`20A` §7's four items — `LE-121`'s silicon run, `LE-117` half (2),
`STORY-P1-09-17`'s remaining criteria, and the measurement sweep — are **not
cancelled and not next**. They are the first things to pick up when a step
lands, and `LE-121`'s run in particular is cheap on a bench that now
power-cycles itself.

## 8. Standing instruction earned

**A register of what is broken cannot tell you what is missing.** Every row in
`loose-ends.tsv` was a true statement about a defect, every closure was correct,
and the set of them contained no sentence saying *the usable OS is not on the
board*. A defect register measures the distance from working; nothing in this
tree measured the distance from **used**, so nothing objected for a week. That
is what §3 is now for, and it belongs beside the destination rather than inside
the register that could not hold it.
