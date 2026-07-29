# STORY-P2-04-01 — DOS Parser: Switches, %VAR%, Register-Shape Parity, Adversarial Totality

Status: **In progress** — assurance state `baseline-debt` (`D23` readiness `design`)
Feature: [`FEAT-P2-04`](../features/FEAT-P2-04.md)
Introduced in: [`session/hand-2026-07-29/16G`](../../session/hand-2026-07-29/16G-tinycmd-vertical-slice.md)
Started: 2026-07-30

## Description

DOS-syntax lines to canonical requests: command-word dispatch over the register's DOS
bindings (`DIR`, `CD`/`CHDIR`, `COPY`, `MOVE`, `REN`, `DEL`/`ERASE`, `MD`/`MKDIR`,
`RD`/`RMDIR`, `TYPE`, `FIND`, `SORT`, `MORE`, `TREE`, `ATTRIB`, `VOL`, `SET`, `PATH`, `ECHO`,
`CLS`, `VER`, `MEM`, `TASKMGR`), `/switch` parsing with the per-verb switch tables the 4.0
source defines plus the recorded deliberate extensions, `%VAR%` expansion against the session
environment, case-insensitive command words. Unknown command → the 4.0 shape
`Bad command or file name`; bad switch → `Invalid switch`; wrong arity → the PARSE-block
shapes (`Required parameter missing`, `Too many parameters`).

## Acceptance criteria

1. Every register DOS binding parses to the right canonical request (table-driven test,
   one row per binding).
2. Message-shape parity: the parser's refusals are byte-equal to the register's recorded
   strings.
3. Totality: a fuzz-shaped corpus (unterminated quotes, `%` bombs, 1-byte to max-length
   lines, raw control bytes) never panics and never produces a request the line does not
   spell — asserted by round-tripping accepted requests back to their meaning.
4. `%VAR%` expansion is bounded (no recursive expansion), and an undefined variable expands
   to the 4.0 behaviour (literal removal), recorded either way in the test.

## Not claimed

No pipes/redirection tokens in this slice (stated debt shared with `STORY-P2-01-01`); no
POSIX flavour (that is `FEAT-P2-05`, which inherits the equivalence obligation); no
performance number (`D23` open debt).
