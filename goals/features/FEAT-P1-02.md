# FEAT-P1-02 — Real CPU Exception Handling & Fault Containment

Status: **Specified — no Story started**
Epic: [`EPIC-P1`](../epics/EPIC-P1.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

Replace `STORY-P0-04-02`'s fail-closed *diverge-and-report* default (correct, but terminal for the whole system) with genuine exception handling (Goal **G-SEC-2** groundwork; Handover 33's priority 2): real `#PF`/`#GP`/`#UD` handlers that capture the faulting context, decide **terminate-the-faulting-task vs. resume** under an explicit, fail-closed policy, record every fault as a spoor, and keep the rest of the system running. Plus the double-fault safety net (TSS/IST stack switching) Handover 32 explicitly left open — a kernel that handles faults must survive a fault *in* its fault path.

This Feature is deliberately sequenced **before** active per-task address spaces (`FEAT-P1-03`), preserving Handovers 32/33/35's standing reasoning: a live CR3 switch with no real fault handler behind it is strictly more dangerous than the current identity map.

## Crate(s) involved

`os/src/hal-x86_64/` (handler entry stubs, TSS/IST, fault-frame capture), `os/src/kernel/` (fault policy, task termination, spoor emission)

## Depends on

`FEAT-P1-01` (fault-path latency is measured, not guessed — the handler's own overhead gets a baseline).

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P1-02-01`](../stories/STORY-P1-02-01.md) | `#PF`/`#GP`/`#UD` handlers: context capture, terminate-vs-resume policy, spoor audit | Specified |
| [`STORY-P1-02-02`](../stories/STORY-P1-02-02.md) | Double-fault safety: TSS/IST stack switching, fault-in-fault-path survival | Specified |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0/C1** · subject **C1/C2/C3** · boundary tests **BND-04, -17, -20**.

A fault is a hostile input: fault frames, error codes, and faulting addresses come from arbitrary (possibly attacker-steered) execution and are parsed defensively. Fault handling terminates or resumes the *faulting context only* — it never widens authority, never leaks another domain's state into the fault record beyond class/actor/action/outcome, and every decision is a spoor (`PD-12` fault containment; `PD-13` teardown precedes reuse).

## Exit criteria

Both Stories **Verified** at Tier 0: deliberate `#PF`/`#GP`/`#UD` fixtures prove capture-terminate-continue and capture-resume paths; a deliberate kernel-stack-destroying fixture proves the IST path survives; fault latency has a `FEAT-P1-01` baseline.
