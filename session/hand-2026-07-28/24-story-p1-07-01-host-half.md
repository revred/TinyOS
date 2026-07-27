# Handover 24 — `STORY-P1-07-01`: the Half That Did Not Need a Board

Follows: [`21-next-session-mandate.md`](21-next-session-mandate.md), which asked for exactly this
and named the split. Companion: [`23-bcm2712-divergence-record.md`](23-bcm2712-divergence-record.md),
which holds the hardware facts and is the document to reopen, not this one.

**On the folder date.** This repository's document dates run one day ahead of the clock, per
Handover 13 §"A note on dates". Do not read a date here as evidence of when anything happened.

## A concurrent session ran alongside this one

Per [`agent/CONCURRENT_SESSIONS.md`](../../agent/CONCURRENT_SESSIONS.md) rule 7, stated up front.

This session started at `ecfebee`. While it ran, `5eef109` — Handover 22, "The zero is real; the
reason this repository gives for it is wrong" — arrived on `main`, taking slot 22. **Slots 23 and
24 were claimed by creating the files before writing them** (rule 4), which is why this handover
is 24 rather than 22 and why there is no renumbering paragraph to write.

That session also has **uncommitted work in this tree** at the time of writing: `STORY-P0-01-05`,
`TEST-P0-01-05-A`, `goals/assurance/guardrail-evidence.tsv`, and modifications to
`story-contracts.tsv`, `xtask/src/assurance.rs` and `xtask/src/main.rs`. **None of it was staged
here** (rules 1 and 3) — nothing in this commit is a file this session did not write. Two
consequences worth knowing:

- the spine counts quoted below (57 Stories, 44 Tests) **include that session's uncommitted work**,
  which is precisely why Handover 21's protocol made population counts floors rather than totals;
- `check-assurance-spine` was green over the combined tree, so it is green over this session's
  subset, but the reverse inference does not hold and nobody should draw it.

The `.githooks` gate is installed (`git config core.hooksPath .githooks`), so the gates ran **after**
staging.

## What was built

The mandate's split, followed exactly: *"build the host-testable half now, and let the adapter gate
the Green."*

| Deliverable | Where |
|---|---|
| AArch64 target spec | `os/targets/aarch64-tinyos.json` |
| Linker script | `os/targets/aarch64-tinyos.ld` |
| BCM2712 constants, hardcoded-and-verified with source revisions | `os/src/hal-arm64/src/board.rs` |
| `CurrentEL` decoding — pure | `os/src/hal-arm64/src/exception_level.rs` |
| PL011 driver behind an MMIO seam | `os/src/hal-arm64/src/pl011.rs` |
| Boot stub, entry report, `EL2 → EL1` drop | `os/src/hal-arm64/src/boot.rs` |
| CI step that builds and lints the AArch64 target | `.github/workflows/ci.yml` |

`hal-arm64` went from **470 lines and 12 host tests** to **1,070 lines and 64 host tests**. The
workspace suite is 498 passing (from 438, with part of the increase belonging to the concurrent
session's `xtask` work).

**`STORY-P1-07-01` is `In progress`, not `Verified`.** Criteria 1 and 5 are Green; criterion 2 is
written and never executed; criterion 3 is Green for the decode and the ordering and blocked for the
register read; criterion 4 is entirely blocked. `TEST-P1-07-01-A` carries the per-clause table and
**not one clause was edited** — the specification is the one written before implementation.

## Three things a reviewer should look at first

**1. The seam is exactly where clause 5 asked, and the boundary is stated rather than assumed.**
`pl011::VolatileMmio` is the only `cfg(target_arch = "aarch64")` item and the only `unsafe` in the
driver. It is *not* true of the Story as a whole, and cannot be: a stack and a zeroed `.bss` need
assembly, and clause 4 requires both. The driver-scoped reading is recorded in `TEST-P1-07-01-A`
under its own heading, with the reason, rather than left for a later reader to infer that the clause
was ignored.

**2. The bounded poll has a derivation, and it is deliberately loose.** `TX_POLL_LIMIT = 100_000`,
derived in the doc comment from one 87 µs character time and a Device-nGnRnE MMIO read. It is
generous on purpose: the bound exists to convert a hang into a return, not to enforce a latency
budget, and a tight bound here would be a timing claim about a board nobody has run. Three tests
drive it, including one asserting the poll count is exactly the documented limit.

**3. The configuration *ordering* is tested, because it is the part that fails while looking
correct.** `LCR_H` latches `IBRD`/`FBRD` on write, so programming `LCR_H` first — which reads
perfectly naturally — silently leaves the firmware's baud in effect. `CR` is written last so the
device is never enabled half-programmed, and a rejected baud leaves it untouched rather than
half-configured.

## What the tests caught that review would not have

**A doubled carriage return in a piece of evidence.** The first implementation spelled its line
endings `"\r\n"` inside `Pl011::write_str`, which frames `\n` as `\r\n` — putting `\r\r\n` on the
wire. It is invisible on most terminals. It would have survived review and landed inside a *quoted
serial capture* offered as clause 4's evidence, and the capture would then have been subtly not what
the source said. The framer owns the CR; the report supplies `\n` only, and a test now pins it.

**An error variant that could never fire.** A `BaudError::UnachievableRate` was written to reject
divisors outside async framing tolerance. The PL011's six fractional bits bound the worst case at
under 0.8%, only at the fastest expressible rate, against a ~2–3% tolerance — so the variant was
unreachable code defending against a state the hardware cannot enter. Deleted, and replaced by a
test asserting the bound across a matrix of clocks and rates. Recorded because "defensive code that
cannot fire" reads as rigour and is the opposite.

**A vacuous assertion.** `assert!(KERNEL_LOAD_ADDRESS >= RAM_BASE)` with `RAM_BASE == 0` — always
true, caught by clippy. The constant went with it: a constant that is always zero and only ever
added is noise a reader has to check.

One test of my own was wrong on first run (`0x…FFF3` decodes to `EL0`, not `EL3` — the level is bits
`[3:2]`). Fixed in the test, since the implementation was right; noted only because this
repository's convention is that a failing pre-committed assertion gets recorded rather than quietly
restated, and this was neither pre-committed nor an assertion about behaviour.

## What this deliberately did not do

- **No `kernel8.img`.** Producing one needs an AArch64 binary crate and an SD-image build, which is
  `STORY-P1-07-05`. The linker script was validated by linking a throwaway binary *outside* the
  workspace — layout confirmed, `_start` the first byte of the flat image — and the recipe is in
  Handover 23. That is a layout check, not clause 2's evidence, and the Test document says so.
- **No seventh Story, no HAL port.** `FEAT-P1-07` §6 held. No RP1, no PCIe, no GPIO, no device-tree
  parser, no address spaces, cores 1–3 parked in `_start`.
- **No measurement, and none is available to quote.** The MMU is off, every access is
  Device-nGnRnE, and `TEST-P1-07-01-A` §7 says what a number obtained here would be worth. Nothing
  was measured; the temptation never arose, because nothing ran.
- **`LE-09` is untouched and open.** So is `LE-23`, the named fallback — board work was possible, so
  the fallback was correctly not taken.

## What the next session does

**Buy two USB-serial adapters and loopback-test one before the board is ever blamed.** That is
`TEST-P1-07-01-A` clause 1, it is the only clause in the Feature runnable before anything else
exists, and it is now the *only* thing standing between this Story and its Green.

Then, in order:

1. Write `config.txt` with **`os_check=0`** — Handover 23 §3, the divergence with no test behind it
   and the one that presents as total silence.
2. Read the first line off the wire. It says `current_el=` and it is the cheapest output this Story
   produces.
3. Confirm the capture, quote it verbatim into `TEST-P1-07-01-A` clause 4, and only then move the
   Story to Verified.
4. **`STORY-P1-07-02` next, before the MMU, always.** Until it lands, every failure on that board is
   the same silence.

If the board says nothing, work Handover 23's list in order: `os_check=0`, then the connector's
mux, then the adapter again. The address, clock and baud are the *least* likely candidates, because
they are the ones with tests behind them.

## State at the close

```
main                    ecfebee + 5eef109 (concurrent); this work committed on top
assurance spine         green: 23 Features, 57 Stories, 44 Tests, 45 Reports, 32 loose ends (21 open)
                        counts include a concurrent session's uncommitted Story — floors, not totals
host tests              498 passing across the workspace; hal-arm64 12 -> 64
aarch64 build           green, and now a CI step
EPIC-P1                 4 Features of 7 complete
STORY-P1-07-01          In progress; criteria 1 and 5 Green, 3 half, 2 and 4 need the board
LE-09                   OPEN
```

Nothing here is evidence. A crate that compiles for AArch64 is not a board that ran it, and the
first byte on that serial line is still worth more than everything above.
