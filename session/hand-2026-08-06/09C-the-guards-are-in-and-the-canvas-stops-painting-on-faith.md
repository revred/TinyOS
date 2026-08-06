# 09C — The Guards Are In, and the Canvas Stops Painting on Faith

Session handover, written 2026-08-06, executing
[`08C`](08C-the-two-gates-are-filed-and-the-density-question-answers-against-us.md)
§6 in order, in the same tree, with **no plug and no board run**.

**The one sentence, if only one survives:** *both of `08C`'s guards are in and
both were falsified by mutation, and the canvas now refuses to paint a
hardcoded physical address on a boot where the firmware reported no display —
which narrows `LE-98` rather than closing it, because the address is still a
constant from a file.*

---

## 1. Item 4 first, because review found it before I did

`08C` claimed the existence check on `--server` "stays quiet" when there is no
evidence. **That was wrong, and the code was stricter than the description.**
The silence condition was `known.Length > 0 && !known.Contains(parsed)`, and
`EnumerateHost` filtered to interfaces that were `Up` — so on a cold
`board-run` the host reports WiFi and loopback, neither is the bench address,
and the warning fires **on every correct run**. Crying wolf on the automated
path, which is the exact training effect the check was designed to avoid.

Probed on the bench rather than reasoned about, and it is worse than the
reading suggested:

```
Up    Ethernet                     [169.254.113.248]
Down  Bluetooth Network Connection [169.254.92.35]
Down  Local Area Connection* 1     [169.254.92.0]
Up    Loopback Pseudo-Interface 1  [127.0.0.1]
```

**`Loopback Pseudo-Interface 1` is always `Up`**, so `known` is never empty on
this host and the quiet branch was not merely rare — it was dead.

The fix is the one the review named, and it closes `08C` §6 item 4 as a side
effect: **a disconnected adapter keeps its APIPA address and .NET reports it**
(`Bluetooth` above is `Down` and still holds `169.254.92.35`). So the bench
address is enumerable *before* the board powers. The address a disconnected NIC
still holds **is** the "recorded bench address" item 4 was waiting for — the
operating system already recorded it.

What changed:

- The NIC walk now returns `HostAddress { Address, LinkIsUp }` and **records**
  link state instead of filtering on it, because the two consumers disagree and
  *that disagreement is the fix*. `LinkLocalCandidates` keeps the link-up
  requirement — admitting dead adapters would turn a cold bench from
  `NoCandidate` into `Ambiguous` and a warm one from `Discovered` into
  `Ambiguous`. `KnownHostAddresses` drops it.
- Loopback is excluded from evidence: it is in the refused set, so offering it
  as "did you mean" offers the one address the tool would reject.
- `Candidates` and `HostAddresses` are now separate fields. One field with two
  meanings is read wrong by whichever caller was written second.

`netboot.tests` **49 → 54**, including one that runs against the real host,
because the walk and its two projections are a seam and `LE-66` says a
declared-thin seam with no test is not thin, it is untested.

## 2. `LE-99`'s guard — closed, and both halves falsified

`PERF-D05-G23`'s filed note says the shipping park loop exceeds the per-round
stamp budget by 3.3× and that the only thing making it harmless is the 1 Hz
beat. Nothing tied that sentence to the code. Now two things do, one per crate,
because the beat is in `hal-arm64` and the stamp is in `kernel` and no single
test can see both.

**The cadence.** `PARK_TICK_MS`, `PARK_BEAT_TICKS` and `PARK_BEAT_MS` are named
constants the loop actually uses, with a test asserting the beat is 1 Hz whose
failure message names the gate. A second test reads the source to confirm the
loop paces itself from the constants rather than from literals beside them, and
a third counts the dispatch rounds in the beat body.

**The stamp.** `STAMPS_PER_DISPATCH_ROUND` in `kernel::board_dispatch`, with a
source-level test asserting stamps equal exits equal three — one stamp per
round on every path. Source-level because it cannot be behavioural:
`tinyos_dispatch_round` drives a real context switch through statics, so a host
cannot call it and count.

Neither guard encodes a measured figure. The cycle costs belong to a boot, not
to a design constant.

**Two holes in the first cut, found by review and closed before this landed.**
Both were the ordinary way a second stamp arrives, and both would have failed
*quiet* — the direction that matters:

- **A stamp inside a callee.** The scan read `tinyos_dispatch_round`'s own body,
  so a stamp added to `dispatch::run_once` was invisible — which is verbatim the
  breach `08C` §2 predicted. Closed by a companion test asserting the round's
  transitive stamp surface is empty: `dispatch.rs`'s shipped half contains no
  stamp at all. That turns *"this function stamps once"* into *"one round stamps
  once"*, which is what `PERF-D05-G23` actually rests on.
- **An unqualified call.** The needle required the `crate::spoor_stream::`
  prefix, so a `use` plus a bare call counted zero and the round could stamp
  four times with the test green. The needle is the bare identifier now, which
  catches both spellings since the qualified one contains it.

Falsified rather than assumed — five mutations, each seen to fail on its named
test:

| mutation | result |
|---|---|
| `PARK_BEAT_TICKS` 10 → 5 | `the_park_beat_is_one_hertz_and_perf_d05_g23_depends_on_it` FAILED |
| a second `dispatch_round()` in the beat | `the_beat_runs_exactly_one_dispatch_round` FAILED |
| a fourth stamp in `tinyos_dispatch_round` | `one_dispatch_round_stamps_exactly_once_on_every_path` FAILED |
| `use` + an **unqualified** stamp in the round | `one_dispatch_round_stamps_exactly_once_on_every_path` FAILED |
| a stamp inside `dispatch::run_once` | `nothing_the_round_calls_stamps_underneath_it` FAILED |

One note on reading that table: rows 3 and 4 are caught by the `stamps == exits`
assertion, **not** by the one naming `STAMPS_PER_DISPATCH_ROUND`. While the
constant is 1 those two assertions say the same thing, so the second is
tautological today. It earns its place only if the constant ever changes —
which is exactly the moment you would want it — but nobody should read the
table as evidence that the constant itself is under test.

**`LE-99` is closed.** Its filed debt was *"nothing guards the assumption — no
test, no gate and no comment ties the park beat to this budget."* Three of those
now exist. The over-budget ratio itself is not a loose end; it is
`PERF-D05-G23`'s row, where it belongs.

**And the row says what the guard cannot see**, because a closed row that reads
as fully guarded is worse than an open one: the transitive check is one module
deep, both scans are *text* so a macro-emitted or generated stamp counts zero,
and a renaming re-export would slip the needle. **A text scan is not a call
graph.** `LE-98`'s row states its residue; this one has no excuse not to.

### The trap in a source-level test, recorded because it cost two rounds

A test that greps its own file **matches its own assertions first**. The first
cut failed against itself twice, at two nesting depths — the second time inside
the helper written to fix the first. Every needle is now assembled with
`concat!("if tick.is_multiple_of(", "PARK_BEAT_TICKS) {")` so no literal can
match its own definition, and the search is scoped past the glue banner. Worth
knowing before writing the third one of these.

## 3. `LE-98` — the canvas stops painting on faith

The real half, and **the obvious fix would have broken the board.**

`hdmi.rs` already performs a validated firmware mailbox exchange, so the
tempting answer was to paint into that framebuffer. But this firmware
**refuses** the framebuffer grant — `fb=refused`, `BOARD VERDICT 4` — so gating
on it would have turned the display off permanently on the only hardware the
project owns. There is now a test asserting exactly that, because it is the
mistake the obvious fix makes:
`the_refused_framebuffer_grant_does_not_veto_the_canvas`.

The evidence this firmware *does* produce was already being computed and thrown
away. `show_splash`'s phase 1 asks the display's native size, and the answer was
folded into `choose_mode` and discarded. It is a different property tag from the
grant, and the firmware can only answer it from a display it has brought up.

- `show_splash` returns `DisplayOutcome { native, framebuffer }` — **both**
  answers, kept separate, because this board answers them differently.
- `hdmi::canvas_permitted` is the one pure decision, with its own tests.
- `canvas::SimplefbSurface` has **no public constructor**. `Canvas::permitted_by`
  is the only route to one, so a boot with no display cannot reach the
  `write_volatile` at all.
- Absent, the canvas reports width and height **zero**, so every drawing
  routine's existing bounds arithmetic writes nothing — the refusal reuses the
  guard that is already proven instead of adding a second one beside it.
- **It is not silent.** The park loop emits
  `TOS64-CANVAS/1 painting=no reason=firmware-reported-no-display (LE-98)`,
  because a surface that refuses and reports nothing is `LE-87`'s shape, which
  is what `LE-98` was raised about.

### What this does not fix, which is why the row stays open

**The address is still a constant from a file.** `SIMPLEFB_BASE` came from a
Raspberry Pi OS capture and there is still no device-tree parser (`BND-03`). A
display existing is not a framebuffer existing *at that address* this boot. The
unjustified write is narrowed from *every boot* to *boots where a panel is
reported* — strictly better, and not the answer. A device-tree parse ends the
argument; nothing else does.

**The fault path still paints without evidence, deliberately**, named at
`Canvas::last_resort_for_fault_report` rather than left as an unexamined
default. It can run before the splash has asked, and serial has never produced
a byte on this bench (`LE-47`), so a fault that paints nothing is a board that
hangs with no symptom. That is a trade of one hazard against a worse one, not a
justification — and it is the second thing a device-tree parser would remove.

**None of it is verified on hardware.** No board run has happened.

## 4. `LE-91` — not started, and why

`08C` §6 item 3. Its own filed estimate is **one session**, and it needs a
mechanism that does not exist: a per-metric domain-and-owning-Story declaration
parsed out of the fixture sources, because one fixture serves six domains while
`list-fixtures` maps a fixture to one owning `TEST`. Starting it here would have
produced a half-built parser instead of two closed guards and a narrowed safety
defect. It is the next session's headline, unchanged and still the right
mechanism before the 127.

## 5. The next session, in order, still with no plug

1. **`LE-91`** (§4). One session, closes a class rather than an instance, and
   `PERF-D11-G01` is the worked example of what a bent label costs.
2. **`LE-98`'s remaining half** (§3): the device-tree parse that makes
   `SIMPLEFB_BASE` evidence rather than folklore, and removes the fault path's
   named exception with it.
3. **When the board next runs**, and it is now a checklist rather than a
   discovery: `hdmi_force_hotplug=1` and HDMI0; whether
   `TOS64-CANVAS/1 painting=no` appears (which would say the 2026-08-06 dark
   canvas was a firmware display failure and not a TinyOS one); and `LE-96`'s
   remaining half — a separate process seeing `.tos64-transfer` *during* a
   transfer.

**Do not start:** `FEAT-P1-05`'s RT reserve, `G09`/`LE-86`, `06A` §4.3. The
hardware-evidence sprint rule from 2026-07-30 has not been lifted, and no design
surface was added here.

## 6. State at close

- **Gates:** spine green, `check-boot-images` green (3 variants — the only thing
  that compiles the changed park loop and fault path), `check-guest-images`
  green, `check-lints` green, `check-citations` green, `cargo fmt --check`
  clean, workspace suite green. `check-timing-regression` RED and unchanged for
  the `LE-23` owner decision. `netboot.tests` 54/54, `power.tests` 99/99, all
  ten C# projects build.
- **Spine:** 31 Features / 99 Stories / 82 Tests / 62 Reports, **99 loose ends
  (49 open** — `LE-99` closed, `LE-98` narrowed and still open**)**, **25 of
  460** release gates carrying evidence. Evidence did not move this session and
  was not expected to: nothing here is a measurement.
- **Bench:** board left **powered and beaconing**, untouched. UDP 67/69 clear.
  **No plug on the desk.**
- **Uncommitted.** Nothing committed, `git add -A` never used
  (`CONCURRENT_SESSIONS` rule 1). `03B` through `09C` are all uncommitted in this
  tree, so **stage by path**.

**The standing instructions, all holding.** Do not report `x/460` undecomposed.
`PERF-Dnn-Gnn` is only meaningful if `Dnn` is the domain of what you measured.
Verify the digest and size the window before you spend the boot. A gate written
for one architecture, one tool or one direction does not generalise itself.
Build the unblocker rather than the next blocked artifact, and say so. A tool
that prints the value it chose is not the same as a tool that refuses a value it
cannot justify. An escape argument is a property of one denominator and does not
generalise to the next.

**And an eighth, from this session:** *check what the code does on the machine
before writing down what it does* — `08C` said the existence check would stay
quiet on a cold run, the code said otherwise, and thirty seconds of enumerating
the actual adapters settled it. The same half-hour turned a check that would
have cried wolf into the one that closes the case it was deferring.
