# Handover 26 — Next-Session Mandate: `STORY-P1-07-02`'s Host Half, Adapter or No Adapter

The start-here document written at the close of 2026-07-28. `main` is at `74d3904`. **CI was red on
`fe62ee7` and is green again on `74d3904`** — see "The red `main`, and the two-word command that
would have prevented it", below, because it is the most immediately useful paragraph in this
document.

**On the folder date.** This repository's document dates run one day ahead of the clock, as
Handover 13 §"A note on dates" records. Do not read a date here as evidence of when anything
happened.

## Where the project stands

Three sessions ran concurrently on this date and each moved a different thing. The short version:

- **`STORY-P1-07-01` is `In progress`.** Its host-testable half is Green — target spec, linker
  script, PL011 driver, `CurrentEL` decoding, boot stub, 64 host tests. It is blocked on **one
  physical object**: a loopback-tested USB-serial adapter. ([Handover 24](24-story-p1-07-01-host-half.md))
- **Ten release gates now carry dated evidence**, from 0 tracked. `0 / 56 Stories
  assurance-verified` is unchanged beside it, correctly. ([Handover 25](25-the-first-gates-that-need-no-hardware.md))
- **`LE-31` is open and is the clearest non-hardware work in the project**: the attribution of the
  assurance-verified zero to `LE-09` is wrong for nine Stories, and nobody has audited what actually
  blocks each one. ([Handover 22](22-the-zero-is-real-but-its-reason-is-wrong.md))

Read, in this order:

1. [`24-story-p1-07-01-host-half.md`](24-story-p1-07-01-host-half.md) — what exists in `hal-arm64`
   now, and the three findings its tests produced.
2. [`23-bcm2712-divergence-record.md`](23-bcm2712-divergence-record.md) — **the hardware reference.**
   Open this before touching the board, not after. Every row names how it fails, and §3 is the one
   with no test behind it.
3. [`goals/tests/TEST-P1-07-02-A.md`](../../goals/tests/TEST-P1-07-02-A.md) — **your Red.** Written
   before implementation, like `-01`'s was. You do not start at spec.
4. [`21-next-session-mandate.md`](21-next-session-mandate.md) — still authoritative for the traps.
   Its *fallback* is superseded here; nothing else in it is.

## First, two minutes of setup

```
git config core.hooksPath .githooks
```

Per-clone, so a fresh clone does not have it. And read
[`agent/CONCURRENT_SESSIONS.md`](../../agent/CONCURRENT_SESSIONS.md) — it is seven rules, it is
binding, and **three sessions shared this tree on the last working date**. Claim your handover
number by creating the file before you write it; slot 27 is free.

## The red `main`, and the two-word command that would have prevented it

`fe62ee7` turned CI red on `clippy::assertions_on_constants`. The session that wrote it *did* run
clippy on the changed crate and saw nothing, because it ran:

```
cargo clippy -p hal-arm64 --all-targets            # <- no -D warnings
```

The lint was a **warning**, CI promotes warnings to errors, and the local grep was looking for
`error`. So the check ran, passed, and proved nothing.

**Always run clippy the way CI does:**

```
cargo clippy -p <crate> --all-targets -- -D warnings
```

Note the per-crate form. `cargo clippy --workspace --all-targets -- -D warnings` — the exact CI
command — **cannot pass on a Windows dev host**: eleven targets fail to compile (`exec` ×9,
`kernel`, `os`) because they are target-only code being built for the host. That is `LE-12` from
the other direction, and it means a Windows session's only honest local lint is per-crate. Say so
in your handover if you relied on it.

For the AArch64 side there is now a second command, and CI runs both:

```
cargo clippy -p hal-arm64 --target targets/aarch64-tinyos.json \
  -Z build-std=core,compiler_builtins \
  -Z build-std-features=compiler-builtins-mem \
  -Z json-target-spec -- -D warnings
```

## What to do

**Build `STORY-P1-07-02`'s host-testable half.** This is the recommendation whether or not an
adapter has arrived, and the reason is arithmetic: of `TEST-P1-07-02-A`'s five clauses, **four are
host-testable and one needs the board.**

| Clause | Needs |
|---|---|
| 1 — all sixteen vectors present, 128-byte alignment asserted at build time | host |
| 2 — a deliberately-triggered fault prints a decoded `ESR_EL1` | **board** |
| 3 — `ESR_EL1` decoding: class, IL bit, class-specific ISS | host |
| 4 — a fault frame is evidence, never authority | host |
| 5 — the handler is bounded, allocation-free, non-reentrant | host |

That is a larger host share than `-01` had. The `-01` split worked and the pattern is now proven, so
**apply it again rather than rediscovering it.**

The payoff is not merely "progress while blocked". It is that when an adapter does arrive, **one
board session can close both Stories' hardware clauses** — `-01`'s capture and `-02`'s fault
injection — instead of two sessions separated by however long it takes to write `-02`'s code. The
scarce resource here is board time, not host time.

### If an adapter *is* in your hand

Do `-01`'s Green first; it is one session's work and it converts an `In progress` Story into the
first hardware evidence this project has ever had.

1. **Loopback-test the adapter against a known-good source before the board is ever blamed.**
   `TEST-P1-07-01-A` clause 1. A suspected-dead board is usually a dead adapter. Buy two.
2. **`config.txt` needs `os_check=0`.** Handover 23 §3. Without it the Pi 5 firmware decides the
   image is a Linux kernel and loads it at `0x200000` instead of `0x80000`, and the symptom is total
   silence. It is the only constant in that record with **no test behind it**, because `config.txt`
   is on an SD card.
3. **Read the first line.** It says `current_el=`. That is the Story's cheapest and most valuable
   output.
4. **Quote the capture verbatim** into `TEST-P1-07-01-A` clause 4, then — and only then — move the
   Story to Verified.

If the board says nothing, work Handover 23's list **in order**: `os_check=0`, then whether the
3-pin connector is muxed to UART rather than A76 SWD, then the adapter again. The address, clock and
baud are the *least* likely candidates, because they are the three with tests behind them.

### If there is no board time at all

The named fallback has changed since Handover 21. Take them in this order:

1. **`LE-31`** — audit what actually blocks each Story from `verified`, rather than attributing all
   56 to `LE-09`. Handovers 22 and 25 arrived at this independently, from opposite directions, which
   is the strongest signal in this repository right now. Handover 25 did the easiest slice by hand
   and found ten gates that were never waiting on hardware; the rest of the audit is unlikely to be
   empty.
2. **`LE-23`** — re-record the timing baseline from a CI run to remove the confirmed 23–53%
   Windows-vs-Linux offset. `LE-24` may come free with it. One fix, two loose ends. This was
   Handover 21's fallback and it is still valid; it is now second because `LE-31` changes what every
   later session *believes*, and a wrong belief compounds.
3. **`D08`'s stale readiness field** — Handover 25 flagged it: recorded `prototype-inactive`, which
   `FEAT-P1-03` made untrue. Small, and it is a correctness problem in data other decisions read.

**Do not take a fallback if board work is possible.** Take it instead of idling.

## Traps, named up front

**1. You start at Red, and `TEST-P1-07-02-A` is not yours to soften.**
Same as last session, and it held: not one clause of `TEST-P1-07-01-A` was edited. Where a clause
needed *interpretation* — its "only `cfg(target_arch)` item, only `unsafe`" is unsatisfiable for a
Story that must establish a stack — the reading was written into the Test document under its own
heading, with the reason. Do that again. Do not do the other thing.

**2. Clause 2 has no version that passes without inducing a fault.**
The Test document says so in bold and it is the sharpest sentence in it: *a claim that failure is
visible, tested only against code that does not fail, is not a test.* If you find yourself building
`-02` such that it could be marked Verified from host tests alone, you have built the wrong thing.

**3. The 128-byte vector alignment must be a build-time assertion, not a run-time one.**
Clause 1 is explicit. A misaligned `VBAR_EL1` write is **architecturally ignored** — no fault, no
error, the handler simply never runs. That is the exact symptom this Story exists to eliminate,
arriving through the Story's own front door.

**4. Order is not negotiable: `-02` before `-03` and `-04`.**
`-03`'s MMU and `-04`'s timer are the two easiest things in this Feature to get subtly wrong, and
the first symptom of either is an exception. Until `-02` lands, that exception is a silent hang.

**5. Do not measure anything, and do not believe anything you accidentally measure.**
Until `STORY-P1-07-03` lands the MMU, every access on that board is Device-nGnRnE. Any number
obtained is not slow-but-proportional, it is **meaningless**. The temptation arrives precisely at
the moment a bring-up session feels successful, which is why this is restated every time.

**6. Do not let `hal-arm64` grow into a HAL port.**
`FEAT-P1-07` §6 is the boundary: no RP1, no PCIe, no Ethernet, no USB, no GPIO, no address spaces,
no preemption, no SD driver, no device-tree parser, single core. The crate went 470 → 1,071 lines
last session, legitimately. **A seventh Story means re-decomposing, not extending.**

**7. There is no `kernel8.img` yet, and building one is not your Story.**
Producing an image needs an AArch64 binary crate and an SD-image build: that is `STORY-P1-07-05`.
`-01`'s linker script was validated by linking a throwaway binary *outside* the workspace, and
`TEST-P1-07-01-A` records that as a layout check rather than as clause 2's evidence. If you need an
image to run `-02`'s fault fixture, that is a real dependency — **say so and decide deliberately**,
rather than growing one in passing.

## What not to be misled by

- **A green CI run is not evidence, and neither is a compiling AArch64 crate.** The build step added
  last session exists so the target spec cannot rot; it says nothing about whether any of it works.
  `LE-09` closes on `STORY-P1-07-06`'s Report and **nothing earlier**.
- **Ten release gates with evidence is 10 of 391.** Handover 25 is emphatic that the Story-level
  zero was *joined*, never replaced. Do not let the new register read as progress toward `verified`
  that it is not.
- **The spine's population counts are floors, not totals.** They include whatever another session
  has uncommitted in the tree. A green `check-assurance-spine` over a mixed tree is green over your
  subset; the reverse inference does not hold.
- **`SEC-01` is selected by this Feature and cannot be closed.** The Pi 5 firmware chain gives
  TinyOS no measured-boot evidence, so `BND-01` is stated debt. A successful boot is not a verified
  boot.
- **Do not reach for `--update-baseline` locally.** It rewrites every measured row with whatever
  your host produced. That is `LE-28`, and it is one command from turning the confirmed cross-host
  offset into a false green.

## State at the close

```
main                    74d3904 (green; fe62ee7 was red, fixed in 74d3904)
assurance spine         23 Features, 57 Stories, 44 Tests, 45 Reports
                        32 loose ends (21 open), 83 status headers
                        10 release gates with dated evidence
host tests              498 passing across the workspace; hal-arm64 at 64
aarch64                 hal-arm64 builds and lints clean against the target spec, in CI
EPIC-P1                 4 Features of 7 complete
STORY-P1-07-01          In progress — criteria 1 and 5 Green, 3 half, 2 and 4 need the board
STORY-P1-07-02          Specified, untouched — four of five clauses are host-testable
Stories verified        0 / 56
LE-09                   OPEN
LE-31                   OPEN, and the clearest non-hardware work in the project
```

The blocker is now a single physical object. That is a better position than this project has been in
for the whole of `EPIC-P1`, and it is also the position in which it is easiest to wait instead of
working — which is why the mandate above does not depend on the adapter arriving.
