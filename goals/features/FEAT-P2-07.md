# FEAT-P2-07 — The `.TCB` Batch Runtime

Status: **In progress — Story 01 started 2026-07-30; Story 02 (the spoor gate, `LE-56`) Verified 2026-07-30** (assurance `baseline-debt`, `D23`)
Epic: [`EPIC-P2`](../epics/EPIC-P2.md) §7 priority 7 — deliberately late in the Epic's own
ordering because *"a batch runtime is an authority multiplier"*; pulled into this vertical
slice because the owner's parity harness **is** a batch file, and a batch that can only run
under the fixture's own session multiplies nothing it wasn't already granted
Introduced in: [`session/hand-2026-07-29/16G`](../../session/hand-2026-07-29/16G-tinycmd-vertical-slice.md)

## Description

The DOS-flavoured batch runtime (`.TCB`): sequential line execution through the DOS front-end,
`ECHO ON/OFF` semantics with the `@` prefix, `REM`, and the 4.0 echo discipline read from
source (the raw command tail printed when echo is on, exactly as `TUCODE.ASM` does it).
Control flow (`IF`/`GOTO`/`FOR`/`CALL`/`SHIFT`) is **not** in this slice and is stated debt in
the Story. A batch runs *within* one session's authority — it can never grant, only spend
(`BND-14`, `PD-14`); every line's verbs pass the same core authorisation as an interactive
line. This is the harness the owner ordered: **the MS-DOS parity tests are `.TCB` files**,
executed under QEMU, their serial transcript compared byte-wise against committed golden
files by `xtask check-shell-parity`.

## Crate(s) involved

`os/src/shell/` (the `batch` module); `os/src/xtask/` (the parity gate);
the `shell-batch` fixture binary.

## Depends on

`FEAT-P2-04` (batch is DOS-flavoured), therefore `FEAT-P2-01`/`-02`.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P2-07-01`](../stories/STORY-P2-07-01.md) | Batch runner + QEMU fixture + golden-transcript parity gate (`TEST-P2-07-01-A`) | In progress |

## Containment contract

See `goals/assurance/feature-contracts.tsv` row `FEAT-P2-07`. A batch file is
**authority-bearing content**: hostile inputs are adversarial `.TCB` bytes — verbs the session
was never granted (must be denied per line, audited), escape-sequence payloads aimed at the
transcript, unbounded lines and runaway length (bounded line count and line length, refusal on
excess). Quarantined batch content is refused execution outright (`BND-11` shape at the
content-not-code level: a quarantine label on a `.TCB` is a refusal, and running a batch never
promotes its provenance, `BND-13`).
