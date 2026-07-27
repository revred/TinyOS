# Handover 21 — Next-Session Mandate: Start `STORY-P1-07-01`, Board or No Board

The start-here document written at the close of 2026-07-28. `main` is at `92fc889`, **pushed, and CI is green on every job** — including the timing gate.

**On the folder date.** This repository's document dates run one day ahead of the clock, as Handover 13 §"A note on dates" records. Do not read a date here as evidence of when anything happened.

## Where the project stands

**`EPIC-P1` is four Features of seven.** `FEAT-P1-01` through `-04` are complete. `FEAT-P1-05` and `-06` are Specified and untouched. **`FEAT-P1-07` is Specified, fully contracted, and is the next work** — six Stories, six Test documents already written, and it is the only Feature in the repository that can close `LE-09`.

That last point is the whole argument for what follows. **38 Stories carry assurance state `baseline-debt` and not one is `verified`, and they are blocked by one thing, not fifty-six.** Every timing Report in this project says the same sentence: no hardware tier. `FEAT-P1-05` and `FEAT-P1-06` would each produce more Tier 0 evidence, which is the one thing this project already has a surplus of.

Read, in this order:

1. [`19-feat-p1-07-acceptance-and-spine.md`](19-feat-p1-07-acceptance-and-spine.md) — the §7 decisions as confirmed, the contract choices worth not re-litigating, and the three traps.
2. [`17-raspberry-pi-5-bring-up-plan.md`](17-raspberry-pi-5-bring-up-plan.md) — the plan itself. §4 is four hardware realities; §4.1 is the one that must not be discovered late. **Its §8 loose-end numbering is superseded** — read it against Handover 19.
3. [`goals/tests/TEST-P1-07-01-A.md`](../../goals/tests/TEST-P1-07-01-A.md) — **your Red.** It is written. You do not start at spec.
4. [`20-swot-response.md`](20-swot-response.md) — what changed about how this repository is committed to, and why.

## First, two minutes of setup

```
git config core.hooksPath .githooks
```

Per-clone, so a fresh clone does not have it. It runs the spine, catalogue and crate-size gates **after staging**, which is the exact gap that pushed a broken spine to `main` on `585a027`. If another agent may be working this tree, read [`agent/CONCURRENT_SESSIONS.md`](../../agent/CONCURRENT_SESSIONS.md) first — it is seven rules and it is binding.

## What to do

**Start `STORY-P1-07-01`.** It is the target spec, the boot stub, the `EL2 → EL1` drop, the PL011 UART, and one byte on the wire.

**You can start it whether or not a serial adapter is in your hand**, and this is the useful thing to notice about the Story. Split it the way `STORY-P1-01-03` split the timer:

- **Needs no hardware** — the target spec and linker script; PL011 register offsets, baud-divisor arithmetic, flag-polling and byte framing behind a one-method MMIO seam; `CurrentEL` decoding; the `.bss` zeroing and stack setup. All host-tested, all pure, on the x86_64 dev machine. This is clause 5 of the Test document and it is most of the Story.
- **Needs hardware** — clauses 1, 3 and 4: the adapter loopback, the printed exception level, and the byte arriving. That is the Green, and it is one session's work once the parts exist.

So the honest sequencing is: **build the host-testable half now, and let the adapter gate the Green.** Do not wait for hardware to start, and do not claim the Story Verified without the capture.

**If no board time is available at all**, the named fallback is `LE-23`: re-record the timing baseline from a CI run to remove the consistently-signed 23–53% Windows-vs-Linux offset. It is small, it needs no hardware, and `LE-24` may come free with it — `pool_u64x64` measured 25 cycles on the Linux runner and 0 on the Windows dev box, so the ungating is a property of where the baseline was recorded rather than of the metric. **One fix, two loose ends.** Do not take this instead of the board work if the board work is possible; take it instead of idling.

## Traps, named up front

**1. You start at Red, not at spec — and the Test document is not yours to soften.**
`TEST-P1-07-01-A` was written before any implementation, deliberately, and its clauses were fixed before anyone knew which would be inconvenient. This is the same unusual starting position Handover 10 described for `STORY-P1-04-02`. The precedent to follow is `TEST-P1-01-04-A` clause 4: a pre-committed bound came back outside its threshold and was **recorded as failed** rather than restated. That behaviour is the single most valuable thing this project does. If a clause turns out to be wrong, correct it *in the open, with the reason*, the way `TEST-P1-02-01-A` clause 4 records the `wrmsr` correction.

**2. Pi 4 material is actively misleading, and the failure is silent.**
The Pi 5 is a larger departure than the version number suggests, and the debug UART is a **dedicated 3-pin connector, not the GPIO header**. Verify every BCM2712 address and the expected baud against current documentation and firmware notes. Record the divergences you find from Pi 4 sources — that is the most reusable output this session produces, and nobody else will write it down.

**3. Loopback-test the adapter before the board is ever blamed.**
A suspected-dead board is usually a dead adapter. Buy two. This is clause 1 of the Test document and it is first for a reason: it is the only clause in the entire Feature that can be run before anything else exists.

**4. Print `CurrentEL` before anything else, and drop conditionally.**
The firmware's entry level is an *input*, not a constant. Hardcoding `EL1` and being wrong gives faults or silence; hardcoding `EL2` and being wrong gives the same in the other direction, with no way to tell them apart. One line of text converts the plan's second-highest risk into a fact.

**5. Do not measure anything, and do not believe anything you accidentally measure.**
Until `STORY-P1-07-03` lands the MMU, every access on that board is Device-nGnRnE — uncached, unbuffered, no speculation. The temptation at the end of a successful bring-up session is *"let's just see how fast it is."* Any number obtained that way is not slow-but-proportional, it is **meaningless**, and the danger is that it will be quoted. `TEST-P1-07-01-A` §7 says this; it is repeated here because the temptation arrives precisely at the moment the session feels successful.

**6. Bounded polling loops only.**
An unbounded wait on the PL011 transmit-FIFO-full flag is a hang indistinguishable from every other hang on this board — and until `STORY-P1-07-02` there is no fault reporting to tell you which one you have.

**7. Do not let it grow into a HAL port.**
`FEAT-P1-07` §6 is the boundary: no RP1, no PCIe, no Ethernet, no USB, no GPIO, no address spaces, no preemption, no SD driver, no device-tree parser, single core. **A seventh Story means re-decomposing, not extending.** `hal-arm64` is 234 lines today; the pressure to make it "a proper port" while you are in there will be real.

## What not to be misled by

- **A green spine is not evidence.** `FEAT-P1-07` has a Feature contract row, six Story rows, six Test documents and a Feature document. None of that is a measurement. `LE-09` closes on `STORY-P1-07-06`'s Report and **nothing earlier**.
- **`SEC-01` is selected and cannot be closed.** The Pi 5 firmware chain gives TinyOS no measured-boot evidence, so `BND-01` is stated debt for the whole Feature. Do not let a successful boot read as a verified boot.
- **The timing gate is now ~2x, not ~1.6x.** A real 50% regression on a gated path passes. If you find yourself looking at `relative_percent: 100` and thinking it should be tighter, read `LE-16` first — the `const _: () = assert!(…)` and its derivation comment exist to stop exactly that edit, and run 1 of the cross-host data would have gone red at 60%.
- **Do not reach for `--update-baseline` locally.** It rewrites every measured row with whatever your host produced, and nothing in it asks where the previous rows came from. That is `LE-28`, and it is one command away from turning a confirmed cross-host offset into a false green.

## State at the close

```
main                    92fc889, pushed, CI green on every job
assurance spine         23 Features, 56 Stories, 43 Tests, 44 Reports
                        30 loose ends (20 open), 82 status headers
host tests              438 passing across the workspace; clippy clean
EPIC-P1                 4 Features of 7 complete
LE-09                   OPEN
```

The first character on that serial line is worth more than every remaining Tier 0 Feature in this Epic, because it is the first thing this project has ever done that QEMU was not doing for it.
