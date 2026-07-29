# STORY-P2-07-01 — Batch Runner + QEMU Fixture + Golden-Transcript Parity Gate

Status: **In progress** — assurance state `baseline-debt` (`D23` readiness `design`)
Feature: [`FEAT-P2-07`](../features/FEAT-P2-07.md)
Introduced in: [`session/hand-2026-07-29/16G`](../../session/hand-2026-07-29/16G-tinycmd-vertical-slice.md)
Started: 2026-07-30

## Description

The owner's harness: **an MS-DOS test is a `.TCB` file**. The batch runner executes lines
through the DOS front-end with 4.0 echo discipline (`ECHO ON` default, `@` suppression,
`REM`, echo-off inside nothing yet — no pipes); the `shell-batch` QEMU fixture seeds the
labelled RAM volume, runs the embedded parity script, streams the transcript over COM1, and
exits through `isa-debug-exit` with success only if every line's own expectation held; and
`cargo run -p xtask -- check-shell-parity` boots the fixture with serial capture and compares
the transcript **byte-wise** against the committed golden file — parity with the best version
of MS-DOS, where "best" is the register's decided column, not nostalgia
(EPIC-P2 §2: 4.0 is a milestone, never the destination).

## Acceptance criteria

1. The parity `.TCB` exercises at minimum: `VER`, `VOL`, `ECHO` (on/off/`@`), `SET`, `PATH`,
   `MD`, `CD`, `DIR` (header, `<DIR>` rows, footer), `TYPE`, `COPY` (with label carriage
   observable via `ATTRIB`-view), `REN`, `MOVE`, `DEL`, `TREE` (`/A` ASCII form), `FIND`
   (`/C`, `/N`, `/V`), `MEM`, `RD`, and an unknown command answering
   `Bad command or file name`.
2. `check-shell-parity` fails on any byte difference, prints a first-divergence diff, and
   passes on the committed golden file; the fixture's `isa-debug-exit` verdict and the
   transcript comparison must both hold (the two-signal discipline `timing.rs` already uses).
3. A denied verb inside a batch (policy withholds it) refuses that line, audits it, and
   continues — a batch cannot spend authority its session lacks.
4. Deterministic transcript across runs (two consecutive boots byte-identical).

## Not claimed

`IF`/`GOTO`/`FOR`/`CALL`/`SHIFT` control flow — stated debt, next Story in this Feature.
No interactive input (serial is TX-only, `LE-55`); `MORE` runs in its non-paging batch form.
No performance number (`D23` open debt) — but this fixture is the natural host for `TG-P02`
prototyping, noted in the gap analysis Axis 4.

## Test

`TEST-P2-07-01-A` — [`goals/tests/TEST-P2-07-01-A.md`](../tests/TEST-P2-07-01-A.md), the
fixture + parity-gate pair.
