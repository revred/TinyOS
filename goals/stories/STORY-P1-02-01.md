# STORY-P1-02-01 — `#PF`/`#GP`/`#UD` Handlers: Capture, Terminate-vs-Resume, Spoor Audit

Status: **Specified, not yet started**
Feature: [`FEAT-P1-02`](../features/FEAT-P1-02.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

Real exception handlers for page fault, general protection, and invalid opcode: save the full faulting context (the existing context-switch frame discipline extended with error code and, for `#PF`, `CR2`), route to a kernel fault-policy decision — terminate the faulting task (default; fail closed) or resume where a documented, explicitly-enumerated recoverable case applies — emit a spoor for every fault with class/actor/action/outcome, and continue scheduling everything else. Every vector not explicitly handled keeps `STORY-P0-04-02`'s diverge-and-report default.

## Depends on

`STORY-P1-01-01` (fault-path overhead is measured); `STORY-P0-04-02`'s IDT.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. A deliberate `#PF`, `#GP`, and `#UD` in a victim task each terminate *that task only* — a Tier 0 fixture proves another task keeps running and the fault appears in the spoor journal.
2. Termination is the default policy; any resume case is explicitly enumerated, documented, and separately tested — no speculative "maybe recoverable" paths.
3. Fault-frame parsing is defensive: error codes and fault addresses are hostile input and never trusted into authority decisions.

## Tests

Not yet written — deferred until this Story starts. Requires Tier 0 fault-injection fixtures plus host tests for policy logic.

## Goals verified

G-SEC-2 (fault-containment half), G-SEC-14.
