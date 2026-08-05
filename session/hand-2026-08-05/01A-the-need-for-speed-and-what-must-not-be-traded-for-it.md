# 01A — The Need for Speed, and What Must Not Be Traded For It

Session handover, written 2026-08-05 at the owner's request, after the session that executed
[`hand-2026-08-04/05A`](../hand-2026-08-04/05A-the-board-speaks-in-spoors-and-what-a-late-listener-cannot-know.md)
§5 and closed `LE-72`.

The owner's framing: *"we now have a route to a verifiable OS development cycle to yield 10×
improvement in speedy feature implementation — with TDD, not letting quality down."*

That is the right instinct and this document exists to make it operational. §2 is where the
speed actually comes from, §3 is the list of things that look like speed and are not, §4 is
the friction still worth removing, and **§5 is the next session's mandate**. Read §5 and §3
if you read nothing else.

---

## 0. The one-paragraph state

Three commits landed: `c54b0e4` (`STORY-P1-10-04` — the retained boot certificate and the boot
epoch), `54fee2e` (`BOARD VERDICT 11`–`13`), `fbb7f6b` (`STORY-P1-10-05` — the die temperature
on the wire). `LE-72` closed with the gate it asked for; `LE-73`, `LE-74` and `LE-75` raised.
Spine: **30 Features / 95 Stories / 79 Tests, 75 loose ends (42 open)**, all gates green. The
board proved that a listener joining at record 74 can still read the boot state, that the epoch
distinguishes boots, and that a certificate is byte-identical to a live frame 0. **The card is
in the laptop carrying `f06bfa8ac7ec`** — the thermal image, staged and hash-verified, never
yet booted. Everything is committed; **nothing is pushed**.

## 1. What the loop is now, measured

`STORY-P1-10-05` is the honest benchmark, because it ran end to end in one sitting from an
observation nobody had planned for:

| Step | What happened |
|---|---|
| Observation | Owner: "the fan doesn't spin under TinyOS; it does under Pi OS" |
| Ground truth | Card swapped to Pi OS, booted, SSH, two read-only probes captured |
| Decision | The evidence said *mappable register, not mailbox* — which sized the work before a line was written |
| Design | Raw word on the wire, host converts; sensing separated from actuation |
| TDD | Vocabulary widened test-first; boundary tests updated on both sides of the nibble |
| Gates | `cargo test --workspace`, `fmt`, `check-boot-images` (3 image variants + clippy), `check-assurance-spine` |
| Artefacts | Story, Test, contract row, dashboard, commit |
| Image | Built, hash-verified, staged on the card |

Roughly an hour, and the only human actions were *insert card, apply power*. Compare with the
sessions this project actually lived through: `hand-2026-08-03` diagnosed the Ethernet chain
by **counting LED blinks**, one bit of information per power cycle. Three board diagnoses in an
evening was a triumph then. This session ran three board experiments and read every result
without the owner touching a keyboard.

## 2. Where the speed actually comes from

Four things, and it is worth being precise because only one of them is the wire.

**1. Evidence replaced transcription.** The canvas and the lamp needed a human to read numbers
off a screen and type them into a document. That is a slow, lossy, un-replayable channel.
Machine-parsed records mean a result can be quoted exactly, diffed against the last run, and
re-read months later.

**2. Ground truth before design.** The single highest-leverage step in `STORY-P1-10-05` was
twenty minutes of SSH *before* any code. It settled register-versus-mailbox, which was the
difference between a small Story and a large one, and it corrected a wrong risk statement the
session had already made out loud. **Capturing ground truth is not overhead in the way that
gold-plating is overhead — it is what stops a day being spent in the wrong direction.**

**3. A capture no longer has to be lucky.** `BOARD VERDICT 10` existed because someone happened
to be listening across a power cycle. Since `STORY-P1-10-04`, any capture started at any moment
yields the boot state within ~5 seconds. That converts board evidence from an event you have to
catch into a resource you can query — which is what makes an agent able to run experiments
unattended.

**4. The gates got cheaper than the failures.** `check-boot-images` takes seconds and catches
the class that cost three red pushes. `check-assurance-spine` refuses a commit whose documents
disagree with its register. Both are faster than the debugging they replace, which is the only
argument for a gate that ever holds up.

## 3. What must not be traded for speed — with this session's receipts

Every rule below was earned by something that actually went wrong. This is the section to
re-read when a deadline makes one of them look optional.

**Test-first, and prove the Red.** `STORY-P1-10-04` was Red-verified by neutering the
implementation and watching **14 tests fail**. Tests written after the fact pass because they
were shaped by the code; nobody can tell the difference later, including you. Record the count
in the commit — it is the only durable evidence that Red ever happened.

**Never substitute a constant for a discovery.** `LE-69`: `gic.rs` asserted five GIC priority
bits, BCM2712 implements four, and the code **refused a conforming device**. The cure was not
`0xF8 → 0xF0` — that is one board's measured value replacing another guess. It was to *probe
the width*. `STORY-P1-10-05` applies the same rule to a temperature, which is why the board
ships a raw register instead of a number: a wrong priority mask refuses loudly, but **a wrong
temperature still reads like a temperature**.

**State what you did not measure.** `LE-74` was filed as a design-time caution that the boot
epoch's entropy is borrowed from firmware timing. The board then **measured it**: two
consecutive boots landed **151 counter ticks apart — 2.8 µs**. Only the low byte moved. The
caution was right and far too gentle. A stated limit gets sharpened by evidence; an unstated
assumption gets discovered by a user.

**A gate that cannot see its subject is not a gate.** `LE-72`: `cargo test`, `fmt` and host
clippy all pass on a tree whose AArch64 image does not link. Three pushes went out green
locally and red on the runner. The fix was not more discipline — it was making CI and the local
gate **the same command**, so they cannot drift.

**Widen a vocabulary; never stretch one.** `Category::Thermal` and `Action::Observe` were added
test-first rather than folding a die temperature into `Boot` because the boot crate stamps it.
Note the cost, though: **`Action` now has one verb of headroom before the `ACT` nibble must
widen**, which is a wire-format change to every spoor ever stored. Speed that spends a
budget nobody is watching is borrowing.

**Say when the register and the prose disagree.** `LE-73`: `kernel::udp_wire` cites
`STORY-P1-10-03`, and **no such Story exists** — no document, no contract row, no Test. Every
gate is blind to it because they check documents against each other and nothing extracts
`STORY-*` citations from source. This is the fourth instance of that class (`LE-30`, `LE-65`,
`LE-70`). It is also `FEAT-P1-07` reading `Specified` while its ladder is done on silicon.

**Sensing before actuation.** `STORY-P1-10-05` reads the temperature and does nothing with it.
An actuator fed by a sensor nobody has validated converts a measurement error into a physical
one. The fan is one PWM write away and that write is deliberately not in this build.

## 4. The friction still in the loop, ranked by what it costs

1. **The card shuffle.** Every board experiment costs a power-down, a card move, a stage, a
   card move and a power-up. The charter-neutral answer is already written down: **Pi 5
   firmware netboot** (TFTP, `BOOT_ORDER` in EEPROM) loads the image before TinyOS exists, so
   no code is ever admitted at runtime and rule 9 is not engaged
   ([architecture §7](../../docs/spoor-transport-architecture.md)). This is the single largest
   remaining multiplier and it is an *investigation*, not a Feature, until the EEPROM path is
   understood.
2. **`sudo` on the Pi OS side is password-gated.** It blocked the one thing the thermal work
   still needs — a paired raw-register/`thermal_zone0` reading to calibrate `LE-75`. One
   `NOPASSWD` line on the ground-truth card would make every future capture unattended.
3. **Ti64Dink is one-shot.** `--live N` then exits. A `--until` mode that watches for a
   condition (an epoch change, a rung appearing, a value crossing a bound) would let a session
   *wait for a board event* instead of guessing a window. **This session lost the
   `== BOOT CHANGED ==` line to a `tail -90`** — a defect in evidence plumbing, not in the
   subject, and the transcript says so.
4. **No board-side cost measurement.** `STORY-P1-10-02` criterion 6 is still unmet: per-stamp,
   per-drain and now per-announce cost are unmeasured. An observability substrate whose
   overhead is unknown is one that gets switched off in the run that mattered.

## 5. The next session's mandate — the board has never run the kernel

**This is the finding that should shape the next several sessions, and it is not what
`FEAT-P1-10`'s Story list currently implies.**

That Feature enumerates, undecomposed, *"the extension that wires the existing
`dispatch`/`lock`/`wcet`/`actuation` spoor call sites into the AArch64 path."* That wording
understates the work by a wide margin. Checked this session:

```
grep -rn "sched::|dispatch::|kernel::lock|wcet::" os/src/hal-arm64/src/*.rs   →   nothing
```

**`hal-arm64` references none of them.** The AArch64 path boots, installs vectors, maps memory,
arms a tick, runs a measurement fixture and parks. The scheduler, the dispatcher, the
priority-inheriting locks and the WCET budgets are Tier 0 — QEMU and x86_64 — and have never
executed on silicon. There are no call sites to wire, because nothing calls them.

So the real next milestone is: **run TinyOS's kernel on the Pi 5.** Not a new subsystem — the
existing, well-tested one, on the hardware. And the spoor stream is precisely the instrument
that makes it observable while it happens, which is why it was worth building first.

Suggested shape, smallest honest increment first:

1. **One task, dispatched once, on the board.** The narrowest thing that proves the seam:
   `kernel::sched` creates a task, `kernel::dispatch` selects it, and a `Dispatch` spoor
   arrives on the wire from silicon. Almost certainly a new Feature under `EPIC-P1`, since it
   crosses from observability into execution.
2. **The tick drives dispatch.** `STORY-P1-07-04` armed a 100 Hz virtual timer and proved it by
   interval ratio (`count=1816 rmin=999 rmax=1000`). Today it increments a counter. Making it
   drive a dispatch round is the step that turns a parked board into a running OS.
3. **Then the call sites stamp for free**, because they already do on the host path — which is
   what `FEAT-P1-10`'s enumerated Story was really describing.

**Before any of that**, two short things that close what this session opened:

- **Boot `f06bfa8ac7ec` and read the thermal rung.** The card is staged. Three outcomes, all
  informative: the raw word tracks and drifts like a die temperature (offset `0x200` confirmed);
  it never moves or its validity bits never set (the offset hypothesis is refuted — which is
  exactly why the board ships the raw register); or it reads `0x00000000` (a mapping problem,
  not an offset one). Then one paired Pi OS reading calibrates it and `TEST-P1-10-05-A`
  clause 7 closes.
- **`REPORT-2026-08-04-01`.** Release-blocking. It closes `LE-09`, `LE-15`, `LE-24` and
  `LE-27`, every input exists as machine-parsed bytes, and it is the document that lets
  `FEAT-P1-07` stop reading `Specified` while its ladder is Green on silicon. **Do not write it
  from the photographs.**

## 6. Also owed

- **`FEAT-P1-09`'s exit criterion** — the beacon byte-compared against the frame builder.
  Ti64Dink already captures beacon frames alongside the spoors.
- **`LE-73`** — file `STORY-P1-10-03` or renumber `udp_wire`, then add the gate that extracts
  `STORY-*` citations from source and refuses one with no filed Story.
- **`LE-75`'s actuation half** — the fan, driven from a *validated* reading and never a
  hardcoded duty cycle. Sensing is in; acting is not, deliberately.
- **`LE-56`'s shell-lane half** — the board half is evidenced; the console lane is untouched.
- **`udp_wire`** is written and tested and still not wired to the board.
- **`STORY-P1-10-04` criterion 6** — the certificate's write-once bound is host-Green only; no
  board run has stamped enough certificate rungs to reach the ceiling.

## 7. Bench facts at close

- **Card: in the laptop, TOS64 role, `f06bfa8ac7ec`** (measure fixture + spoor egress + boot
  epoch + retained certificate + thermal sampling), hash-verified by `tos64-cardswap`.
  `pios-backup\` retained; `tos64-cardswap pios` restores Pi OS whenever ground truth is needed.
- **Board unpowered.** Per `LE-75`, power it for a run and power it down after — TinyOS has no
  thermal response, and nothing measured supports leaving it running.
- **The Pi OS side is reachable**: `ssh revanur@raspberrypi.local`, key auth, link-local IPv6
  over the direct cable. `sudo` needs a password.
- **Ti64Dink**: `dotnet run -- --live <seconds>` from `work/tools/ti64dink`, unelevated via
  Npcap. Do **not** pipe it through `tail` — that is how this session lost a line.
- **Three board epochs on record**: `0x049F8B28`, `0x04B328BC`, `0x04B32825`.
- **`ACT` nibble headroom: one verb.** `MAX_RECORDS` is 181. The frame header's reserved
  padding is **spent** — the `flags` word has 15 bits left and there is no spare field.
- **`ANNOUNCE_EVERY = 5`, `CERTIFICATE_CAPACITY = 16`** — both chosen, neither measured, both
  recorded as debt.
- Host `cargo clippy --workspace --all-targets` **cannot** build `kernel`'s `[[bin]]` on this
  Windows machine (`hal_x86_64` is `cfg(not(windows))`, `LE-64`'s class). `check-boot-images`
  is the local signal that is clean.
- **Nothing is pushed.** Three commits sit on `main` locally.
