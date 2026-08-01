# Handover 03A — The Dashboard Moves Because the Register Moved: 02C Executed in Full

**Session date:** 2026-08-01, executing [`02C`](02C-next-session-mandate-the-dashboard-moves-itself.md)'s
mandate — all three steps, including the optional third. Three commits, pushed, CI watched to
completion and green (after one witnessed `LE-18`-class flake and one same-commit re-run, both
recorded below). No concurrent session was observed; the `_soak-p0-03-01.log` one-line append was
left in the working tree, as four sessions now have.

## What landed, in order

### 1. `d2c9969` — Step 1: `STORY-P0-01-08` advanced to the state its Report proved; `LE-65` raised

Verified first, per Handover 35's rule: 241 `xtask` host tests, `emit-dashboard`'s byte-compare,
and the badge/count gates re-run against the current tree before touching the header. Then the
Story header (`Specified` → `Functionally Verified (Host), 2026-07-28`, with a dated correction
note), `FEAT-P0-01`'s table cell, and the dashboard badge moved in one change. `LE-65` was raised
for the gap itself: nothing compared a Story's header to the Story's own filed Reports, so
header, table cell and badge were wrong *together* for four days with every gate green — the
third instance of the prose-versus-register class, on the most-machine-checked page in the
project.

### 2. `e71b30b` — Step 2, the deliverable: `STORY-P0-01-09`, the surviving numerics generated or gated

Test-first (`TEST-P0-01-09-A` + contract row before code), and the red was real: with the gates
implemented and the page untouched, the spine refused the committed tabstrip — **"3 decomposed"
directly beneath a tile saying 4/12**, one hand-refresh old. Per item:

- The four **Overall-progress tiles** are a second generated region (`emit-dashboard` prints
  both blocks, still never runs the check, still never writes the file).
- The **tabstrip counts**, **progress-bar width** (integer-rounded Stories ratio), **footnote
  state counts**, and the **Epic-denominator sentence** are extracted-and-gated; every refusal
  prints the expected text.
- The **Epic population is derived from disk**: `EPIC-P*` documents ∪ the backlog phase table
  (= 12), horizon `EPIC-H*` excluded on the backlog's own authority; "decomposed" = has ≥ 1
  Story contract row (= 4). A new `EPIC-P10.md` or backlog row moves the denominator by itself.
- The prose argument, per-Story tables and UPDATE narrative were **not** generated — `-08`'s
  named trade, restated and held.

Fourteen host tests (255 total then), each refusal beside its acceptance case.
[`REPORT-2026-08-01-01`](../../goals/reports/REPORT-2026-08-01-01.md); the Story's header
advanced **in the delivery commit**, which was the whole moral of Step 1.

### 3. `59466f9` — Step 3: `LE-65` closed by `STORY-P0-01-10`

The spine now refuses a `Specified` Story header covered by a Report whose `## Result` opens
with a bolded pass — the direction every existing gate missed. **Half of `LE-65`'s proposal did
not survive contact with the register and was deliberately dropped**: `In progress` beside a
passing Report is *legitimate stated-debt delivery* — [`REPORT-2026-07-30-01`](../../goals/reports/REPORT-2026-07-30-01.md)
records PASS while its `FEAT-P2` Stories deliberately stay `In progress` with `D23`/`D14`
numbers open — so refusing that pairing would refuse honesty. The row was amended with the
finding before closing. Seven more host tests (262 total); the summary line now prints
`8 passing Reports cross-checked against headers`.
[`REPORT-2026-08-01-02`](../../goals/reports/REPORT-2026-08-01-02.md).

**The `-09` machinery paid for itself inside the same session:** `-10`'s own documents moved the
register (71 → 72 Stories, 55 → 56 Tests), the spine refused the page, and the fix was
regenerate-and-paste rather than hand-arithmetic. The dashboard now moves because the register
moved, which is the only kind of "moving the dashboard" worth building.

## Where the numbers stand

`check-assurance-spine`, green at `59466f9`: **28 Features / 72 Stories / 56 Tests / 61
Reports · 65 loose ends (37 open) · 71 Feature/Story status rows agree · 8 passing Reports
cross-checked · 55 dashboard badges agree**. Stories functionally verified: **51 / 72**
(generated). No performance guardrail closed; no Story is assurance `verified`; nothing here
touched the hardware-evidence sprint's queue or TinyTile.

## CI, and one more `LE-18` witness

The push's run (`30720639920`) went red on exactly one job: the timing gate,
`D07/pool_u64x4_alloc_denied_exhausted_per_op_of_64` p50 at 82,692 against an 81,632 limit —
**1.3% over a 2× tolerance on a ~20-cycle metric**, on a commit that changed only docs and
`xtask`. That is the same class and nearly the same margin `02B` witnessed. Per the standing
rule: one re-run of the same commit, which came back green; run concluded `success`. This is the
**third witnessed** `LE-18`-class flake overall but the **first this session**, so no new
register evidence was filed (the mandate's threshold is twice in one session). If the next
session sees two, that is `LE-18` row evidence, not a reason to touch tolerances.

Also caught pre-push by the standing cross-target mirror: one clippy `redundant-closure` in the
new gate code that host-target clippy on Windows never saw. The `LE-64` rules held: both lint
jobs mirrored before every push, every push's run watched to completion.

## What the next session should know

- **The Pi 5 board sprint still outranks everything** the moment the loopback-tested USB-serial
  adapter exists; the [runbook](../../docs/pi5-board-session-runbook.md) is the entry point.
- **`LE-34` is now the last named member of this failure family** in reach: `README.md`'s v1
  supported-set prose list. `-08`/`-09`/`-10`'s shape transfers; scope it as its own Story.
- **Known ungated numerics that remain, deliberately:** Epic-panel narrative counts (e.g.
  `EPIC-P0`'s "has since grown to 28", now three Stories stale) — argument, not tiles; the
  footnote's date; and `In progress` headers above fully-passing Reports (the `-10` exemption,
  with its reason in the gate's docs).
- **The footnote gate assumes the four-state shape** (`Verified`/`Functionally
  Verified`/`Specified`/`In progress`). A fifth Story state will refuse until the sentence is
  reshaped by hand — intended.
- Handover numbering: this document claimed `03A` at session start, per
  `CONCURRENT_SESSIONS` rule 4.
