# STORY-P1-09-18 — The Verb Core on the Board: TINYCMD Reached Over the Cable, Read-Only and Stateless

Status: **Functionally Verified — built and proven on silicon 2026-08-08 ([`22A`](../../session/hand-2026-08-08/22A-the-usable-os-is-on-the-board.md)), executing [`21A`](../../session/hand-2026-08-08/21A-the-destination-and-the-three-steps-to-it.md) §3 steps 1, 2 and 3 in their stated order. All six criteria Green; criterion 6 closed on a Raspberry Pi 5 over a direct Ethernet cable, [`REPORT-2026-08-08-02`](../reports/REPORT-2026-08-08-02.md) — `TINYCMD` executed command lines a human typed and answered with its own output, `0 exchange(s) went unanswered`. Assurance state `specified`: the mapped performance domains, security controls and containment classes are unmoved by a functional pass.**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: [`session/hand-2026-08-08/21A-the-destination-and-the-three-steps-to-it.md`](../../session/hand-2026-08-08/21A-the-destination-and-the-three-steps-to-it.md) (the mandate) and [`22A`](../../session/hand-2026-08-08/22A-the-usable-os-is-on-the-board.md) (this document)

## Description

[`STORY-P1-09-17`](STORY-P1-09-17.md) made a frame mean something and bounded what it
could mean to two rows that execute nothing. Its own text recorded the price of that
bound in one sentence: *"a third row is a charter re-read, not an addition."*

**This Story is that re-read, and the thing it lets through is the OS.** `21A` found
that `TINYCMD` — the verb core, the labelled RAM volume, the DOS front-end, the `.TCB`
runner and the byte-exact parity gate that holds them — had been built, tested and
shipped on **x86_64**, while the Pi 5 image carried `boot`, `measure`, `fault`,
`qual-control`, `qual-campaign` and a two-verb command channel. *"The thing a human
would use was never compiled into the thing a human would boot."* Two steps close
that, and this Story is both of them.

### Step 1 — `shell` compiles for the board (`LE-123`)

Measured rather than assumed, exactly as `21A` §4 required. `cargo check -p shell --lib`
against `aarch64-tinyos` produced 29 errors and **every one was inside `hal-x86_64`**;
not one was in `shell`'s own source, which is `no_std`, heap-free,
`#![forbid(unsafe_code)]` and arch-neutral by construction. The obstacle was a
crate-wide dependency declared for the benefit of an x86_64 fixture binary.

**The gate was written first**, per `21A` §4's closing instruction, and it is
`xtask check-boot-images` — the only gate that compiles anything for AArch64, and the
one `LE-72` exists over. `shell --lib` now builds and lints for the board target
beside `hal-arm64`, `kernel` and `pi5-image`.

### Step 2 — the board image carries it, and the command channel reaches it

Three pieces, each reusing what the one before it proved:

- **`VERB_TABLE` gains one row, `SHELL` (id 3).** The envelope, the fixed offsets, the
  total classifier, the deny-by-default resolution and the one-line-per-beat answer
  rate are `-17`'s, untouched. What changed is that the fixed 30-octet argument field
  now means a command line for exactly one row.
- **A runner seam, not a handler.** `tos64_cmd` still executes nothing: it classifies,
  reports the pending line through `CommandChannel::pending_line`, and renders what
  the caller hands back in `AnswerText`. The run happens through a
  `#[no_mangle] extern "C"` symbol, the same inverted-dependency shape
  `hal_arm64::spoor` already uses to reach `kernel` — necessary because `shell`
  depends on `kernel` and on AArch64 the dependency runs `kernel` → `hal-arm64`, so a
  HAL naming `shell` would be a cycle.
- **`pi5-image` composes it**, because it is the only crate that sees the whole graph.
  It supplies the grant set, the seeded volume and the session name; the `extern "C"`
  shim is a clamp and a call, so every decision is in a module compiled and
  unit-tested on **every** architecture (`LE-66`'s lesson: a thin seam that decides
  anything is a decision no host test can reach).

## The charter, read again — not cited

`-17` answered each obligation for a table that executed nothing. That answer no
longer covers the table, so each is answered again here for a row that does.

- **`PD-02`** — *"Kernel-derived caller identity … never from caller-supplied
  identifiers."* The wire peer still has **no** authenticated identity, and `-17`'s
  consequence — the table may hold only verbs whose answers disclose what the board
  already broadcasts and whose execution changes nothing — is **kept, not relaxed.**
  It is satisfied a second way:
  - *Execution changes nothing.* The runner builds its `World` from a `const` seed on
    every command and drops it before returning. There is no `static`, no cell and no
    carried handle on the path, so no cwd, environment variable, file, label or
    counter survives one wire command into the next: the board after any admitted
    sequence is bit-identical to the board before it. This is a property of the shape
    rather than a discipline, which is why it is asserted directly rather than
    audited for.
  - *Nothing new is disclosed.* The grant set is the **read-only** subset of the verb
    core over a volume the image itself seeded, so a peer can read back only bytes
    that shipped in a published artifact.
  - The session name is a fixed constant (`WIRE`) and no field of the frame reaches a
    policy decision. Identity is not taken from the caller because there is no place
    for it to be taken from.
- **`BND-03`** — *"C1 contains no complex hostile-format parser."* The classifier is
  unchanged: fixed offsets, no value from the frame used as an offset, length or
  address, verb resolution bounded by the table's own length. **What is new is that a
  fixed-width field is now handed onward as data,** and the honest statement of that
  is: `shell`'s DOS front-end is a parser, it is not in C1, and it parses a
  30-octet buffer this crate copied at a fixed width — never a length the frame
  supplied. The old sentence *"no byte of the argument steers anything"* has been
  replaced with a narrower true one rather than being quietly kept; both halves are
  pinned by tests, and the retirement is recorded in the test that used to hold it.
- **`PD-14`** — *"No ambient namespace or class-derived authority."* Two tables now,
  and both deny by default: the verb table (a row absent from it does not exist) and
  the `GrantSet` (a verb absent from it does not run, and the denial is audited into
  the transcript the peer receives). The second enumerates over `VerbKind::ALL`, so a
  verb added to the core tomorrow is denied here without anyone remembering to deny
  it.
- **`SEC-20` / amplification** — the rate bound is unchanged *and the work bound is
  new*. The run happens **inside** the bounded answer slot, so one admitted frame
  costs exactly the one beat its answer was always going to cost. One frame in, one
  line out: shell output that exceeds the answer line is carried as a prefix with the
  withheld octets counted in a `more=` field, never continued into a second frame.
- **`RCG-*`** — untouched, asserted not inherited. No byte from the wire is code. The
  command line is compared, trimmed at a fixed width and executed **as text by an
  interpreter**, which maps nothing, launches nothing and patches nothing; `shell`
  carries `#![forbid(unsafe_code)]` and cannot.
- **`PD-07` / fail-closed** — a receive error still disables receive terminally. The
  runner has no error channel because it has no failure the shell would not already
  have printed, and every input — including one that is not UTF-8 — produces output
  rather than silence.

### What is withheld, and what it waits on

Recorded as decisions so a later session does not read them as oversights.

- **Every mutating verb** (`CD`, `COPY`, `MOVE`, `DEL`, `MD`, `RD`, `SET`, `PATH`) and
  `CLS`. Not because the RAM volume matters — it is rebuilt every command — but
  because granting them would make *"execution changes nothing"* true only by accident
  of the rebuild. The rebuild is defence in depth; the grant set is the defence. `CLS`
  joins them because it emits a real terminal escape, and trusted output that repaints
  an operator's screen is authority over a human.
- **Every verb that reads live kernel state** (`MEM`, `TASKMGR`, `SPOOR`). These are
  read-only and stateless and satisfy the first sentence perfectly; they fail the
  second. A task table, a memory figure and an audit journal are facts only the
  running board holds, and disclosing them to a peer with no identity is a decision
  worth taking deliberately. It waits on the session/authentication story
  (WCI/deploy-protocol model) — the same thing the write half waits on.

## Acceptance criteria

1. `shell --lib` builds and lints clean for `aarch64-tinyos`, and a gate that would
   fail if it stopped doing so exists in `check-boot-images` and was written before
   the fix.
2. Every AArch64 image variant — the featureless one CI builds and all four fixtures —
   links with `shell` in it, and the flattened `kernel8.img` stays inside the 8 MB
   base-image ceiling with the growth measured rather than assumed.
3. `VERB_TABLE` holds exactly three rows; the classifier is still total over all
   65,536 verb ids; the argument steers the `SHELL` row's command line and steers no
   row's classification; the retired `-17` sentence is retired in writing where it
   was asserted.
4. One admitted frame produces exactly one answer line whatever the shell printed;
   output that does not fit is carried as a labelled prefix whose count is exact; no
   octet a runner can return can end the line early or move an operator's cursor.
5. The wire session is stateless and read-only, asserted on the wire path and not only
   on the runner: no command can change what the next command sees, every verb outside
   the grant set is denied with the denial spoken, and the grant set contains no
   mutating verb and no live-kernel-state verb.
6. **Board:** `ti64dink --console` sends `SHELL VER`, `SHELL DIR` and `SHELL DEL
   README.TXT` to a running Pi 5 and the capture shows `TINYCMD`'s own output for the
   first two and an audited denial for the third — a human typing at TinyOS over
   Ethernet, on silicon.

## Progress, 2026-08-08

| Criterion | State |
|---|---|
| 1 — `shell --lib` for the board, gate first | **Green.** `LE-123` measured, closed by target-gating `hal-x86_64` to `cfg(target_arch = "x86_64")`; `check-boot-images` now compiles and lints `shell --lib` for `aarch64-tinyos`. |
| 2 — the image carries it | **Green.** All five AArch64 variants build and link. Flattened `kernel8.img` 323,400 → 525,624 octets: 6.3% of the 8 MB ceiling, and the delta is a measurement rather than an estimate. |
| 3 — three rows, classifier still total | **Green (host).** 27 tests in `tos64_cmd`, including all 65,536 ids re-exercised and the two-directions parity test that reads the Rust source from Ti64Dink's side. |
| 4 — one frame, one bounded line | **Green (host).** `ANSWER_CAPACITY` raised 128 → 256 (the largest the existing text frame can carry); ` more=` reserved before the output is written so the field reporting what did not fit can never itself be what did not fit. |
| 5 — stateless and read-only | **Green (host).** 13 tests in `pi5-image`, five of which drive a real `TOS64-CMD/1` envelope through the board's own classifier into the real verb core and back out as a rendered answer line. |
| 6 — the board answers a typed command | **Green on silicon, 2026-08-08** ([`REPORT-2026-08-08-02`](../reports/REPORT-2026-08-08-02.md)). Nine typed exchanges, `0 exchange(s) went unanswered`: `VOL` returned the seeded volume's label and serial, `FIND /N "cable" README.TXT` returned a numbered match out of the board's own RAM volume, `DEL` and `SET` were each refused by name and audited against the `WIRE` session, and `WOBBLE` was refused by the verb table. A `DIR` before and after two denied mutations was byte-identical. The board's own row read `TOS64-CMD/1 last=SHELL answered=17 refused=2 lastlen=144`. |

**One defect this work found that no test was looking for.** The first wire transcript
ever rendered from the board's own composition answered `SHELL VER` with *"TinyOS
Version 0.2.0 (Tier 0, x86_64)"* on an AArch64 target. It is a literal in
`shell/src/verbs.rs`, it was true of every context the shell had ever run in, and it is
pinned byte-exact by `TEST-P2-07-01-A`'s golden transcript — so the gate that exists to
catch shell output drift is currently the thing requiring the false string. Raised as
`LE-124` and deliberately not fixed here: regenerating a golden transcript in the same
session that widens a hostile-input surface is two reviews wearing one hat.

## Depends on

- `STORY-P1-09-17` — the envelope, the classifier, the rate bound and the containment
  argument this Story re-makes rather than cites.
- `LE-122`'s closure — every command was refused before the verb table was read until
  the MAC stopped handing four FCS octets to a fixed-width classifier.
- `FEAT-P2-01`/`-02`/`-04` — the verb core, the labelled volume and the DOS front-end,
  used as they are and not modified.

## What landing this Story must also do

- `FEAT-P1-09`'s contract row: the authority posture now names a third row that
  executes, and the hostile-input list gains *"a 30-octet command line, interpreted as
  DOS syntax by `shell` outside C1"*.
- `LE-124` before the board demo is filed as evidence of anything the `VER` line is
  quoted in.
- The 30-octet command-line bound is a real limitation of the fixed-width envelope and
  is stated in the console rather than discovered at a bench; if it bites, widening it
  is a change to `-17`'s no-padding argument and owes that argument again.
