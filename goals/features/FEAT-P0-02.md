# FEAT-P0-02 — Preemptive Priority Scheduler Core

Status: **Verified — 4/4 Stories Verified**
Epic: [`EPIC-P0`](../epics/EPIC-P0.md)
Introduced in: [`session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md`](../../session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md); decomposed in [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)

## Description

The preemptive, priority-based scheduler with bounded interrupt latency and priority inheritance/ceiling protocols described in [`README.md`](../../README.md#1-a-real-multitasking-rtos-core) Design Pillar 1, and required by Goal **G-RT-1**.

## Crate(s) involved

`os/src/kernel/`

## Depends on

`FEAT-P0-01` (needs a booting kernel to run the scheduler inside).

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P0-02-01`](../stories/STORY-P0-02-01.md) | Task creation and priority assignment | Verified |
| [`STORY-P0-02-02`](../stories/STORY-P0-02-02.md) | Context switch | Verified |
| [`STORY-P0-02-03`](../stories/STORY-P0-02-03.md) | Priority inheritance on lock contention | Verified |
| [`STORY-P0-02-04`](../stories/STORY-P0-02-04.md) | WCET budget enforcement | Verified |
| [`STORY-P0-02-05`](../stories/STORY-P0-02-05.md) | Priority-ordered cooperative dispatch loop | Verified |

`STORY-P0-02-01` implemented and Verified in [`session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md`](../../session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md) — entirely host-testable (`os/src/kernel/src/sched.rs`), no QEMU dependency. `STORY-P0-02-02` (context switch) implemented and Verified in [`session/hand-2026-07-26/13-story-p0-02-02-context-switch-implementation.md`](../../session/hand-2026-07-26/13-story-p0-02-02-context-switch-implementation.md) — Tier 0 QEMU-verified (`os/src/kernel/src/context.rs`, `context_switch_fixture.rs`). `STORY-P0-02-03` (priority inheritance) implemented and Verified in [`session/hand-2026-07-26/16-story-p0-02-03-priority-inheritance-implementation.md`](../../session/hand-2026-07-26/16-story-p0-02-03-priority-inheritance-implementation.md) — a new `kernel::lock::PriorityInheritingLock`, host-only (no dispatcher exists yet to verify the behavioral half; see that Story's own scope-resolution note). `STORY-P0-02-04` (WCET enforcement) implemented and Verified in [`session/hand-2026-07-26/17-story-p0-02-04-wcet-enforcement-implementation.md`](../../session/hand-2026-07-26/17-story-p0-02-04-wcet-enforcement-implementation.md) — a new `kernel::wcet::record_tick`, the same host-only, detection-half-only scope `STORY-P0-02-03` established (no timer to drive it, no real watchdog to hand an overrun off to, both concrete open prerequisites restated in that Story's own scope-resolution note). **`STORY-P0-02-05`** (priority-ordered cooperative dispatch loop — not part of this Feature's original 4-Story decomposition, added in the same session `-03` and `-04` independently surfaced the identical missing-dispatcher gap) implemented and Verified in [`session/hand-2026-07-26/18-story-p0-02-05-dispatch-loop-implementation.md`](../../session/hand-2026-07-26/18-story-p0-02-05-dispatch-loop-implementation.md) — a new `kernel::dispatch::run_once` plus `Scheduler::highest_priority_ready`, closing `STORY-P0-02-03`'s own behavioral gap with a real `context::switch`-based dispatch round (see that Story's "Update" note); `STORY-P0-02-04`'s gap (no timer, no watchdog) remains open regardless, since a dispatch loop is not a timer.

**All five Stories now Verified.** One structural gap remains, `STORY-P0-02-04`'s own: no live timer drives `kernel::wcet::record_tick`, and no real watchdog/failsafe subsystem exists to hand a detected overrun off to. `STORY-P0-02-03`'s equivalent gap (no dispatcher to behaviorally prove starvation prevention) is now closed by `STORY-P0-02-05`. `STORY-P0-02-05`'s own dispatch loop is cooperative, not preemptive — true involuntary preemption still needs a timer interrupt / IDT (`STORY-P0-05-02`'s named gap), which is also `STORY-P0-02-04`'s missing timer source. One remaining infrastructure gap, not two independent ones — restated here since it's easy to undercount once `-03`'s half closes.

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C1** · subjects **C2/C3/C4** · boundary tests **BND-04, -15, -16, -17, -20**.

That row also selects this Feature’s [`PD-*`](../security/protection-domain-contracts.tsv) and [`RCG-*`](../security/code-admission-gates.tsv) Security Charter obligations. Every Test repeats the exact selections and CI rejects drift.

The scheduler treats containment class, capability authority, and scheduling criticality as independent fields. Creating or prioritizing a task grants no capability; a lower-trust class cannot starve admitted RT work; and every context switch must carry the complete active address-space and protection state needed to contain a compromised task. Required evidence includes all-class-pair memory denial, priority orthogonality, bounded hostile-load recovery, class-aware spoors, and single-seeded-defect containment.

## Exit criteria

`STORY-P0-02-01` through `-04` all reach **Verified**. **Met**, as of 2026-07-26. (`STORY-P0-02-05` was added mid-Feature and is additionally Verified, beyond this Feature's original exit criteria's own 4-Story scope.)
