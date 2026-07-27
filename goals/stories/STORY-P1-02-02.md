# STORY-P1-02-02 — Double-Fault Safety: TSS/IST Stack Switching

Status: **Specified, not yet started**
Feature: [`FEAT-P1-02`](../features/FEAT-P1-02.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

The safety net under the net: a TSS with dedicated IST stacks so `#DF` (and `#PF`-during-fault-handling) lands on a known-good stack instead of cascading into a triple-fault reset — the "TSS/IST" item Handover 32 explicitly named and left open. The double-fault handler is terminal-but-reporting: it records what it can (spoor + QEMU-visible failure exit under Tier 0) and halts, because a double fault means the primary fault path itself is compromised.

## Depends on

`STORY-P1-02-01`.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. A Tier 0 fixture that deliberately destroys the kernel stack (the classic escalating-fault scenario) reaches the IST-backed `#DF` handler and produces a distinguishable failure exit — not a silent QEMU triple-fault reset (`-no-reboot` makes the difference observable).
2. IST stack sizes are named constants with documented rationale, counted against `kernel::capacities`' static budget.
3. The primary `#PF`/`#GP` path is demonstrably unaffected (regression fixtures from `STORY-P1-02-01` still pass).

## Tests

Not yet written — deferred until this Story starts. Requires a Tier 0 escalating-fault fixture.

## Goals verified

G-SEC-2 (fault-path survivability), G-SEC-14.
