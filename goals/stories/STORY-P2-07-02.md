# STORY-P2-07-02 — The Spoor Gate: Kernel-Audited Denials in the Parity Evidence Chain

Status: **Verified (Tier 0, denial half), 2026-07-30** — assurance state `baseline-debt`
(`D23` readiness `design`, `D14` `stand-in-only`; no performance number claimed)
Feature: [`FEAT-P2-07`](../features/FEAT-P2-07.md)
Introduced in: [`session/hand-2026-07-30/04A`](../../session/hand-2026-07-30/04A-spoor-gate-action-plan.md)
(ordered by the owner after [`03A`](../../session/hand-2026-07-30/03A-android-plan-and-the-spoor-gap.md)
recorded `LE-56`: no console/tab run had ever captured a kernel spoor)
Started: 2026-07-30

## Description

Close the spoor gap in the shell/target lane: the parity evidence chain carries the kernel
spoor journal as a **machine-checked signal**, and the claim is screenshotable, never again
asserted in prose. A journaling decorator policy (`SpoorPolicy` over the real `GrantSet`,
beside the fixture — the `shell` crate stays `no_std`, kernel-free) journals each verb
denial as a `Spoor` `(Category::Shell, Actor::Session, Action::VerbDenied, Outcome::Failed,
target = the denied verb)` at the same `VerbPolicy::allows` decision point every request
passes. The `shell-batch` fixture asserts `spoor_journal_len == denials ==
expected_denials()` in-guest, emits a `TOS64-SPOOR/1 len=<n> denials=<n>` trailer *after*
the transcript, and `check-shell-parity` splits the capture at that marker: the transcript
before it stays byte-sacred against the golden; the trailer parses into a **third signal**
(missing/malformed fails closed). The register-decided `SPOOR` verb renders the journal
from an injected kernel-free view, so the golden transcript itself shows the spoor row
under QEMU; the parity tab renders the three-signal rule and `smoke.json` carries
`spoor_signal`.

## Acceptance criteria (hand-2026-07-30/04A §2, all seven)

1. Fixture in-guest assertion covers `spoor_journal_len` — signal A.
2. Golden byte-comparison semantics untouched; trailer parsed — signals B and C, all three
   named in `check-shell-parity`'s success line.
3. Parity tab wall shows the spoor row; `smoke.json` carries `spoor_signal: true`; a
   committed screenshot shows the wall including the spoor row **and** the `SPOOR` verb's
   output in a transcript.
4. Decorator proven authorisation-neutral — auditing must never change a verdict.
5. Missing/malformed trailer fails closed.
6. Golden regenerated only via the `#[ignore]`d recorder, committed as a reviewed diff.
7. `check-assurance-spine` green; `LE-56` closed in the same commit as the last test.

## Not claimed

Denial spoors only — grant/outcome spoors are a named later seam. Host tabs remain
kernel-spoorless until the on-target tab host exists: the `SPOOR` verb over the injected
view makes that boundary visible (`Spoor journal (host-side journal): No spoors journaled`)
instead of hiding it. The supervisor-scope refusal inside `task-kill` is a separate seam
(not exercised by the parity `.TCB`). No wire-format change to the spoor atom (taxonomy
additions only: `Category::Shell`, `Actor::Session`, `Action::VerbDenied`). No new QEMU
fixture. No performance number (`D23`/`D14` debt stands, same rows as `STORY-P2-07-01`).
`STORY-P0-06-01` (the spoor atom's own contract) is not re-opened.

## Test

`TEST-P2-07-01-A` — [`goals/tests/TEST-P2-07-01-A.md`](../tests/TEST-P2-07-01-A.md), grown
a third signal (this Story's §1.4); evidence in
[`REPORT-2026-07-30-02`](../reports/REPORT-2026-07-30-02.md).
