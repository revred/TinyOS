# STORY-P1-03-02 — W^X/NX Mappings & Generation-Safe Teardown

Status: **Specified, not yet started**
Feature: [`FEAT-P1-03`](../features/FEAT-P1-03.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

Executable sealing and clean death: replace the all-RWX view with W^X/NX-correct mappings (kernel text RX, rodata RO-NX, data/stacks RW-NX; task sections per their PE64 permissions, which `exec` already computes but nothing yet enforces at runtime), and implement address-space teardown per the charter's `PD-13`: revoke mappings, wipe frames, advance the generation before any frame reuse — closing the "executable sealing" and "teardown" items on the Security Charter's runtime-evidence list.

## Depends on

`STORY-P1-03-01`.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. A write to executable memory and an execute of writable memory each *fault* under Tier 0 (`BND-05` proven adversarially in both directions), contained by the `#PF` handler.
2. Teardown-then-probe: after a task's space is torn down, a stale-mapping probe faults and a reused frame is provably wiped (fixture checks for the dead task's residue) with the generation advanced.
3. No mapping anywhere in the running system is simultaneously writable and executable — verified by a page-table audit fixture, not by convention.

## Tests

Not yet written — deferred until this Story starts. Requires Tier 0 W^X-violation and teardown fixtures plus a page-table audit.

## Goals verified

G-SEC-2 (W^X/teardown halves), G-SEC-8 (immutable-image substrate, partial).
