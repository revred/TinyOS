# Handover 09D — `STORY-P1-06-01` Delivered: the Decision-to-Actuation Path

**Session `D`.** Follows and executes [`05B`](05B-next-session-agenda.md), the work order. Concurrent
sessions `A` (Tauri/tab host: `02A`, `03A`, `06A`, `07A`) and `C` (`08C`) were live throughout; neither
was touched, and §6 below says exactly where this session's commits met theirs.

`main` is at **`3c617bf`**, **pushed**, CI **green**.

## 1. What landed

| # | Item | State |
|---|---|---|
| **W1** | **`STORY-P1-06-01`** — the bounded decision-to-actuation path | **Verified (Tier 0, mechanism half)**, `45e99d8` |
| — | rustfmt across the workspace; CI was red on format | `fbe080f` |
| **W2** | **`LE-23`** — the baseline recorder | **Mechanism landed** (`3c617bf`); **the recording itself is blocked — see §3** |
| **W3** | **`LE-46`** — soak sweep under `--serial-capture` | **Not actionable from this repository — see §4** |

`05B` §6 asked for the push, and it happened: `origin` was four commits behind and is now level at
`3c617bf`. That is what made W2 askable at all, and W2's answer arrived within the hour.

## 2. W1 — what is claimed, and the two halves that are not

`FEAT-P1-06`'s exit criteria have three halves gated by three different things. **The mechanism half is
taken; the other two are refused explicitly**, which is the shape `05B` §3 recommended and the evidence
supports.

- **Taken.** The path exists (decision → command → a real ISA port write standing in for an actuator
  line), the budget *and* the deadline are declared, the deadline monitor enforces the deadline, the
  WCET enforcement trips the declared policy, and the distribution is measured and recorded with its
  provenance (`tier=T0`, `qualification=none`).
- **Not taken — the bound.** No `PERF-D03-G04`/`PERF-D05-G04` row, no `guardrail-evidence.tsv` row at
  all. Under `ADR 0005` a worst-case bound is quotable only from a qualified platform and there are
  none. **The bound is stated debt against `LE-09`, in those words**, exactly as `05B` §3 instructed.
- **Not taken — hostile load.** `FEAT-P1-05` has no Story started. `FEAT-P1-06`'s status now says
  **In progress — the Feature is NOT complete**, rather than leaving a reader to infer it from a Story
  table.

**`PERF-D03-G20` was measured and still not filed**, and that decision is worth carrying forward
because it is the interesting one. The denial path is exactly `G20`'s shape — a denial that must be
cheap and change nothing — and the *state* half holds outright: 1,100 refusals, zero writes to the
line. The *latency* half does not survive its own variance: run-to-run p99 CV of **55%**. Measuring
something is not the same as being able to stand behind a number for it, and a register row is a claim
about a threshold. `PERF-D09-G20`'s Tier 0 filing had a 2.8× margin and a stable measurement; this has
neither.

**The deadline is now a real quantity.** `STORY-P1-04-02` scoped it out honestly and `FEAT-P1-04`'s
title has named it as missing ever since. A budget counts ticks *attributed* to a task; a deadline
counts ticks *elapsed* since the activation was armed. A task that is preempted and starved meets its
budget perfectly and misses its deadline badly — pinned by a host test rather than left as an argument.

### The four defects the falsifications found, all in the instruments

This is the part worth reading. **Every one of them would have shipped a green run that proved less
than it appeared to**, and three were caught only because `TEST-P1-06-01-A` demanded a check before the
code existed.

1. **The authority falsification passed.** With the owner check removed from `ActuationPort::emit`, the
   fixture still reported `ok=true` — the intruder was `Ready`, not `Running`, so the port's *separate*
   running check refused it. The fixture was exercising the wrong check and reporting it as evidence
   for authority. The intruder is now placed in `Running` so identity is the only difference; the
   falsification then fails 3/3 with 1,100 unauthorized commands reaching the line. **The host tests
   were never fooled** — four failed immediately. The Tier 0 fixture was the weaker instrument, which
   is not the direction anyone expects.
2. **`preemptions=1311424` in a run that saw ten ticks**, and the same value as `overhead_cycles` in
   another. An 8 KiB task stack overflowed into `.bss` and spilled cycle deltas landed in whichever
   static lay below. **The symptom was plausible numbers in the artifact, not a fault.** Stacks are now
   32 KiB and carry a canary checked *before* any number is read as evidence.
3. **The prevention falsification proved nothing, twice.** First the overrun run could not detect an
   emitted late command at all (the trip removes the offender before it reaches any emit) — hence the
   late-emit probe. Then a faster tick collapsed the window between the missed deadline and the trip,
   and the probe fired in **neither** build, so both "passed" identically. Budget is now 12 ticks
   against a deadline of 2.
4. **At the inherited APIC reload the release build proved nothing at all**: it finished all 2,200
   iterations before the first tick, so `ticks=0` and the monitor was never exercised. Clause 1's own
   `ticks >= 1` check failed the run rather than reporting the percentiles.

The general lesson, and it is not this Story's alone: **the fixtures were wrong four times and the
kernel was wrong zero times.** An instrument that has never been demonstrated to fail is not evidence,
and "run the falsification" is not the same as "run the falsification and check it failed *for the
reason you predicted*".

## 3. W2 — `LE-23` is blocked on `LE-24`, which does not come free

`05B` listed `LE-24` as something that "may come free" behind `LE-23`. **It does not. It is the
blocker.**

The recorder job (`3c617bf`, `workflow_dispatch` only) ran on the Linux runner and did exactly what it
should: it measured, and then **refused to write**.

```text
xtask: refusing to write a baseline this parser rejects:
  baseline `D07/pool_u64x4_alloc_denied_exhausted` carries a zero ratio;
  nothing can be compared against it
```

The cause is arithmetic, not flakiness. That run reported `overhead_cycles=26`, and every percentile of
the denial metric is **also 26** — so after the harness subtracts its own calibrated read-pair cost the
metric's *minimum* is **0**, its `min_ratio_ppm` is 0, and `gate.rs` correctly refuses a baseline row
nothing can ever be compared against.

**This corrects `LE-24`'s own recorded belief.** That row says the metric "medians to 0 cycles on the
Windows dev host … CI run 30294647525 measured it at 25 cycles, so on the Linux runner it is measurable
and would be gateable". On this runner it is **not** gateable: the operation costs about one calibrated
read pair, so the honest corrected value is zero wherever the harness is calibrated at all. The
host-specific framing was too optimistic. The row is left unedited on purpose — `loose-ends.tsv` has a
concurrent session's uncommitted append in it, and rule 8's sibling says not to write a file another
session is mid-edit in.

`LE-24`'s own recorded remedy is still the right one and is now a prerequisite rather than a
nice-to-have: **time N denials and divide**, so the measured region is large against the calibration
and the result is host-independent. That is a change to `fixture_measure::phase_pool_denial`, it
redefines a metric every committed baseline carries, and it therefore wants doing in the *same* commit
as the re-record — which is convenient, because the re-record is what it unblocks.

**Sequence for the next session:** fix `LE-24` → re-run `gh workflow run CI --ref main` → download the
`tier0-x86_64-baseline` artifact → commit it → confirm the gate passes on the next push. `LE-42` (the
`D09` accept path at 17.6–39.1× budget) waits behind that, unchanged.

**Do not run `--update-baseline` locally** (`LE-28`). The recorder job exists so that nobody has to.

## 4. W3 — `LE-46` cannot be closed from inside this repository

The soak sweep is **not in this repository**. `goals/reports/_soak-p0-03-01.log` is appended by an
external process started outside the workflow (`REPORT-2026-07-27-01` says so), and there is no script,
no `xtask` subcommand and no CI job that runs it. `LE-46` asks for that sweep to pass
`--serial-capture`, and the file to change is one this session cannot see.

Two things follow, and the second is the useful one:

- The row should say where the sweep lives, or `LE-46` is unactionable for every session that inherits
  it. It cost this session a real search to establish that the answer is "nowhere in the tree".
- **The better fix may not need the script at all.** `xtask qemu-x86_64` could capture serial
  unconditionally and print it on an unexpected exit code, instead of capturing only when a caller
  remembers `--serial-capture`. That closes `LE-46`'s actual sentence — *"at minimum retain a capture
  for any non-zero exit"* — for **every** caller including the external sweep, with no coordination.
  Recommended, and deliberately not done here: it changes the behaviour of a harness every fixture step
  in CI depends on, on a day this session had already changed the fixture table.

The current soak is ~66 h into 72 h and its last checkpoint is clean. **Nothing in this session touched
it**, and `PERF-D07-G22` still will not close on this run by owner decision (`LE-45`).

## 5. Owed register rows — now three

`05B` §4 owed two the moment the concurrent session's `LE-53` lands. **There is a third**, and this
session hit the same wall from a new direction:

- **The actuation path is proven in a fixture, not on the shipping `os` image** — the same shape
  `LE-20` had for WCET enforcement. Named in `STORY-P1-06-01` and `REPORT-2026-07-29-02` as unregistered
  debt, in prose, because id contiguity is enforced and the next id is held in another session's
  uncommitted row.

An earlier draft of the Report cited that unlanded id directly, and **the pre-commit gate refused the
commit** — correctly. That is the third independent arrival at coupling point 2 in two days, and the
strongest argument yet for the `xtask register-loose-end` that allocates and appends atomically.

## 6. The tree, and where this session met the others

Sessions `A` and `C` were live throughout. **Nothing of theirs was committed here**, and this session's
files were staged by path except one:

- **`goals/index.html` was staged by *content*, not by path** (`CONCURRENT_SESSIONS` rule 1). Session
  `A` has a pending edit to the *same paragraph* — the loose-end count — as this session's spine
  counts. The staged blob is `HEAD` + this session's four lines only; their number is still pending in
  the working tree and is still theirs to commit. `git diff --cached --numstat` was read before the
  commit, not after.
- `goals/assurance/story-contracts.tsv` is shared but nobody else had it open; staged by path.
- `check-spine-files` was run after **every** hand edit to a spine TSV, per rule 8.
- **Slot `08C` was taken mid-session by session `C`**, between this session deciding it needed a
  handover and creating the file. Claiming the slot first (rule 4) is what turned that into a rename of
  an empty file rather than a collision in the record.

**One thing to know if you inherit a red CI:** `os/src/exec/src/shared_memory.rs` had been unformatted
on `main` since `82e3d57`, and CI had not run since `49e5b08`, so the first push after it inherited a
red format job for a line nobody in that session wrote. It is fixed in `fbe080f`.

**And the gap that let it happen:** `.githooks/pre-commit` checks the assurance spine, the performance
catalogue and the crate-size ceiling against the staged tree — but **not `cargo fmt --check`**. That is
the one gate whose absence locally is *guaranteed* to be discovered remotely, and it is a two-line
addition to the hook. Worth doing before it costs someone else a red main.

## 7. Traps this session hit, for the next one

All ten in [`38A`](../hand-2026-07-28/38A-outstanding-actions.md) §6 still stand. These three are new
today:

1. **A falsification that fires is not a falsification that worked.** Check *which clause* failed and
   whether it is the one you predicted. Two of this session's four defects presented as a falsification
   apparently succeeding.
2. **A fixture stack is not a host stack.** Any fixture that runs the measurement harness from task
   context needs more than the customary 8 KiB, and the overflow presents as *plausible numbers*, not
   as a fault. Put a canary in and check it before reading any number.
3. **A fixture that passes in dev may prove nothing in release.** Anything timer-driven must assert that
   ticks actually arrived; the optimizer can otherwise finish the whole run inside one tick period.

## 8. Evidence

- `cargo test --workspace` — **634 passed, 0 failed** (621 before).
- `--fixture=actuation` — exit 0, `ok=true`, **10 consecutive runs**.
- `--fixture=actuation-overrun` — exit 1 (its correct outcome), `ok=true`, 3 consecutive runs.
- Both falsifications — 3/3 failing, each naming its own clause.
- `check-assurance-spine`, `check-spine-files`, `cargo fmt --all --check`, clippy on the real target for
  both fixture features — pass.
- **CI green on `fbe080f`** (run 30482980298), including both new steps on the Linux runner.
