# 13D — Cover Note for the Next Session

Short by design. [`12D`](12D-the-domain-label-is-machine-checked-now.md) is the
full handover and this does not repeat it — this is the orientation a fresh
agent needs in its first five minutes, and the traps this tree sets *now*, which
are not the same four [`11C`](11C-cover-note-for-the-next-session.md) listed.

## Where the project actually is

**Nothing is committed.** That is the single most important sentence here.
`HEAD` is still `4f5f2a4`, and the working tree carries **two sessions' work
interleaved** — `12D`'s `LE-91` closure and a concurrent session's `FEAT-P1-12`
and `ADR 0015`. Both are complete, both are green, neither is on `main`.

The combined tree passes everything: 16 workspace suites, spine (32 Features /
100 Stories / 4050 selected contracts / 101 loose ends, 49 open), `fmt`,
`check-metric-labels`, `check-citations`, `check-lints`, `check-boot-images`,
`check-guest-images`, `check-spine-files`, `check-crate-sizes`,
`check-performance-catalogue`. `check-timing-regression` is untouched and still
`LE-23`.

The Pi 5 is powered and beaconing, untouched since the 2026-08-06 boot.
**There is no plug on the desk**, so nothing below needs one.

## Read these, then start

1. [`agent/CONCURRENT_SESSIONS.md`](../../agent/CONCURRENT_SESSIONS.md) — rules 1
   and 8, and read them **before** you touch git, not after. See §"Your first
   act".
2. [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md) — binding.
3. [`12D`](12D-the-domain-label-is-machine-checked-now.md) §2 and §4 — the two
   things that changed what this project can trust about its own gates.

## Your first act: commit, and it is not routine

Five files carry **both** sessions' changes: `os/src/xtask/src/main.rs`,
`os/src/xtask/src/assurance/spine_tests.rs`,
`goals/assurance/story-contracts.tsv`,
`goals/assurance/guardrail-evidence.tsv`, `goals/index.html`.

`git add -A` is banned and **path-level staging is file-level staging** — the
whole point of rule 1's second half. The two bodies of work are coherent
together and were verified that way, so the sane move is one commit carrying
both, with a message that says plainly which parts each session authored and
reviewed (rule 3: authorship asserts review; where that would be false, say so).
Read `git diff --cached` **before** committing, not after.

The pre-commit hook now runs `check-metric-labels` as well. Install it if you
have not: `git config core.hooksPath .githooks`.

## Your task: `LE-100`

**CI runs no tests. At all.** `.github/workflows/ci.yml` has four jobs and not
one `cargo test` step. `clippy --all-targets` *compiles* the harnesses, which is
exactly why nobody noticed: a broken test fails the build, a **failing** test
does not.

Roughly 1210 host tests pass locally, and among them is **every source-level
guard this project has ever filed as the closure of a loose end** — `LE-99`'s
stamp density, `LE-97`'s cadence, the `G23` pair equivalence, `check-citations`'
own tree assertion. Each was written as *the mechanism that stops this
recurring*. Each is invisible to the runner.

This is `LE-72` and `LE-92` a third time: a gate that exists but is not executed
where it counts. It is worth doing first because it changes what every other
closed row in the register means.

**What the row says is buildable.** A host test job in `ci.yml` — **not** a line
appended to `governance-gates`. It needs the same pinned toolchain and it belongs
beside the QEMU jobs, because the fast deterministic gates exist to fail in
seconds and should not queue behind a full suite. Decide deliberately whether
day-one failure is blocking (it should be), and expect the first run to surface
tests that pass on Windows and fail on Linux — that is `LE-64`'s family, and it
must not be met mid-merge.

Until it lands, **treat every `#[test]`-only closure in `loose-ends.tsv` as
locally enforced only**, and prefer an `xtask` subcommand wired into `ci.yml` for
anything filed as a mechanism. That is what `check-metric-labels` does.

## Four traps this tree has sprung, updated

- **A gate that would not have caught the defect it was written for has not been
  checked against its own instance.** `LE-91`'s prescription was built exactly as
  the row specified, and the original defect passed straight through it. The
  mutation that mattered was not one that made the gate fire — it was the one
  that made it stay **silent**, run against the exact case the row was filed for.
  Do this for every guard you write.
- **A source-level test matches its own assertions.** Still true, and it bit
  again: `metric_labels.rs` failed on its own doc comment describing
  `MetricLabel { .. }`, and again on its own error-message string literal.
  Comments are now stripped and the file is self-exempt with a one-entry list a
  test pins at one.
- **`check-boot-images` and `check-guest-images` are siblings and running one is
  not running the other** (`LE-72`, `LE-92`). Unchanged.
- **A worktree is no longer a trap, and this is new.** `LE-101` is closed:
  `.gitattributes` covered only `*.golden.txt`, so a `git worktree add` on this
  host produced CRLF `.rs` files and `board_dispatch`'s `include_str!` guard went
  red on code the session never touched — in exactly the workflow rule 8
  *mandates*. `*.rs -text` fixes it. If you see a source-level guard fail in a
  fresh checkout, check line endings before you believe it.

## CI, unchanged from `11C` and still not yours

No run exists for `b4a7010`/`cb9b27b`; Actions was in a `major_outage`. `10C` §3
lists six ruled-out candidates so you do not re-check them. **Re-run
`gh run list` once Actions recovers.** When it does run, expect red naming three
unbaselined metrics — that is `LE-23`, declined by four independent sessions, and
`--update-baseline` is *not* the fix: it rewrites the whole file, replacing
CI-runner rows with Windows-host rows and re-creating the exact offset the row
exists to record.

**Red for any other reason is new and is yours.**

## Do not start

`FEAT-P1-12` — the RT reserve. **It has a name now**, and that is the point: four
handovers called it "`FEAT-P1-05`'s RT reserve", and the ambiguity is what let it
get started while listed as do-not-start. Also `G09`/`LE-86` and `06A` §4.3.
**Do not add design surface** — the hardware-evidence sprint rule from
2026-07-30 has not been lifted.

## The standing instructions

Ten now, stated in full at the end of
[`10C`](10C-the-work-is-on-main-and-the-runner-is-in-an-outage.md) and `12D` §8.
The three most likely to save you an hour:

- **A gate is only as strong as the weakest place it is actually executed.**
  This is `LE-100`'s whole subject; you are about to close it.
- **Build the unblocker rather than the next blocked artifact, and say so.**
- **Check what the code does on the machine before writing down what it does.**

## If you finish early

In order: **`EPIC-P1`'s Features table is missing its `FEAT-P1-11` row**
(pre-existing drift, found while `FEAT-P1-12` was added; the owner has approved
adding it — summarise `FEAT-P1-11` as board-proven but not Complete and name
what it waits on). Then `LE-98`'s remaining half — the device-tree parse that
makes `SIMPLEFB_BASE` evidence rather than folklore, and removes the fault path's
named exception with it. Then the board checklist in `10C` §5 item 4, which
needs a hand on a mains plug that nobody has yet.
