# FEAT-P2-01 — TINYCMD Canonical Verb Core + ACI Authorisation Seam

Status: **In progress — Story 01 started 2026-07-30** (assurance `baseline-debt`: `D23` is a
`design`-readiness domain; every selected domain carries an open-debt row until the shell
subsystem is measurable under the catalogue)
Epic: [`EPIC-P2`](../epics/EPIC-P2.md) §7 priority 1 — *"first, always"*
Introduced in: [`session/hand-2026-07-29/16G`](../../session/hand-2026-07-29/16G-tinycmd-vertical-slice.md),
executing the owner's order: run MS-DOS tests like a `.bat` file and check output parity

## Description

The canonical verb core of `TINYCMD` ([`docs/cli-compatibility-mvp.md`](../../docs/cli-compatibility-mvp.md)):
typed, capability-checked request objects for the MVP verb set, executed through **one**
authorisation decision point — the ACI seam — with spoor-style audit emission. **No syntax
front-end lives here** (EPIC-P2 §3.2: one core, one decision point, or a front-end grows its
own policy path). Every verb resolves deny-by-default against the session's granted verb set;
identity is the session, never the request payload. Output is written through a `fmt::Write`
sink so the same core serves a serial fixture, a future tab host, and host-side tests
byte-identically.

## Crate(s) involved

`os/src/shell/` (new — the crate the delivery strategy reserves for Phase 2;
`#![forbid(unsafe_code)]` in the library).

## Depends on

`EPIC-P0` (complete). The full ACI capability registry is Roadmap Phase 5; this Feature builds
the *seam* (a `VerbPolicy` trait answering allow/deny per session+verb) so the engine can be
installed later without touching a verb — the same externalised-resolver shape Stage C proved
for the Tauri fork.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P2-01-01`](../stories/STORY-P2-01-01.md) | Verb core: typed requests, deny-by-default policy seam, deterministic output formatting | In progress |

## Containment contract

See `goals/assurance/feature-contracts.tsv` row `FEAT-P2-01`. Implementation is
memory-safe library code (`C1`/`C2` at Tier 0, where the fixture runs it kernel-side); subjects
are the operator session and, later, hostile tab content. Hostile inputs are
attacker-influenced command arguments: traversal paths, oversized names, escape-sequence-bearing
strings (EPIC-P2 §6.5 rule 3). An unlisted verb is refused with the denial audited (`BND-17`);
no verb acquires authority from possession of a name (`PD-14`, `PD-03`).
