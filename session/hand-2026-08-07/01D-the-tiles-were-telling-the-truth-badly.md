# 01D — The tiles were telling the truth badly, and the 125 are 14

Executes an owner question against the assurance release-status tiles: *what
can be done to fix the situation these numbers paint?* Four items were agreed
and all four are actioned here, though two of them end in a decision that is
not this session's to take, and that is stated rather than smoothed over.

## 1. Two of the five tiles meant something other than what they looked like

The row published `0 / 100` Stories assurance-verified, `25 / 460` release
gates with dated evidence, and `414 / 460` reachable with no board. Read
together those say *"almost nothing is done and the board is why."* Neither
half of that is true.

**`0 / 100` is a locked door, not a backlog.** Every in-play domain carries one
`G04` bound-class gate; every Story selects at least one domain; `ADR 0005`
refuses a `G04` row from any platform without a secure-world qualification
record; and `qualified-platforms.tsv` holds five platforms with **zero**
qualified. So no Story in this project can reach assurance `verified` — not
after any amount of engineering. `LE-94` already recorded this and asked, in
its own words, that *"the register should say the ceiling is a lock rather than
a backlog."* It did not say it. It does now.

**`25 / 460` overstates the work remaining and understates the indictment**,
which is `release_status.rs`'s own sentence about itself. Half the denominator
cannot be closed by construction. The board is not the constraint and never
was: of the gates in domains that actually exist, the number only a board could
move is **zero**, derived rather than asserted.

Three tiles now ship beside the originals, all from the same `decompose()` call
`xtask assurance-status` makes, so the dashboard and the subcommand cannot
disagree:

| tile | reads | why it is there |
| --- | --- | --- |
| Platforms qualified | `0 / 5` | names `ADR 0005` and that it bars every `G04`; explains the tile above it |
| Evidence against the closable denominator | `25 / 220` | the denominator that can actually be closed |
| Unmeasured and measurable today | `125` | no board, no decision, no absent mechanism |

`LE-94` is **partly actioned and stays open** — the register now says the
ceiling is a lock, but the owner decision and the not-applicable design
question are both untouched, and those are the substance.

## 2. The 125 are not 125 jobs — they are 14 measurements

`assurance-status` published `125` for four handovers and nobody could act on
it, because a count names no gate. It now decomposes two ways.

**By domain**, nearest to complete first, naming guardrails rather than
counting them. **By guardrail** — and this is where the leverage turns out to
be:

```text
THE SAME 125, BY GUARDRAIL — 14 distinct measurements, not 125 jobs.
  guardrail  domains  owed by
  G05             10  D01 D03 D04 D05 D06 D07 D08 D09 D11 D24
  G06             10  D01 D03 D04 D05 D06 D07 D08 D09 D11 D24
  ... G07 G08 G09 G10 G12 G17 G18 all 10 ...
  G20              9  D01 D03 D04 D05 D06 D07 D08 D11 D24
  G02              7  D01 D03 D06 D08 D09 D11 D24
  G23              7  D01 D03 D06 D08 D09 D11 D24
  G01              6  D01 D03 D06 D08 D09 D24
  G03              6  D01 D03 D06 D08 D09 D24
```

The ten implemented domains owe **the same guardrails**. Nine of the fourteen
are owed by every one of them, so a single harness arm moves ten gates rather
than one. That is a different piece of work from what "125 gates" describes,
and reading the by-domain table alone hides it completely — which is why both
views ship.

`9×10 + 9 + 2×7 + 2×6 = 125`. It reconciles, and
`the_per_domain_worklist_reconciles_with_the_totals` asserts that it does at
every level. That test exists because handover `09A` computed this ledger by
hand and **its first printing did not reconcile** — 164 where the answer is
220. It also asserts the rows name at least one guardrail, because every sum in
it is satisfied by a worklist that names nothing.

**What was not done: none of the 125 was measured.** Filing a guardrail-evidence
row requires a dated Report with raw evidence, and manufacturing 125 of those is
the precise failure this repository exists to prevent. What is delivered is the
work made ordinary rather than the work done.

### 2b. Except that some of the 125 are not unmeasured — they are unread (`LE-104`)

Mapping the worklist onto the harness that already exists turned up the
cheapest finding in the register, and it is not a measurement task at all:

```text
goals/reports/_measure-p1-01-01/pool-bench/measure-run-{1,2,3}.log
  size_of::<Pool<u64,64>>()[G10] = 1024 bytes fits_8kib_budget=true
  pool-bench denial[exhausted][G20]
  pool-bench denial[invalid_handle][G20]
```

Three committed captures, **the fixture labelling its own output with the
guardrail it answers**, and not one row in `guardrail-evidence.tsv` cites any of
them. Two more of the same shape without the self-labelling:
`REPORT-2026-07-29-02` carries `D03`'s `decision_to_actuation_emit` p50/p99 and
says in its own text that it closes no guardrail; `REPORT-2026-07-28-09` carries
`D09`'s `pe_parse_blue_sharc_accept` p50 = 1,952.7 µs, p99 = 3,808.1 µs at
n = 600 and says the same. In both the author correctly declined to file, and
nobody afterwards read the number against the target.

**This project has been counting a gate as unmeasured when what it is, is
unread.** So `125` over-counts the remaining measurement work by an unknown
amount, and the cheapest closable gates in the register are ones where the
measuring already happened.

Filed as `LE-104` rather than actioned, because reading a number against a
target is a judgement belonging to whoever owns the Story. **Two precisions are
recorded there and matter more than the finding**, because the temptation is to
file rows quickly and this row would then cause the damage it warns about:

- `G10` and `G20` are self-labelled. **`G12` is not** — it was proposed because
  pool-bench measures best, middle and last free slot plus exhaustion and
  recovery separately, which is `G12`'s stated method verbatim. The match is an
  inference, probably right, and not the same kind of fact.
- The proposal that `G06`/`G07` come free wherever `G01`/`G02` is filed —
  because the same envelope already reports raw cycles — **holds for `G06` and
  does not hold for `G07`.** `G06`'s method is *"invariant TSC or architectural
  counter with measurement overhead subtracted"*, which the Tier 0 envelope
  does exactly. `G07`'s is *"collect **PMU** cycles"*, and Tier 0 x86_64 reads
  `rdtsc` — the TSC, not the PMU. Filing `G07` from an `rdtsc` sample is
  quoting one instrument as another, which is `LE-33`'s shape and precisely
  what `bound_provenance.rs` exists to refuse for `G04`.

One question is raised and deliberately left open: the same log carries
`exhaustion_drain_cycles[G21]`, and `G21` is on the mechanism-absent list that
§3 re-verified as still correct the same day. Both can be true — a pool
demonstrating its own exhaustion is not a system containing one class's
exhaustion from another — but that list is a per-*domain* blanket and `D07` is
where it is most contestable. It is the second concrete argument for the
per-guardrail readiness column `LE-84` already asks be considered.

## 3. The seven mechanism-absent guardrails re-verified, and one overstatement corrected

`MECHANISM_ABSENT_GUARDRAILS` is a declared judgement, not a derived fact, and
it had not been re-read against the tree since it was written. All seven —
`G13`–`G16`, `G19`, `G21`, `G22` — are **still correctly listed**: no queue
with serviced residence, no offered-load or backpressure harness, no soak
runner, and `Tcb` still carries no containment class with the pool still one
flat capacity and no reservation floor.

One precision was owed and is now in the source. The doc comment's unqualified
*"unbuilt containment mechanism"* overstates, and an overstatement there is the
kind that gets quoted: `kernel::fault` **does** contain a fault to the task
that raised it — three real faults, each contained, scheduler still dispatching
afterwards, gated in CI by `--fixture=fault`. What `G19`/`G21` require and this
project lacks is the *class* and the *reservation floor*. Single-task fault
containment is real; per-class resource containment is not.

## 4. The qualification decision is one decision, and the instrument for it is blind

The fourth item was to close the gap so the owner faces a decision rather than
an investigation. Auditing what the project already holds against `ADR 0005`'s
`Q1`–`Q4`:

- **`Q1` largely held.** SoC and board revision, bootloader EEPROM version and
  date, `config.txt`'s `os_check=0`, and GICv2 GIC-400 with 4 implemented
  priority bits confirmed on silicon. **Two gaps:** the exception level TinyOS
  is entered at has never been captured — the decode and wire text are
  host-tested against a double and the register read needs one boot — and the
  secure-side GIC configuration is absent. One wording gap: the ADR names
  `start*.elf`/`bootcode` and the Pi 5 uses SPI-EEPROM firmware, and nothing
  records that the substitution satisfies the clause.
- **`Q2` is a zero, not a thin holding** — no vendor documentation consulted,
  no firmware source examined, and the ADR's mandated sentence for the
  closed-firmware case never written. It is therefore the **largest** gap and
  simultaneously the **cheapest to start**: pure laptop and documentation work,
  and the ADR explicitly accepts *"the firmware is closed and this cannot be
  determined"* provided the record says so in those words.
- **`Q4` is already written**, as consistent practice in near-record-ready
  language across `REPORT-2026-08-04-01`, the board-session runbook and
  `STORY-P1-07-06`. It needs transcription, not authorship.
- **`Q3` is the finding, and it is filed as `LE-103`.**

### `LE-103` — the Q3 instrument reads the one counter that can be made to lie

`ADR 0005` defines the residency campaign as divergence between the **physical**
counter `CNTPCT_EL0` and work `NS-EL1` can account for. The only thing in the
tree resembling that campaign is `hal_arm64::timer::probe_pmccntr`, and its
window comes from `SystemRegisters::count()` — which is **`CNTVCT_EL0`, the
virtual counter** (`mrs {value}, cntvct_el0`; the module doc says so).

`CNTVCT_EL0` is architecturally `CNTPCT_EL0 - CNTVOFF_EL2`. A world above
`NS-EL1` that wanted to hide residency would move `CNTVOFF`, and the virtual
counter would then report that no time had passed while the physical counter
reported that it had. **The instrument is blind by construction to the single
phenomenon the campaign exists to detect.**

This is not a defect in `probe_pmccntr`: it was built for `LE-15`, the
counter-selection question, where the virtual counter is the right read, and it
answered correctly and closed that row on silicon. The debt is that it *looks
like* a `Q3` campaign — right register pair in the name, already board-proven —
so the next session would reasonably conclude `Q3` was nearly done. `LE-94`'s
own owner path makes exactly that assumption, and it is corrected there.

`LE-80`'s family a fifth time in six sessions: a tool that answers confidently
and cannot distinguish the two answers that matter.

**Deliberately not fixed here.** A `CNTPCT_EL0` read is a three-line sibling of
the one already in `timer.rs`, but an instrument that has never run on the board
is not evidence, this bench has no board, and adding an unrun probe would put a
*second* confident-looking `Q3` instrument in the tree beside the first.

## What the owner is actually being asked

`Q2` is a laptop afternoon. `Q1` needs one boot for `CurrentEL`. `Q3` needs
`LE-103` fixed and then a campaign with stated duration, sample count, largest
excursion, distribution shape, recorded environmental conditions and the
injected-perturbation positive control `ADR 0005` makes mandatory — gated on
`LE-95`, the power relay nobody has bought. `Q4` is written.

So `LE-94`'s pending decision is **not blocked on research**. It is one
decision, and the thing standing behind it is a £15 relay and an afternoon.

## What this session did not do

None of the 125 measured. No Story moved. Of `01C`'s task list, `LE-98`'s
device-tree half and the board checklist are untouched. The do-not-start list
was honoured and no design surface was added.

**One correction, and it is against this session rather than an inherited
claim.** `01C` item 1 said `EPIC-P1` was missing its `FEAT-P1-11` row in two
places. That was true when written and is false now: a **concurrent session
repaired both the Features table and the `Status:` header** while this session
was mid-flight, and this session's `git add -A` swept that repair into commit
`231a6db` under a message about the dashboard, with no mention of it. Nobody
was misled for long, but two things follow. A cover note's task list can be
actioned by someone else between writing and reading. And **`git add -A` in a
tree with concurrent sessions commits work you did not do and did not read** —
`01C`'s own trap list warns that a session can commit mid-turn, and the warning
did not stop this because the trap is not the concurrent commit, it is the
blind stage. `git status` before staging.

## Postscript: this session reddened the governance job, and the trap was its own

Run `31167021880` failed on `Format, lint, size, assurance, missing_docs`:

```text
error: duplicated attribute
  --> src/xtask/src/assurance/release_status.rs:515:5
```

A doc comment was inserted between an existing `#[test]` and its function,
orphaning the attribute onto the next one. `cargo test --workspace` compiles
that happily — it is a **warning** — and CI's `clippy -D warnings` makes it an
**error**. Every gate this session ran was green.

That is `01C`'s own clippy trap firing on the session that wrote it. **And the
first correction was itself wrong in the more interesting way:** it presented
`cargo clippy -p xtask --all-targets -- -D warnings` as a discovery, when
`cargo run -p xtask -- check-lints` **already existed** and runs exactly that —
host clippy per package, so one crate's failure cannot hide the next crate's.

So the gate was never missing. It is absent from `ci.yml`, from the pre-commit
hook and from `CI_ENFORCED`, which means `check-ci-gates` — the thing built to
refuse a workflow where a filed mechanism is not wired in — **is blind to it**.
That is `LE-100`'s own thesis recurring on a different gate days after `LE-100`
closed, and it is filed as `LE-106`.

The lesson is sharper than the one I first wrote down: a trap entry that
teaches a raw `cargo` invocation teaches *around* the project's own gate, and
the next session then re-derives by hand a subcommand that was already there.
**Before recording a command as the way to check something, run
`xtask help`.** `01C`'s entry now names `check-lints`.

---

**Written 2026-08-07.** Loose ends: `LE-103` and `LE-104` raised, `LE-94`
partly actioned and still open. Gates: `cargo fmt --all --check`, `cargo test --workspace`,
`cargo clippy -p xtask --all-targets -- -D warnings` (added after it caught
what the others missed), `check-assurance-spine`, `check-ci-gates`,
`check-metric-labels`. No board crate touched, so
`check-boot-images`/`check-guest-images` do not apply — and that is a
judgement, not an omission: the change is confined to `xtask` and `goals/`.
