# STORY-P1-02-02 — Double-Fault Safety: TSS/IST Stack Switching

Status: **Verified (Tier 0 + Host) 2026-07-27 — assurance state `baseline-debt`**
Feature: [`FEAT-P1-02`](../features/FEAT-P1-02.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)
Delivered in: [`session/hand-2026-07-27/07-story-p1-02-02-double-fault.md`](../../session/hand-2026-07-27/07-story-p1-02-02-double-fault.md)

## Description

The safety net under the net: a TSS with a dedicated IST stack so `#DF` (and a `#PF` raised *during* fault handling) lands on a known-good stack instead of cascading into a triple-fault reset — the "TSS/IST" item Handover 32 explicitly named and left open as `LE-04`. The double-fault handler is terminal-but-reporting: it records what it can (spoor + a UART report + a QEMU-visible exit under Tier 0) and halts, because a double fault means the primary fault path itself is compromised.

## Depends on

`STORY-P1-02-01`.

## Acceptance criteria (finalized 2026-07-27, when the Story started)

1. **A Tier 0 fixture that deliberately destroys the kernel stack reaches the IST-backed `#DF` handler**, and proves it did so by checking that the handler's *own* stack pointer lies inside the reserved `#DF` stack — not merely that it produced output, which a lucky non-IST handler could also do. The frame's saved `RSP` is checked against the destroyed address, so the evidence shows the fault came from the broken stack.
2. **The contrast is observed, not assumed.** What the same fixture does with the IST removed — a triple fault: QEMU resets, never reaches the debug-exit port, and the harness sees no kernel verdict — is recorded in the Report.
3. **IST stack size is a named constant with documented rationale**, counted against `kernel::capacities`' static budget, with exactly one slot populated (`#MC` deliberately gets none: no Tier 0 way to raise a machine check, so wiring it would put an unexercised gate in a fault path).
4. **The primary `#PF`/`#GP`/`#UD` path is demonstrably unaffected** — `STORY-P1-02-01`'s fixture still passes, as does every other Tier 0 fixture, since the GDT and TSS are installed on the **real boot path** too, not only in the fixture that tests them.
5. **`Disposition::of` is not extended and gains no vector-dependent branch.** A double fault is not a disposition question; it gets its own entry symbol and its own audit function, and the audit records the faulting context for *attribution* while never claiming containment in either context.

Note the criterion that changed shape from the draft. The draft asked for "a distinguishable failure exit". What the fixture actually produces on success is a **`Success`** debug-exit code plus a `TOS64-RESULT/1` line, because reaching the `#DF` handler at all is the property under test; the *failure* being distinguished from is the no-IST triple fault, which produces no debug-exit code at all. Distinguishability was the point; which side of it carries the success code was not.

## Tests

- [`TEST-P1-02-02-A`](../tests/TEST-P1-02-02-A.md) — nine clauses: TSS layout, additive GDT, IST index typing, `#DF` capture through its own entry point, no disposition, the Tier 0 escalation, primary-path regression, budgeted capacity, and what this explicitly does not establish.

## Goals verified

G-SEC-2 (fault-path survivability), G-SEC-14.

## What this Story does not close

`#MC` has no IST. Nothing survives a fault inside the `#DF` handler itself. `RSP0` is zero and unused — there is still no privilege boundary. And `FEAT-P1-02` cannot exit on these two Stories alone: `LE-17`, the fault-latency baseline its exit criteria require, remains open.
