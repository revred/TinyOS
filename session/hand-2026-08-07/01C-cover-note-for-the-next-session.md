# 01C — Cover Note for the Next Session

Short by design. [`01B`](01B-the-runs-were-never-created-and-not-windows-is-not-bare-metal.md)
is the last full handover and this does not repeat it. This exists because the
thing `01A` told the last session to watch has finally happened, and **the
channel it opened is one this project has never had before**: a test can now
fail somewhere that is not this bench.

## Where the project actually is

Everything is committed and pushed. Tree clean. `main` carries the first
passing host test suite in this project's history — run
[`31163551078`](https://github.com/revred/TinyOS/actions/runs/31163551078),
**1231 tests, 0 failed**, on Linux.

Three loose ends closed in the `12D`→`14D` arc (`LE-91`, `LE-100`, `LE-101`)
now have runner evidence rather than an assertion that they would. `LE-102`
closed the run-creation question and the two defects that surfaced behind it.

The Pi 5 is powered and beaconing, untouched since the 2026-08-06 boot.
**There is still no plug on the desk.** Every item below runs on a laptop.

## `main` is red, it will stay red, and only the owner can change that

One job fails: `check-timing-regression` in `kernel-boot-x86_64`, refusing three
unbaselined spoored metrics. That is **`LE-23`**, and it is not a defect —
it is an owner decision that **five** independent sessions have now declined,
each for the same correct reason: `--update-baseline` rewrites the whole file,
replacing CI-runner rows with Windows-host rows and re-creating the exact
cross-host offset the row exists to record.

**Do not "fix" it.** But do not let it stay ambient either: `main` cannot go
green until it is decided, and a permanently-red `main` is how a real red stops
being read. If the owner is available, this is the question to put to them —
it is one decision and it unblocks the repository's headline signal.

**Red for any other reason is new and is yours**, and that sentence means more
now than it did last week. See the next section for why.

## The new thing: a test can now fail where you cannot see it

Until 2026-08-06 every `#[test]` in this repository gated a Windows laptop and
nothing else. It now gates a Linux runner too, and `01B` spent the whole session
on what that exposed. **Two defects, neither visible to any gate on this bench,
both from the same mistake:** `not(target_os = "windows")` used as a synonym for
*bare metal*. A Linux host satisfies it exactly as `x86_64-tinyos` does.

- `hal_x86_64::boot`'s `_start` linked into every `std` test harness →
  `duplicate symbol`, four crates, **no test in the workspace ran**.
- `exec::address_space::unmap_page` executed `invlpg` — a **ring-0**
  instruction — in a userspace test process → `SIGSEGV` after five `ok` lines,
  which reads like a flaky harness rather than a defect.

Both are fixed. **The class is not.** The same spelling still stands throughout
`hal-x86_64` (`gdt`, `paging`, `pci`, `fault`, `interrupts`, `qemu_exit`,
`serial`) and in `kernel::context::switch_address_space`. If you touch any of
them, assume a Linux host compiles and *runs* what you write.

They cannot simply be converted, and the obstacle is worth knowing before you
try: `kernel` and `exec` ship `no_main` fixture `[[bin]]`s referencing those
items **ungated**, and the Linux governance job compiles those bins for the
host. Tightening the gate reddens a green job with `E0432`/`E0433`. **The fix
that ends the class is a change to how those bins are built** — that is design
surface, the 2026-07-30 sprint rule has not been lifted, and it is a proposal
to put to the owner rather than a change to make.

## Your task, in order

1. ~~**`EPIC-P1` is missing `FEAT-P1-11`.**~~ **DONE — and not by the session
   that wrote this line.** It was true when written: the Features table jumped
   `FEAT-P1-10` straight to `FEAT-P1-12` and the `Status:` header stopped at
   `FEAT-P1-10`. A **concurrent session repaired both** while `01D` was in
   flight, and `01D`'s `git add -A` swept the repair into commit `231a6db`
   under an unrelated message. Verified present in both places at `398cff1`.
   **Two lessons, and the second is the one that costs.** A cover note's task
   list can be actioned by somebody else between writing and reading, so
   re-check before starting. And `git add -A` in a tree with concurrent
   sessions commits work you did not do and did not read — `git status` before
   staging, every time.
2. **`LE-98`'s remaining half** — the device-tree parse that makes
   `SIMPLEFB_BASE` evidence rather than folklore, and removes the fault path's
   named exception with it. Safety precedes correctness: this is a 4 MB write
   to a fixed physical address on a no-IOMMU machine.
3. **The board checklist**, [`10C`](../hand-2026-08-06/10C-the-work-is-on-main-and-the-runner-is-in-an-outage.md)
   §5 item 4 — a checklist rather than a discovery, and it needs a hand on a
   mains plug that nobody has yet.

## Six traps, current

- **A source-level scan matches its own text.** Now **three** instances, and
  the third was written by an author who had just read a warning about the
  first two: `01B`'s ring-0 guard failed on its own needle list, because the
  four string literals it searches for *are* lines containing `write_cr3(`. It
  now stops at `#[cfg(test)]` **and asserts that it stopped**. If you write a
  scan, run it before you believe it.
- **A gate that would not have caught the defect it was written for has not
  been checked against its own instance.** Three consecutive sessions.
  Mutate the **real file** and read **which** error came back — a fixture
  contains only what its author already thought of.
- **`cargo clippy --workspace --all-targets` is NOT a local gate on this
  bench — but a per-package one is, and you want it.** The workspace form is
  red on the pristine tree with seven `E0432`/`E0433`s, for exactly the cfg
  reason above, so a session that reaches for it misreads the result **in both
  directions**: red on code it did not break, and silent about the two defects
  `01B` found. `check-guest-images` and `check-boot-images` are the local gates
  for board code, as `CLAUDE.md` says. **The local lint gate is
  `cargo run -p xtask -- check-lints`** — host clippy per package, so one
  crate's failure cannot hide the next crate's, and for `xtask` it runs exactly
  the `-D warnings` command CI runs. `01D` learned this the expensive way
  twice over: an `xtask` change that passed `cargo test --workspace` reddened
  the governance job on a `duplicated attribute` — a *warning* locally, an
  *error* under `-D warnings` — and then `01D`'s first correction to this very
  entry taught the raw `cargo clippy -p xtask ...` invocation, **not knowing
  the subcommand already existed**. Teaching the raw command teaches *around*
  the gate. `check-lints` is filed as a mechanism and is wired into neither
  `ci.yml`, the pre-commit hook, nor `CI_ENFORCED` — so `check-ci-gates`,
  which exists to refuse exactly that, is blind to it (`LE-106`).
  **`cargo test` will not save you from a lint.**
- **`check-boot-images` and `check-guest-images` are siblings** and running one
  is not running the other (`LE-72`, `LE-92`). Touching `kernel`, `hal-arm64`,
  `pi5-image`, `exec` or `shell` means both.
- **`origin/main` and `HEAD` are not the same thing.** `01B`'s entire first
  half existed because a handover asserted a push it had not made, and four
  sessions inherited the claim. `git ls-remote origin main` is one command and
  it is the only one that answers.
- **A concurrent session can commit mid-turn.** A `git diff` that empties
  between two tool calls is a live session, not a bug.

## An instrument rule this project keeps re-learning

`LE-80`'s family has now appeared four times in five sessions. `01A` stated the
rule as *check that your instrument can return both answers before you believe
the one it gave you*. `01B` had to sharpen it again:

> An API that takes an identifier will accept an identifier it can never match.
> `actions/runs?head_sha=` works — with a **full 40-character SHA**. Given an
> abbreviated one it answers `total_count: 0`, correctly, about nothing.

Both halves are load-bearing. Prefer listing and matching client-side.

## Do not start

`FEAT-P1-12` — the RT reserve. It has a name now, and that is the point: four
handovers called it *"`FEAT-P1-05`'s RT reserve"*, and the ambiguity is what let
it get started while listed as do-not-start. Also `G09`/`LE-86` and `06A` §4.3.
**Do not add design surface** — the hardware-evidence sprint rule from
2026-07-30 has not been lifted, and that includes the bin-build change named
above however tempting it looks.

## The standing instructions

Stated in full at the end of
[`10C`](../hand-2026-08-06/10C-the-work-is-on-main-and-the-runner-is-in-an-outage.md),
with `12D` §8's tenth. The three most likely to save you an hour here:

- **A gate is only as strong as the weakest place it is actually executed.**
  `LE-100` closed on that sentence. The runner has now executed it, twice
  refusing to, and both refusals were real defects.
- **Check what the code does on the machine before writing down what it does.**
- **Build the unblocker rather than the next blocked artifact, and say so.**
