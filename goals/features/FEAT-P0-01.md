# FEAT-P0-01 — Workspace Bootstrap & Walking Skeleton

Status: **Verified** (locally; CI run pending — see linked Reports on each Story/Test)
Epic: [`EPIC-P0`](../epics/EPIC-P0.md)
Introduced in: [`session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md`](../../session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md)

## Description

The first milestone, per [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md#delivery-strategy-walking-skeleton-first): the smallest possible end-to-end slice through the whole pipeline, proven before any real kernel logic exists. Success is `cargo run -p xtask -- qemu-x86_64` booting an empty `kernel` crate to a clean halt with no panic, with CI reporting green on format, lint, the crate-size ceiling check, and `missing_docs`.

## Crate(s) involved

`os/src/kernel/` (empty shell), `os/src/hal/` + `os/src/hal-x86_64/` (empty shells), `os/src/xtask/`, `os/targets/x86_64-tinyos.json`.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P0-01-01`](../stories/STORY-P0-01-01.md) | Empty kernel crate boots in QEMU x86_64 and halts cleanly | Verified |
| [`STORY-P0-01-02`](../stories/STORY-P0-01-02.md) | CI pipeline runs format/lint/crate-size-ceiling checks on every PR | Verified |
| [`STORY-P0-01-03`](../stories/STORY-P0-01-03.md) | `xtask qemu-x86_64` command builds and launches the kernel under QEMU | Verified |
| [`STORY-P0-01-04`](../stories/STORY-P0-01-04.md) | The harness is held to the discipline it enforces: panics and unrouted interrupts report themselves, both exit-code holes close, and every fixture is provably run by CI | Verified (Tier 0 + Host; assurance `baseline-debt`) |
| [`STORY-P0-01-05`](../stories/STORY-P0-01-05.md) | A guardrail evidence register, and the first release gates that need no hardware: `G11` recorded for ten domains on a compiler-enforced no-heap property | Verified (Host; assurance `baseline-debt`) |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0/C1** · subjects **C0–C4** · boundary tests **BND-01, -02, -03, -17, -18**.

That row also selects this Feature’s [`PD-*`](../security/protection-domain-contracts.tsv) and [`RCG-*`](../security/code-admission-gates.tsv) Security Charter obligations. Every Test repeats the exact selections and CI rejects drift.

Boot and governance may verify, measure, transfer, or reject; they never create runtime ambient authority. C0 must expose no reusable runtime command surface after handoff, C1 must link no complex hostile-format parser, and CI must reject incomplete class, Feature, Story, security-control, or boundary-test contracts. Required evidence includes authenticated handoff, runtime-reentry denial, privileged-parser absence, complete mappings, and negative build-profile surface scans.

## Exit criteria

All Stories reach **Verified**. Met locally on 2026-07-26 for the first three (see `goals/reports/REPORT-2026-07-26-01` through `-03`); this unblocked `FEAT-P0-02` (scheduler), since scheduler work needs a booting kernel to run inside.

**`STORY-P0-01-04` (2026-07-28) retired the assurance debt in the harness itself**, and found that this Feature's exit had been claimed on weaker evidence than it read as: nine of the twenty-three Tier 0 fixtures — including `context-switch`, `idt-apic-timer` and `address-space`, all named by owning Test documents claiming Tier 0 evidence — **had no CI step at all**. They passed when run, so the behaviour was sound; what did not exist was continuous evidence for it. All nine now run on every push, both exit-code holes are closed by asserting on serial content rather than on an exit code any failure produces, and a host test fails if the fixture table and the workflow drift apart in either direction.
