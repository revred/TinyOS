# 05E — Cover Note for the Next Session

Short by design. [`04E`](04E-the-report-that-cannot-go-stale-and-what-it-shows.md)
is the last handover from this session; [`03D`](03D-the-board-is-still-talking.md)
is the concurrent one's. This note exists to answer one question the owner
asked: **what shrinks the gap the feasibility report shows?**

Read [`goals/feasibility.html`](../../goals/feasibility.html) first. Open it
from disk. It regenerates in one command and CI refuses a stale copy.

## The uncomfortable finding, stated first

**Closing the entire measurement backlog would not change the verdict.**

The report's verdict is about *goals*: one of six substantially met, one partly,
four not started. The 195 empty closable gates are an **evidence** gap. Moving
`25 / 220` to `220 / 220` would prove that what exists works — it would not make
TinyOS coexist with Linux, install on a laptop, host an inference runtime, or
take orders from an agent, because **none of those has any code**.

So a session that spends itself on measurement is doing real, honest work that
leaves the feasibility verdict where it found it. Know that before choosing.

## Two decisions gate the real gap, and neither is yours

1. **Qualify one platform under `ADR 0005`.** This is the single highest-leverage
   act available to the project. It unlocks 10 `G04` bound gates and makes
   assurance `verified` reachable for the first time — today it is arithmetically
   locked at `0 / 100` (`LE-94`) and no engineering moves it. Blocked on
   `LE-95`: a **£15 LAN relay**. That is the entire obstacle between this project
   and its first qualification record.
2. **Lift or except the hardware-evidence sprint rule** (2026-07-30) for one
   not-started goal. Phases 3–8 have no Epic documents at all. Until an Epic
   opens, four of six founding goals cannot move by definition.

**Put both to the owner before starting anything else.** If the answer to (1) is
yes, the work below reorders completely.

## What you CAN do today, in leverage order

1. **`Q2` — the secure-world determination. Do this first.** It is the largest
   gap in the qualification record and simultaneously the cheapest: **pure
   laptop and documentation work, no board, no relay, no elevation.** Read the
   Broadcom/Raspberry Pi firmware material and determine what is routed to
   `EL3` — or establish that the firmware is closed and **write that down in the
   ADR's own words**, which `ADR 0005` explicitly accepts as a valid `Q2`. With
   `Q4` already written in near-record-ready language across three documents,
   one afternoon takes the record from zero parts held to two of four.
2. **`LE-103` — correct the `Q3` instrument.** `probe_pmccntr` reads
   `CNTVCT_EL0`; `Q3` needs `CNTPCT_EL0`, and since `CNTVCT` is `CNTPCT` minus
   `CNTVOFF_EL2` the existing probe is blind by construction to the residency it
   would be used to detect. Writing the physical-counter sibling and its host
   tests is laptop work. **Do not file it as done** — an instrument that has
   never run on the board is not evidence, and adding a second confident-looking
   `Q3` probe beside the first is how this gets worse.
3. **`LE-111` and `LE-104` — read and re-harvest.** `LE-111` (concurrent
   session) records three evidence rows quoting a boot that is no longer the one
   on the wire; `LE-110` made re-harvesting the cheapest action on the bench.
   `LE-104` records gates that were measured, committed and never read against
   their target. Neither needs code or a power cycle.
4. **The 69 measurable-today gates.** 14 distinct measurements, nine owed by all
   ten instrumented domains, so one harness arm moves ten gates.
   `xtask assurance-status` prints the worklist by domain and by guardrail.
5. **The 56 with no instrument.** `D01`, `D06`, `D08`, `D24` declare no
   `MetricLabel` anywhere. These are fixture-building jobs, not measurement
   jobs (`LE-109`), and calling them measurable overstated the position until
   2026-08-07.

## The board is available, and more of it than four handovers implied

`LE-110`: the Pi 5 has been transmitting a full Tier 1 envelope continuously,
and a **passive capture needs no elevation and no power cycle**:

```sh
dotnet run --project work/tools/ti64dink -- --live 30 --text C:\tmp\env.txt
cd os && cargo run -q -p xtask -- parse-meas C:\tmp\env.txt
```

`LE-95`'s relay blocks **booting a new image**. Every handover from `10C` to
`01C` generalised that into *the board loop is blocked*, and the half that
needed no relay went unused for two days. **When you record a blocker, record
its scope** — a blocker without one becomes a blocker on everything near it.

## Five traps, current

- **`git add -A` while another session is live commits work you did not write.**
  It happened twice to this session, the second time *after* the trap was
  documented. `git status` before staging; stage paths.
- **A cover note's task list can be actioned by somebody else between writing
  and reading.** `01C` item 1 was true when written and false four hours later.
  Re-check before starting.
- **`cargo test` will not save you from a lint.** The local gate is
  `cargo run -p xtask -- check-lints`, per package. The *workspace* clippy form
  is red on the pristine tree and misreads in both directions.
- **A source-level scan matches its own text.** Four instances now. The most
  recent failed on its own needle list — the literals it searched for were
  lines containing the needle.
- **`check-boot-images` and `check-guest-images` are siblings.** Touching
  `kernel`, `hal-arm64`, `pi5-image`, `exec` or `shell` means both.

## Do not start

`FEAT-P1-12`, `G09`/`LE-86`, `06A` §4.3. **Do not add design surface** — and
that explicitly includes the filesystem-on-a-database question, which now has a
reference note (`docs/filesystem-on-a-database-libsqlfs-note.md`) that
deliberately stops short of decomposition. Note also `LE-112`: `EPIC-P2` still
declares itself blocked on `LE-48`, which closed nine days ago. **Correcting
that clause belongs to whoever owns the Epic**, not to a passing session — the
replacement wording is a judgement about whether the RAM-volume answer is the
one that Epic wants.

## The standing instruction that earned its place this session

**Check what the code does on the machine before writing down what it does** —
extended twice on 2026-08-07 and both extensions cost a red build to learn:

> An instrument that can be handed an argument it accepts and cannot match will
> answer confidently about nothing. And **before recording a command as the way
> to check something, run `xtask help`** — a trap entry teaching a raw `cargo`
> invocation teaches *around* the gate this project already owns.
