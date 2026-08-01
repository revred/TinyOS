# Handover 02C — Next-Session Mandate: the Dashboard Moves Because the Register Moved

**The start-here document for the next working session.** Written 2026-08-01, after
[`02B`](02B-pushed-and-ci-fixed-le-64.md) left `main` pushed and CI green. The charge: make
[`goals/index.html`](../../goals/index.html) move — not by another hand-sync (this session did one,
and parts of it were stale the same day), but by moving the register beneath it and by finishing
the machine that `STORY-P0-01-08` started. Every step below is Host-tier assurance tooling of the
class `FEAT-P0-01` has absorbed five times already (`-01-04` … `-01-08`); nothing here touches the
hardware-evidence sprint's queue or opens design surface, so the 08A sprint rule is honoured, not
excepted.

## 0. State you inherit

- **`main` is pushed and green at `bf81dda`** — the first green CI since 2026-07-27. Two
  Windows-blind failures were fixed to get there (`LE-64`: a `#[path]` through a phantom
  directory; then a `cfg(not(windows))` clippy error), and one timing-gate flake was witnessed and
  re-run (`LE-18`'s class, recorded in `02B`). Two standing rules came out of it, and they bind
  this session: **watch every push's CI run to completion** (`gh run watch <id> --exit-status`),
  and before pushing code, **mirror CI's lint gate from this Windows host**:
  `cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu -- -D warnings` plus
  the AArch64 job (`cargo clippy -p hal-arm64 --target targets/aarch64-tinyos.json
  -Z build-std=core,compiler_builtins -Z build-std-features=compiler-builtins-mem
  -Z json-target-spec -- -D warnings`). Host-target clippy is structurally blind to every
  `cfg(not(windows))` line in this repository.
- **The dashboard was hand-refreshed 2026-08-01** (`fc04161`): freshness line, the four
  Overall-progress tiles, tabstrip counts, two UPDATE entries, backlog rows. It is honest *today*.
  The tabstrip had said "2 decomposed / 30 open" against a register at 3-plus-partial / 37 —
  hand-maintained numerics drifted again, exactly as `LE-30` predicted, in the parts `LE-30`'s
  closure did not cover.
- **TinyTile is fully planned and untouched here**: owner review of `EPIC-P6B`, `ADR 0012/0013`
  and `docs/tinytile-architecture.md` is still the open gate
  ([`01C`](01C-next-steps-after-tinytile-planning.md) Step 1); implementation stays queued behind
  the Pi 5 sprint. This mandate does not go near it.
- **The Pi 5 board work (`FEAT-P1-07`) remains the headline and remains blocked on one physical
  object** — the loopback-tested USB-serial adapter. When the adapter exists, the
  [board-session runbook](../../docs/pi5-board-session-runbook.md) outranks everything in this
  document. This mandate is what a session does honestly *while waiting*.
- Loose ends stand at **64 rows, 37 open**. The `_soak-p0-03-01.log` one-line append in the
  working tree belongs to the soak run — leave it, as three sessions now have.

## 1. The finding this mandate is built on

**`STORY-P0-01-08` is delivered and its own `Status:` header does not know it.** The header says
*Specified*. But [`REPORT-2026-07-28-11`](../../goals/reports/REPORT-2026-07-28-11.md) records
**Pass, all five clauses** on 2026-07-28; [`42A`](../hand-2026-07-28/42A-the-dashboard-generated-and-gated.md)
is the delivery session; `LE-30` is closed *by that Story* in the register; `emit-dashboard`
exists and runs today; and the spine prints `53 dashboard badges agree` on every gate run —
because the machinery this "Specified" Story built is what does the checking.

Read the failure class carefully, because it is the same one this repository keeps finding one
document along: the badge gate compares the dashboard badge to the Story header, both say
*Specified*, **both are wrong together, and the gate is green**. `LE-44` caught Feature-table
cells disagreeing with headers; `STORY-P0-01-08` caught badges disagreeing with headers; nothing
yet catches a header disagreeing with the Story's own filed Report. That is a third instance of
prose-versus-register, and it sat undetected for four days on the most-machine-checked page in the
project.

## 2. The work, in order

### Step 1 — Truth first: advance `STORY-P0-01-08` to the state its Report proves *(small, do it first)*

Per the [`Handover 35`](../hand-2026-07-28/35-le-43-closed.md) precedent: **verify, don't
inherit.** Re-run the Story's own evidence against the current tree before touching the header —
the host refusal tests (`cargo test -p xtask`), `emit-dashboard`'s byte-compare region,
`check-assurance-spine`'s badge and count gates — then, and only then:

1. Advance the Story header: `Status: Functionally Verified (Host), 2026-07-28` — citing
   `REPORT-2026-07-28-11`, with a dated note that the header was corrected on this session's date
   after re-verification, four days stale. Assurance state stays `baseline-debt` (the Report says
   so itself: no guardrail closed).
2. Update `FEAT-P0-01`'s Stories table and the dashboard badge **in the same change** — the
   `LE-44` and badge gates enforce agreement, so a partial edit is a red spine between tool calls
   (`CONCURRENT_SESSIONS` rule 8: validate with `check-spine-files` immediately, and
   `check-assurance-spine` before commit).
3. **Raise `LE-65`** for the gap itself: *no gate cross-checks a Story's `Status:` header against
   its own filed Reports* — a Story whose Report says "Pass, all clauses" can read Specified
   indefinitely with every gate green. Owner-path: extend the spine's status machinery
   (`STORY-P0-01-07`'s vocabulary) to refuse a `Specified`/`In progress` header on a Story whose
   linked Report records a passing result. Whether to *close* it in this session is Step 3's
   call; raising it is not optional — the register, not this handover, is where defects live.

**What moves on the dashboard:** the `STORY-P0-01-08` badge (SPECIFIED → FUNCTIONALLY VERIFIED
(Host)), the hand-written "Stories functionally verified" tile (48 → 49), and the honest end of a
four-day-old falsehood on the page's own worked-example Feature.

### Step 2 — The main course: `STORY-P0-01-09`, the numerics that survived `-08` become generated or gated *(the deliverable)*

`-08` deliberately generated only the *Assurance release status* tiles and gated only the
spine-count sentence, loose-end count, and badges. Everything numeric that remained hand-written
has since drifted — this very day. Decompose a new Story under `FEAT-P0-01`, **test-first, contract
row before code**, that finishes the job for the numerics while leaving the argument human:

**In scope (each either generated into a marked region, or extracted-and-gated like the count
sentence — choose per item and say why in the Story):**

- The four **Overall progress** tiles (Epics decomposed, Features, Stories functionally verified,
  Test docs) — pure `list-status` + spine arithmetic; the page itself has flagged them "distrust
  first" since 07-28. Generate them.
- The **tabstrip counts** ("Epics *N* decomposed", "Loose ends *N* open") — same data, same
  treatment.
- The **progress bar width** — derived from the Stories ratio, or deleted; a hand-tuned
  percentage is a decoration pretending to be a statistic.
- The **"Counted from `xtask list-status` on \<date\>"** footnote — gate the counts in it, or make
  the sentence part of the generated region so its date moves when the numbers do.
- The **Epic-decomposition claims** in the tabstrip/panel notes ("P2 partial", denominator 12) —
  gate against the epics on disk (`goals/epics/*.md` + backlog rows), so the next written Epic
  cannot leave the page claiming the old denominator.

**Explicitly out of scope, restating `-08`'s named debt because the temptation recurs:** the
prose argument, the per-Story tables, Report links, and the UPDATE narrative are editorial and
stay hand-written — the machine refuses their *claims* where extractable, it never writes their
*words*. Generating them wholesale destroys the page's value; that trade was declined once with
reasons and the reasons hold.

**Mechanics, all binding:**

- New Story doc `STORY-P0-01-09` + `TEST-P0-01-09-A` written first + `story-contracts.tsv` row
  (model it on `-08`'s: `D01`, `SEC-19`/`SEC-20`-class controls, `C0`/`C1`, the same boundary
  tests — justify any divergence in the contract row itself). `feature-contracts.tsv` needs no new
  row (`FEAT-P0-01` exists); `FEAT-P0-01.md`'s table gains the row in the same change.
- Red first: every refusal demonstrated — a stale tile, a stale tabstrip count, an unclosed
  region, a hand-edited generated block, an Epic-count claim that disagrees with disk — each with
  an acceptance case beside it, `-08` clause 5's shape exactly.
- `emit-dashboard` gains the new regions and **still must not run the check itself** (`-08`
  clause 1's reason: the command that prints the fix must not refuse to run when it is needed).
- Hand-edit `loose-ends.tsv` only with the write guarded and `check-spine-files` run before the
  next tool call. Stage narrowly; read `git diff --cached` before every commit; the pre-commit
  hook re-runs the gates on the staged tree.
- A dated `REPORT-2026-08-*` with the raw evidence, statuses and dashboard updated in the same
  change per the traceability sync rule — and this time the Story's header advances **in the
  delivery commit**, which is the whole moral of Step 1.

**What moves on the dashboard:** Stories 70 → 71, Tests 54 → 55, Reports 59 → 60 (spine floors
advance and the generated tiles say so *by themselves*); `FEAT-P0-01` grows to 9 Stories; the four
Overall tiles and tabstrip counts change from hand-written-and-stale-tomorrow to
generated-and-gated — after which the page moves every time the register moves, which is the only
kind of "moving the dashboard" worth building.

### Step 3 — If, and only if, Steps 1–2 land green and pushed: pick **one** of

- **Close `LE-65`** (the Report-vs-header gate raised in Step 1) — small if the Report parser from
  `STORY-P0-01-07`'s bound-provenance work is reusable; a natural `-01-09` sibling. Test-first.
- **`LE-34`** — `README.md`'s v1 supported-set prose list, the same failure mode in a third
  document. `-08`'s shape transfers; scope it as its own Story, don't smuggle it into `-09`.

Do not start both. A third partially-done register gate is worth less than two finished ones.

## 3. What this mandate is not

- **Not a licence to inflate the numbers.** `FEAT-P1-05` (hostile load) and `FEAT-P1-06`'s second
  half would move badges too — and they produce more Tier 0 evidence, which the dashboard itself
  says is the one thing this project has a surplus of. They stay queued behind the sprint.
- **Not TinyTile.** No decomposition, no contract rows, no `extern "C"` anything. The review gate
  (`01C` Step 1) is the owner's; the probes (`01C` Step 3) are opt-in and not this mandate's.
- **Not a board session.** If the adapter arrives mid-session, stop this work at the next green
  commit and open the runbook instead — board time outranks tooling time, by standing order.

## 4. Traps

1. **The exact-string gates bite the hand that feeds them.** The `LE-30` count gate refused this
   session's own dashboard refresh until the loose-end sentence matched byte-for-byte — the error
   message prints the expected block; paste it, don't retype it.
2. **Never hand-edit a generated region**, including "just to check". The byte-compare will
   refuse, and the fix direction is always regenerate-and-commit, not edit-and-hope.
3. **A status flip without re-verification is inheritance, not truth.** Handover 35's rule: verify
   the artifact against its evidence clause by clause, then write the state. Step 1 is cheap
   precisely because the evidence already exists; run it anyway.
4. **The timing gate flakes on unchanged code** (`LE-18`, witnessed twice now, most recently on a
   docs-only commit at 1.3% over a 2× tolerance on a 20-cycle metric). One re-run of the same
   commit before diagnosing; if it flakes twice in one session, that is new evidence for the
   `LE-18` row, not a reason to touch tolerances mid-mandate.
5. **The badge vocabulary is a deliberate mapping** (`-08`'s named debt). `-09` must not derive
   badge text by uppercasing header strings — a new state reaching the page must force a human to
   choose its wording.
6. **Two sessions, one tree** — `agent/CONCURRENT_SESSIONS.md` binds: `git config core.hooksPath
   .githooks` first, stage narrowly, never commit unread files, claim your handover number by
   creating the file before writing it.

## 5. Kill criteria — what would make this session's output dishonest

- A dashboard that generates its **argument**: if the diff replaces editorial prose with emitted
  text, the Story has overreached and the reasons `-08` declined that trade have been ignored.
- A green run offered as evidence: the committed tree satisfies the new gates by construction once
  fixed, so **only demonstrated refusals prove anything** — same as `-07` and `-08`.
- A count asserted anywhere the machine could have derived it. The number `9` (FEAT-P0-01's
  Stories), the number `71`, the number `49` — if any of them is typed by hand into a gated page,
  the session has rebuilt the defect it was mandated to remove.
- An unwatched push. `LE-64`'s closing line is now a rule: a push whose CI run is not watched is a
  gate that does not exist.
