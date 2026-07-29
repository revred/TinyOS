# Handover 16G — TINYCMD Exists: The MS-DOS Parity Tests Run as a `.TCB` Under QEMU

Same session as [`15G`](15G-fork-vendored-in-tree.md), continuing on `os.tauru.poc`.
Executes the owner's order: *"Fill all the gaps — run the MSDOS tests like running a .bat
file and check the UX is apt and the output is at parity with the best version of MSDOS"* —
with the owner's rider that the kernel beneath must serve **DOS, Linux/POSIX, RT and a
minimal GUI shell equally**, which is exactly `EPIC-P2` §3.2's one-core-many-front-ends
architecture and is preserved here: nothing DOS-shaped exists below the parser layer.

## 1. What exists now that did not this morning

The `shell` crate (`os/src/shell/`, 1,666 lines, `#![forbid(unsafe_code)]` lib), four
Features decomposed with contracts and Stories, and a passing end-to-end parity harness:

1. **`FEAT-P2-01` — the canonical verb core + ACI seam.** 22 typed verbs, one
   deny-by-default `VerbPolicy` decision point, audited denials carrying session identity,
   untrusted strings rendered inert (a filename carrying `ESC[2J` cannot repaint), output
   through `fmt::Write` so host tests, the QEMU fixture and a future tab host render
   byte-identically.
2. **`FEAT-P2-02` — the labelled RAM volume**, closing **`LE-48`** (the owner's order *is*
   the acceptance of EPIC-P2 §5's proposal). `G-SEC-5` labels from creation; quarantine
   survives copy→rename→copy chains; traversal refused; every capacity exhausts as a typed
   refusal.
3. **`FEAT-P2-04` — the DOS front-end.** Register-bound switch tables and message shapes
   (`Bad command or file name`, `Invalid switch`, the PARSE-block strings), `%VAR%`
   expansion (bounded, non-recursive), total over adversarial input.
4. **`FEAT-P2-07` — the `.TCB` batch runner + the parity harness.** 4.0 echo discipline
   (`@`, `ECHO ON/OFF`, `REM`, prompt+raw-line echo as `TUCODE.ASM` does it); the
   `shell-batch` QEMU fixture seeds the volume, runs the embedded parity script and
   streams the transcript over COM1; `cargo run -p xtask -- check-shell-parity`
   byte-compares it against the committed golden
   (`os/src/shell/golden/parity-smoke.golden.txt`, 61 lines) — **PASS**, with the
   two-signal discipline: in-guest assertions gate `isa-debug-exit`, the transcript gates
   the comparison, and neither alone is a pass. Both CI steps added (the fixture-coverage
   test in xtask forces that honesty).

Evidence: 20/20 shell host tests (volume V1–V5, core C1–C4, DOS D1–D5, batch B1–B3,
parity P1–P2), 204/204 xtask tests, spine green throughout
(27 Features / 67 Stories / 52 Tests), `TEST-P2-07-01-A` written before the code.

## 2. Decisions and honest bounds

- **"Best version of MS-DOS" = the register's decided column**, not nostalgia: `MOVE`
  exists, `DEL` demands `/Y` in scripts (stricter than 4.0 *and* POSIX), exit paths are
  always meaningful. Golden-file changes are review events by construction.
- **Stated debt, deliberately:** no pipes/redirection (`SORT`/`MORE` take a file
  argument); no wildcards; no `IF`/`GOTO`/`FOR` (next Story in `FEAT-P2-07`); `task-*`
  runs against an injected table until the tab host lands; `D23`/`D14` guardrail numbers
  are open-debt rows until the measurement harness covers the shell (the `TG-P*` ids wait
  there). Statuses stay **In progress** — a Feature with one exercised Story is not
  Complete.
- **The golden recorder is `#[ignore]`d** (`regenerate_golden`) — run deliberately,
  reviewed as a diff, the LE-23 division of labour.
- The fixture binary is boot glue on the `exec-fixture` pattern; kernel/exec/shell fixture
  bins type-check on CI's ubuntu but not on this Windows host (`hal-x86_64`'s
  `not(windows)` gates) — pre-existing, now affecting one more crate.

## 3. Queue

1. **Flavour equivalence**: `FEAT-P2-05` (POSIX front-end) + the three-way equivalence
   test; `FEAT-P2-06` (RT flavour) — both unblocked today.
2. **Batch control flow** (`IF`/`GOTO`/`FOR`/`CALL`) and redirection/pipes in the core.
3. **First `D23` measurements** through the batch fixture (`TG-P02`/`TG-P03` prototypes).
4. Everything 15G §3 already lists (upstream PR, ADR 0010/0011, terminal-gap spine gate,
   merge decision).
