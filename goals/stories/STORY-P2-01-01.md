# STORY-P2-01-01 — Verb Core: Typed Requests, Deny-by-Default Seam, Deterministic Output

Status: **In progress** — assurance state `baseline-debt` (`D23` readiness `design`; the
open-debt row records why no guardrail number is claimable yet)
Feature: [`FEAT-P2-01`](../features/FEAT-P2-01.md)
Introduced in: [`session/hand-2026-07-29/16G`](../../session/hand-2026-07-29/16G-tinycmd-vertical-slice.md)
Started: 2026-07-30

## Description

The canonical verb set as typed request values; a `VerbPolicy` seam answering allow/deny per
(session, verb) with **deny-by-default** — no installed policy means nothing runs; execution
writes through a `fmt::Write` sink with output shapes bound to the terminal-gap register
(`DIR` header/footer, message strings, `Press any key…` only where the register says so).
Untrusted strings (file names, variable values) are rendered inert before display: C0/C1
control bytes and `ESC` are replaced, so a filename cannot move the cursor (EPIC-P2 §6.5
rule 3 — the test plants escape sequences in a filename).

## Acceptance criteria

1. Every MVP verb is a typed request; an unlisted/unimplemented verb refuses with the
   register's message shape and a meaningful error code — never a panic.
2. With no policy installed, every verb is denied; with a policy, exactly the granted verbs
   run; denials are observable (audit line) with the session identity attached.
3. Output through the sink is byte-deterministic for a fixed volume state — the property the
   golden-transcript gate (`TEST-P2-07-01-A`) stands on.
4. A filename carrying `ESC[2J` (or any C0 byte) renders inert in `DIR`/`TYPE`/`TREE` output.

## Not claimed

No performance number (`D23` open debt). No pipes/redirection in this slice (stated debt,
with `sort-stream`/`page` accepting seeded input only). `task-list`/`task-kill` execute
against an injected task-table trait, not the live scheduler, until the tab host exists.
