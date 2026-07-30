# Handover 05A — The Spoor Gate Is Closed (`LE-56`): 04A Executed

**Session date:** 2026-07-30. **Executes:** [`04A`](04A-spoor-gate-action-plan.md), the
owner-ordered action plan. **Concurrent session note (rule 7):** another session is live in
this tree — its in-progress edits to `03A-deep-os-textbook-code-review.md` and this folder's
`index.html` (the 03A review-lane entry) were left untouched and uncommitted; this session's
`index.html` entry was staged content-level over `HEAD` so nothing of theirs travelled.

## What landed — all seven of 04A §2, in one commit

The parity evidence chain now carries the kernel spoor journal as a **machine-checked third
signal**, and the claim is screenshotable. Evidence:
[`REPORT-2026-07-30-02`](../../goals/reports/REPORT-2026-07-30-02.md) (serial capture with
the `SPOOR` rows and `TINYOS-SPOOR/1` trailer, `06-parity-wall.png` with the three-signal
rule green, `03b-spoor-verb-host-side.png`, `smoke.json` with `spoor_signal: true`).

1. **Kernel vocabulary** (`spoor.rs`): `Category::Shell`, `Actor::Session`,
   `Action::VerbDenied` — red-first round-trip tests; two `ACT` nibble values of headroom
   remain.
2. **The decorator** (`os/src/shell/src/spoor_policy.rs`, beside the fixture — the shell
   library stays `no_std`/kernel-free): `SpoorPolicy` forwards to the real `GrantSet` and
   journals each denial into an atomic `DenialJournal` (same 8-byte `to_bits` record shape
   as `SPOORJ01` journals). Proven authorisation-neutral over the whole verb vocabulary
   (SP1–SP5). Shared by `#[path]` with a `#[cfg(test)]`-only lib include so
   `cargo test -p shell --lib` runs its tests — an integration-test placement fails because
   cargo auto-builds the bare-metal fixture bin for integration tests.
3. **In-guest assertion**: `spoor_journal_len == denials == expected_denials()`; the
   deliberate-red run (`len == 0`) was observed failing under QEMU first.
4. **Trailer + splitter**: fixture emits `TINYOS-SPOOR/1 len=<n> denials=<n>` *after* the
   transcript; `check-shell-parity` splits at the marker (transcript stays byte-sacred),
   fails closed on absent/malformed/mismatch, and its success line names all three facts.
5. **Three-signal tab**: `parse_parity_signals` → 3-tuple, `overall_verdict` requires the
   spoor signal affirmatively, wall row `spoor journal corroborates denials
   (TINYOS-SPOOR/1)`, `smoke.json` gains `spoor_signal`, reserved `parity: PASS` implies it.
6. **The `SPOOR` verb** (register row `verb:spoor-journal`, `live-verified`):
   `VerbKind::SpoorJournal` renders an injected kernel-free `SpoorView`; the parity lane
   (host golden test and fixture alike) injects the live decorator journal via the new
   `parity::world_with(policy, spoors)` seam, so the golden shows the spoor row
   byte-identically on host and target (64-line golden, regenerated via the `#[ignore]`d
   recorder, diff = exactly the three `SPOOR` lines). Host tabs inject an empty view whose
   banner says `host-side journal` — honest, per 04A §3.
7. **Close-out**: `STORY-P2-07-02` (contract `D14,D23 / SEC-05,SEC-14 / C1,C2`,
   `baseline-debt`, matching open-debt rows), `TEST-P2-07-01-A` grown clause 4,
   `FEAT-P2-07` status updated, `LE-56` closed, dashboard + count sentences re-synced,
   `check-assurance-spine` green.

## Incidental find, fixed in the same change

The committed golden is LF but `core.autocrlf=true` smudged it to CRLF on Windows
checkouts, so the host golden test (`p1`) was red on clean `HEAD` here while CI was green.
Fixed both layers: root `.gitattributes` marks `*.golden.txt -text` (byte-exact checkout
everywhere), and `p1` normalises the golden side CRLF→LF — the same normalisation xtask's
`compare_transcript` already states.

## What stays open (by design, stated in the Story's Not-claimed)

Grant/outcome spoors (later seam); host tabs kernel-spoorless until the on-target tab host
exists (`LE-53`/`LE-55` lanes); supervisor-scope `task-kill` refusals not journaled by the
decorator (different seam, not exercised by the parity `.TCB`); no `D14`/`D23` numbers.

## Verification surfaces (all green at handover)

`cargo test --workspace` (os/, 606 tests), `cargo test -p stage-e-console` (24 + 7 e2e),
`cargo run -p xtask -- check-shell-parity`
(`… TINYOS-SPOOR/1 len=1 denials=1`), `check-spine-files`, `check-assurance-spine`,
`STAGE_E_SMOKE=1` windowed run exit 0.
