# 11C — Cover Note for the Next Session

Short by design. [`10C`](10C-the-work-is-on-main-and-the-runner-is-in-an-outage.md)
is the full handover and this does not repeat it — this is the orientation a
fresh agent needs in its first five minutes, and the traps this particular tree
sets.

## Where the project actually is

The Pi 5 is **powered and beaconing** and has been untouched since the
2026-08-06 boot. That boot broke a three-session stall: fourteen metrics off the
wire, 89 spoor records, 0 lost, 0 refused, one continuous boot — and the two
gates it fed are now filed, so **release-gate evidence moved 23 → 25 of 460**,
the first movement in four sessions.

Everything through `10C` is committed and pushed (`cb9b27b`). The tree is clean.
**There is no plug on the desk**, so nothing you do this session powers the
board; every item below runs on a laptop.

## Read these, then start

`agent.md` lists seven documents and they all still apply. The three that decide
whether your first hour is useful:

1. [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md) — binding, not advisory.
2. [`agent/CONCURRENT_SESSIONS.md`](../../agent/CONCURRENT_SESSIONS.md) — short, and rules 1/2/3 bite immediately if you commit.
3. [`10C`](10C-the-work-is-on-main-and-the-runner-is-in-an-outage.md) §5 — the ordered next steps, and §3/§4 for why CI is red.

## Your task: `LE-91`

It is item 1 and it has been item 1 for two sessions with a stated reason, not
by neglect: its own filed estimate is **one session**, and it closes a class
rather than an instance.

**The defect.** Nothing machine-checks which performance domain a fixture metric
is labelled with. From 2026-08-05 to 2026-08-06 `fixture_measure_arm64` emitted
three spoor metrics as `domain=D07` because `STORY-P1-10-02`'s contract selected
only `D07`. `D07` is pool allocation; `D11` is spoor stamp and journal, which is
exactly and only what those three measure. The numbers were on the wire, quoted
in a Report, a handover and a Story's status header, and **never once compared to
`D11`'s targets** — which the stamp misses by 1.9× at the median. A wrong label
is not a naming defect; it is an unread gate.

**Why the obvious check is wrong.** "A fixture's domains must be a subset of its
owning Story's contract" would be false: one fixture serves six domains
(`REF`, `D02`, `D04`, `D05`, `D07`, `D11`) while `list-fixtures` maps a whole
fixture to **one** owning `TEST`. Do not assert that rule.

**What the row says is buildable.** Declare, per metric, its domain *and* its
owning Story at the `collect` site; have `xtask` parse the `collect` calls out of
the fixture sources so the declaration cannot drift from the code the way
`LE-80`'s mirror did; assert each metric's domain is selected by its named
Story's contract. That turns a bent label into a build failure.

`PERF-D11-G01`/`G02`/`G03` in `guardrail-evidence.tsv` are the worked example of
what the bent label cost. Read them before you design the check.

## Four traps this tree has already sprung

- **A source-level test matches its own assertions.** `include_str!` plus a
  literal needle finds the test, not the code. `hal_arm64::ethernet`'s cadence
  guard failed against itself twice, at two nesting depths. Every needle there is
  now `concat!`-assembled and the search is scoped past a banner. You will be
  writing exactly this kind of test for `LE-91`.
- **A mutation that fails for the wrong reason is a mutation that was not run.**
  Two of three falsifying mutations for `LE-99` died on lint config before the
  assertion executed. Check *which* error you got, not the exit code.
- **`check-boot-images` and `check-guest-images` are siblings and running one is
  not running the other.** Nothing else local compiles for AArch64 or for the
  x86_64 guest. Three pushes went out green locally and red on the runner
  (`LE-72`); a `E0308` in a Tier 0 fixture reached CI past a fully green local
  gate set (`LE-92`).
- **`git add -A` is banned and path-level staging is file-level staging.** The
  shared registers under `goals/assurance/` take *every* change in the file.
  Read `git diff --cached` before every commit, not after.

## CI is red, and you should not fix it

Two separate things, neither of them yours:

1. **No run exists** for `b4a7010` or `cb9b27b`. GitHub Actions was in a
   `major_outage` at the time of writing. Six configuration candidates and the
   `GITHUB_TOKEN`-actor hypothesis were all ruled out — `10C` §3 lists them so
   you do not re-check them. **Re-run `gh run list` once Actions recovers.**
2. **When it does run, expect red.** `check-timing-regression` will name **three**
   metrics measured with no baseline. That is `LE-23`, an owner decision declined
   by four independent sessions, and `--update-baseline` is *not* the fix: it
   rewrites the whole file, so running it on a laptop replaces CI-runner rows
   with Windows-host rows and re-creates the exact systematic offset `LE-23`
   exists to record.

**Red for any other reason is new and is yours.**

## Do not start

`FEAT-P1-05`'s RT reserve, `G09`/`LE-86`, `06A` §4.3. **Do not add design
surface** — the hardware-evidence sprint rule from 2026-07-30 has not been
lifted. If you find yourself designing rather than measuring or guarding, stop
and re-read this line.

## The standing instructions

Nine now, stated in full at the end of [`10C`](10C-the-work-is-on-main-and-the-runner-is-in-an-outage.md).
The three most likely to save you an hour:

- **`PERF-Dnn-Gnn` is only meaningful if `Dnn` is the domain of what you
  measured.** This is `LE-91`'s whole subject; you are about to mechanise it.
- **Build the unblocker rather than the next blocked artifact, and say so.**
- **Check what the code does on the machine before writing down what it does.**
  `08C` described a check as staying quiet on a cold run; the code disagreed, and
  thirty seconds of enumerating the actual network adapters settled it.

## If you finish early

In order: `LE-98`'s remaining half (the device-tree parse that makes
`SIMPLEFB_BASE` evidence rather than folklore, and removes the fault path's
named exception with it), then the board checklist in `10C` §5 item 4 — which is
a checklist rather than a discovery, and which needs a hand on a mains plug that
nobody has yet.
