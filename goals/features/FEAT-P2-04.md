# FEAT-P2-04 — The DOS Flavour Front-End

Status: **In progress — Story 01 started 2026-07-30** (assurance `baseline-debt`, `D23`)
Epic: [`EPIC-P2`](../epics/EPIC-P2.md) §7 priority 5 — a **hostile-format parser** by
classification, held to parser discipline
Introduced in: [`session/hand-2026-07-29/16G`](../../session/hand-2026-07-29/16G-tinycmd-vertical-slice.md)

## Description

The DOS syntax front-end: `DIR /P`, `COPY A B`, `DEL /P`, `%VAR%` expansion — parsed into the
canonical verb requests of `FEAT-P2-01`, **never executed directly**. Behaviour is bound to the
source-verified register ([`goals/context/terminal-gap.tsv`](../context/terminal-gap.tsv)):
4.0's message shapes and switch semantics where 4.0 was right, the recorded deliberate
divergences where it was not (owner directive, EPIC-P2 §2: *4.0 is a milestone, never the
destination* — `MOVE` exists, recursion exists, exit codes are always meaningful). The parser
is total over arbitrary bytes: no input panics, no input bypasses the core, unknown commands
answer the 4.0-shape `Bad command or file name`.

## Crate(s) involved

`os/src/shell/` (the `dos` module).

## Depends on

`FEAT-P2-01`, `FEAT-P2-02`. The POSIX flavour (`FEAT-P2-05`) inherits the three-way
equivalence obligation when it lands; the DOS/canonical pair carries it until then.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P2-04-01`](../stories/STORY-P2-04-01.md) | DOS parser: switches, `%VAR%`, message-shape parity with the register, adversarial totality | In progress |

## Containment contract

See `goals/assurance/feature-contracts.tsv` row `FEAT-P2-04`. The front-end's input is
attacker-influenceable by definition (batch files, pasted text); hostile inputs are malformed
switch soup, unterminated quotes, `%`-expansion bombs, traversal attempts and
escape-sequence injection. The parser holds no authority: it produces requests the core then
authorises (`PD-03`/`PD-14`), so a parser bug is a wrong request, never a bypass (`BND-20`).
