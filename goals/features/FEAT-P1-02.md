# FEAT-P1-02 — Real CPU Exception Handling & Fault Containment

Status: **Functionally complete — both Stories functionally Verified (Tier 0 + Host) 2026-07-27, and the fault-latency baseline (`LE-17`) landed the same day; assurance state remains `baseline-debt`, not `verified`, until hardware-tier evidence (`LE-09`) exists**
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
| [`STORY-P1-02-01`](../stories/STORY-P1-02-01.md) | `#PF`/`#GP`/`#UD` handlers: context capture, terminate-vs-halt policy, spoor audit | Verified (Tier 0 + Host; assurance `baseline-debt`) |
| [`STORY-P1-02-02`](../stories/STORY-P1-02-02.md) | Double-fault safety: TSS/IST stack switching, fault-in-fault-path survival | Verified (Tier 0 + Host; assurance `baseline-debt`) |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0/C1** · subject **C1/C2/C3** · boundary tests **BND-04, -17, -20**.

A fault is a hostile input: fault frames, error codes, and faulting addresses come from arbitrary (possibly attacker-steered) execution and are parsed defensively. Fault handling terminates or resumes the *faulting context only* — it never widens authority, never leaks another domain's state into the fault record beyond class/actor/action/outcome, and every decision is a spoor (`PD-12` fault containment; `PD-13` teardown precedes reuse).

## Exit criteria

Both Stories **Verified** at Tier 0: deliberate `#PF`/`#GP`/`#UD` fixtures prove capture-terminate-continue (**done** — `STORY-P1-02-01`); a deliberate kernel-stack-destroying fixture proves the IST path survives (**done** — `STORY-P1-02-02`, with the no-IST triple fault recorded as the contrast); fault latency has a `FEAT-P1-01` baseline (**done** — `LE-17`, [`REPORT-2026-07-27-07`](../reports/REPORT-2026-07-27-07.md): a sixth `fixture_measure` phase times a real `#UD` through the fixture's own fault entry, and a committed Tier 0 baseline exists at `D02/fault_ud2_capture_terminate_kernel_context`).

All three exit-criteria clauses are now met. This Feature is functionally complete; it still carries assurance state `baseline-debt` rather than `verified` (Tier 0 only, no hardware-tier evidence — `LE-09`), the same distinction every other Story in this Epic already draws between "the mechanism was demonstrated" and "the guardrail closed."

Note one correction this Feature's first Story forced on its own exit criteria: there is no "capture-resume path" to prove, because no resume case exists to enumerate. `STORY-P1-02-01`'s policy has two arms — terminate the faulting task, or halt when the kernel itself faulted — and the resume arm was deliberately not built rather than built unreachable. See that Story's second acceptance criterion.
