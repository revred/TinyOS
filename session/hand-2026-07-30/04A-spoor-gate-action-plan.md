# Handover 04A — Action Plan: Fix the Spoor Gate (`LE-56`)

The start-here work order, in the `13F`/`17G` mandate pattern. Ordered by the owner
2026-07-30 after [`03A`](03A-android-plan-and-the-spoor-gap.md) recorded the gap: **no
console/tab run has ever captured a kernel spoor**. The end state this plan buys: the
parity evidence chain carries the spoor journal as a *machine-checked signal*, the parity
tab renders it, and the claim is screenshotable — never again asserted in prose.

## 0. Design decision made here (so the implementer doesn't re-derive it)

The `shell` crate stays `no_std`, kernel-free and flavour-agnostic — `EPIC-P2` §3.2's
one-core rule. Therefore **spoor emission does not go inside the shell crate**. The policy
seam already sees every verdict: `verbs::execute` consults `VerbPolicy::allows` on each
request. A **journaling decorator policy** — a wrapper `VerbPolicy` that forwards to the
real `GrantSet` and journals each *denial* as a `Spoor` — captures the audit fact at the
same decision point with **zero shell-crate changes**. The fixture (kernel side) installs
the decorator over the parity policy; the host tab keeps its existing console-side denial
log and remains honestly spoorless until the on-target tab host exists (that half of
`LE-56` stays open by design, stated, not silently). Outcome-level spoors (grants, costs)
are a later seam; this plan closes the denial half the register names.

## 1. The work, TDD-ordered — a failing test precedes every step

1. **Kernel vocabulary** (`os/src/kernel/src/spoor.rs`): add the shell taxonomy rows —
   `Category` gains a shell/ACI entry, `Action` gains `VerbDenied` (names per the file's
   own taxonomy conventions; `CAT`/`ACT` are TinyOS-owned, the wire format is fixed).
   *Red first*: a kernel unit test asserting the new variants pack/unpack losslessly
   through the 64-bit atom, alongside the existing round-trip tests.
2. **The decorator** (in the fixture binary, `os/src/shell/src/fixture_batch_main.rs`, or
   a small shared module beside it): `SpoorPolicy<'a>` wrapping `&'a GrantSet` + a journal
   handle; `allows()` forwards, and on `false` journals
   `(Category::Shell, Actor=session, Action::VerbDenied, Outcome::Failed, target=verb)`.
   *Red first*: a host-side unit test (the decorator is plain code) — one denied verb
   produces exactly one journal append; a granted verb produces none; the wrapped verdict
   is unchanged (the decorator must never alter authorisation).
3. **The fixture asserts the journal** — the in-guest half of the two-signal discipline:
   after the batch run, `spoor_journal_len == stats.denials == parity::expected_denials()`
   (today: exactly 1, the withheld `CLS`). A mismatch is an in-guest assertion failure →
   `isa-debug-exit` failure code. *Red first*: temporarily assert `len == 0` and watch the
   fixture fail under QEMU, then assert the real invariant.
4. **The serial trailer, without breaking the golden**: the transcript byte-comparison is
   sacred (`TEST-P2-07-01-A`). The fixture emits, *after* the transcript, one marker line:
   `TINYOS-SPOOR/1 len=<n> denials=<n>`. `check-shell-parity` (`os/src/xtask`) splits the
   capture at the marker: everything before it byte-compares against the untouched golden;
   the trailer parses into a **third signal** — "spoor journal corroborates the denial
   count". *Red first*: `shell_parity.rs` unit tests for the splitter (marker present,
   absent, malformed, count mismatch — absent/malformed is a FAIL, never a skip; the
   success line names all three facts).
5. **The parity tab renders the third signal**: `parity_suite.rs` already parses
   `check-shell-parity`'s output — extend `parse_parity_signals` to the spoor signal and
   `overall_verdict` to require it affirmatively (the two-signal rule becomes
   three-signal, missing-never-passes preserved). The tab's wall gains the row
   `spoor journal corroborates denials (TINYOS-SPOOR/1)`; the smoke JSON gains
   `spoor_signal`; the reserved region's `parity: PASS` now implies it. *Red first*: `s2`/
   `s3`-style host tests over canned output, then the e2e smoke re-run for the screenshot.
6. **The journal-dump verb — the visible half** (register-decided, terminal-gap
   discipline): add the verb to `goals/context/terminal-gap.tsv` with its decided output
   shape (suggested binding: `SPOOR` — lists journal entries oldest-first, fixed-width,
   deterministic rendering; refusal shape when the journal is empty). `VerbKind` gains the
   entry behind the same deny-by-default seam (`shell` renders from an injected journal
   *view* — a `&[Spoor]`-shaped World field, host-injectable exactly like `tasks`, so the
   crate stays kernel-free). Add it to the parity `.TCB` script, regenerate the golden
   **deliberately** (`regenerate_golden`, reviewed as a diff — the `LE-23` division of
   labour), and the transcript itself then *shows* spoors under QEMU. Host tabs render
   their injected view and say `host-side journal` in the banner — honest, not blurred.
7. **Contracts and close-out**: Story row in `story-contracts.tsv` (under `FEAT-P2-07`,
   security-control selection includes the audit row; `STORY-P0-06-01` is the spoor
   atom's own contract and is not re-opened), statuses updated, `LE-56` flipped to closed
   citing this plan's evidence, dashboard re-synced, spine green.

## 2. Acceptance — what "fixed" means, all seven or it isn't

1. Fixture in-guest assertion covers `spoor_journal_len` (step 3) — signal A.
2. Golden byte-comparison untouched semantics, trailer parsed — signal B + the new
   signal C, all three named in `check-shell-parity`'s success line.
3. Parity tab wall shows the spoor row; `smoke.json` carries `spoor_signal: true`;
   a committed screenshot shows the wall including the spoor row **and** the `SPOOR`
   verb's output in a transcript.
4. Decorator proven authorisation-neutral (step 2's test) — auditing must never change
   a verdict.
5. Missing/malformed trailer fails closed (step 4's tests).
6. Golden regenerated only via the `#[ignore]`d recorder, committed as a reviewed diff.
7. `check-assurance-spine` green; `LE-56` closed in the same commit as the last test.

## 3. Bounds and non-claims

Denial spoors only (grant/outcome spoors are a named later seam); host tabs remain
kernel-spoorless until the on-target tab host exists — the `SPOOR` verb over an injected
view makes that boundary visible instead of hiding it; no wire-format change to the spoor
atom (taxonomy additions only); no new QEMU fixture (the `shell-batch` fixture grows an
assertion — `08C` §4's no-new-fixtures rule doesn't bind the shell lane, but there is no
need). Estimated shape: kernel vocab + decorator + fixture ~1 session with the parity
golden change; the verb + tab surfacing a second. Sequence exactly as numbered — the
golden regeneration (step 6) lands *after* the splitter (step 4) so the golden never
carries the trailer.
