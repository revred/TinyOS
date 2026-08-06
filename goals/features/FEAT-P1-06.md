# FEAT-P1-06 — Deterministic Actuation Proof (G-PA-1 Flagship Path)

Status: **In progress — 1 of 2 Stories Verified at Tier 0 (mechanism half, 2026-07-29); the Feature is NOT complete** (assurance `baseline-debt`). Three of the four halves of the exit criteria below remain open and are gated by different things: the *cheap-denial* half is measured and **refused** rather than passed (55% run-to-run p99 CV), the *under-hostile-load* half is gated by `FEAT-P1-05`, which has no Story started, and the *bound* half by hardware plus a secure-world qualification record no platform holds (`ADR 0005`, `LE-09`). `STORY-P1-06-02` was added 2026-08-06 and is `In progress` — its criterion 4 needs one board run. A Feature whose Stories are Verified is not thereby Complete, and saying so here is cheaper than discovering it at a release gate
Epic: [`EPIC-P1`](../epics/EPIC-P1.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

The Epic's integration exit (Goal **G-PA-1**): one end-to-end path from *decision* (an RT task computes an actuation command) to *actuation* (the command reaches an output boundary — under Tier 0, a measurable I/O port/MMIO write standing in for a real actuator line) with a **scheduler-enforced** worst-case latency bound: the actuation task's WCET budget and deadline are declared, the deadline monitor (`FEAT-P1-04`) enforces them, a deliberate overrun trips the declared fault policy, and the measured decision-to-actuation distribution (via `FEAT-P1-01`) sits inside the declared bound with the margin recorded. "Enforced by the scheduler, not merely observed in testing" is the goal's own wording — the proof must show the *enforcement* firing, not only clean runs.

This is the primitive the `G-PA-8` 5-axis CNC flagship milestone (a cross-`EPIC-P0`–`P3` checkpoint, per the backlog) eventually stacks G-code parsing, motion planning, and real I/O onto. Here it is one task, one output, one bound — deliberately minimal, so the determinism claim is attributable to the kernel rather than to application structure.

## Crate(s) involved

`os/src/kernel/` (the actuation task fixture, budget declaration), `os/src/hal-x86_64/` (the bounded output primitive), `os/src/xtask/` (end-to-end measurement)

## Depends on

`FEAT-P1-04` (deadline enforcement is the claim), `FEAT-P1-01` (the measurement), `FEAT-P1-02` (the overrun path). Composes with `FEAT-P1-03` when both are done (actuation from a task in its own address space) — worth demonstrating but not gating on.

**`FEAT-P1-05` — added 2026-07-29, and it gates *exit* rather than start.** The exit criteria below have always required the measured distribution to hold *"under `FEAT-P1-05`'s hostile load"*, but this list did not name it, so the Feature read as startable-and-finishable on three complete dependencies when one of the four is `Specified — no Story started`. **The Story here can begin without it; it cannot be Verified without it.** Stated here because a dependency that appears only in an exit criterion is one a planning session will miss — the same unwritten-dependency shape as `LE-48` and as the AArch64 binary crate [Handover 31](../../session/hand-2026-07-28/31-qemu-virt-fixture-scoping.md) found.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P1-06-01`](../stories/STORY-P1-06-01.md) | Bounded decision-to-actuation path: declared budget, enforced deadline, measured distribution, demonstrated overrun trip | **Verified** (Tier 0, mechanism half, 2026-07-29, `REPORT-2026-07-29-02`; assurance `baseline-debt`) |
| [`STORY-P1-06-02`](../stories/STORY-P1-06-02.md) | The actuation path reaches the real-time architecture: an ARM64 `OutputLine` over RP1 bank-0 GPIO 20..27 | In progress — host half Green, criterion 4 needs a board |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0/C1** · subject **C1/C2** · boundary tests **BND-15, -16, -17**.

Actuation authority is explicit: only the declared actuation task can reach the output primitive (no ambient path to it), the enforcement decision on overrun is a spoor, and no load elsewhere in the system (see `FEAT-P1-05`) may widen the actuation latency distribution beyond its declared bound — that cross-Feature composition is part of the evidence, not an afterthought.

## Exit criteria

The Story **Verified** at Tier 0: measured distribution inside the declared bound under idle *and* under `FEAT-P1-05`'s hostile load, enforcement demonstrated by a deliberate overrun tripping its policy, and the standing hardware-tier debt named — a QEMU-measured bound is the mechanism's proof, the boards' numbers are the product's, and the Report must keep that distinction explicit.

### Amended 2026-07-29 by [`ADR 0005`](../../docs/adr/0005-arm64-real-time-tier-is-conditional-on-secure-world-qualification.md) — the *bound* half of these criteria is no longer closable at Tier 0

This Feature was written 2026-07-26. **`ADR 0005` landed on 2026-07-28 and `STORY-P0-01-07` then put a machine behind it**; this document predates both and cited neither. **Nothing above is withdrawn** — the sentence *"a QEMU-measured bound is the mechanism's proof, the boards' numbers are the product's"* already drew the right distinction, and this section sharpens it into something a Report can be held to.

**Why it matters here more than elsewhere.** `STORY-P1-06-01` selects domains `D03` and `D05`, and `PERF-D03-G04`/`PERF-D05-G04` are titled *"observed maximum and **WCET bound**"* with the target *"declared bound has ≥ 20% evidence margin"*. That is a `G04`-class bound claim by name, and under `ADR 0005` a worst-case bound is quotable **only from a platform holding a secure-world qualification record**. Zero platforms hold one. So:

- **What this Feature can still establish at Tier 0, in full:** the *mechanism*. The path from decision to actuation exists, the budget and deadline are declared, the deadline monitor **enforces** them, a deliberate overrun **trips the declared policy**, and the distribution is measured and recorded with its margin. `G-PA-1`'s own wording — *"enforced by the scheduler, not merely observed in testing"* — is about enforcement firing, and **enforcement firing is fully provable here.**
- **What it cannot close at Tier 0:** `PERF-D03-G04` or `PERF-D05-G04`. `xtask`'s bound-provenance check (`os/src/xtask/src/bound_provenance.rs`) **refuses a `G04` row sourced from Tier 0 or from an unqualified platform**, and it is right to. A session that measures, writes the Report and then files the row will be refused *at the end* of the work rather than at the start — which is the whole reason this section exists.
- **Consequence for the Story's scope.** Verified-at-Tier-0 means *the mechanism and its enforcement are proven and the numbers are recorded as Tier 0 mechanism evidence*. The bound itself is **stated debt against `LE-09`**, and the Report must say so in those words rather than presenting a QEMU distribution as a satisfied margin.

**The positive control is not optional here either.** A deliberate overrun that trips the policy is this Feature's detector, and `ADR 0005`'s trap applies unchanged: a clean run proves nothing until the enforcement has been *seen* to fire. That is already an exit criterion — *"the proof must show the enforcement firing, not only clean runs"* — and it predates the ADR by two days, which is the third independent arrival at that rule in this repository.

### Re-worded 2026-08-06 — three halves, each naming its own gate

`09A` §11 asked whether this Feature is technically complete with evidence and found the answer
is *"the claimed half is; the Feature is not"* — but the exit criteria above state the three
halves in one sentence, so no session can tell which one it has just discharged. Split, with the
gate each closes against:

| # | Half | What closes it | Blocked by |
|---|---|---|---|
| 1 | **Mechanism and enforcement at Tier 0** | `STORY-P1-06-01` functionally `Verified` + `REPORT-2026-07-29-02` | **Done, 2026-07-29.** Re-derived and reproduced on the current tree 2026-08-05, seven days and one `TOS64-*` rename later |
| 2 | **Cheap, state-free denial** | `PERF-D03-G20` | **Measured and refused**, not unmeasured — 55% run-to-run p99 CV. The refusal is now a `refused` row in `guardrail-evidence.tsv` rather than Report prose only |
| 3 | **Under hostile load** | `PERF-D05-G19`, `PERF-D05-G21` | `FEAT-P1-05`, whose mechanism is unbuilt — see that Feature's Story scope section |
| 4 | **The bound** | `PERF-D03-G04`, `PERF-D05-G04` | `ADR 0005` + zero qualified platforms. **The owner's decision, not this Feature's** |

**Three things this Feature's evidence does not cover, all found by checking rather than reading,
and none of them blocked by the owner or by an unbuilt Feature:**

- **Every line of it is Tier 0 QEMU `x86_64`.** `ADR 0004` makes ARM64 the real-time tier, and
  `os/src/hal-arm64/src/` contained no `actuation` module and no `OutputLine` implementation
  until 2026-08-06 — the arch-neutral trait was built so a Pi 5 backend could slot in, and
  nothing had. The Report is careful that *the boards carry the product's numbers*; what was
  written nowhere is that **the mechanism itself had never run on the architecture this project
  designates as real-time.**
- **The path is fixture-only** and has never run on the shipping `os` image — the `LE-20` shape.
  `LE-85`.
- **`STORY-P1-06-01` held zero rows in `guardrail-evidence.tsv`** until 2026-08-06, so in the
  register this Feature was indistinguishable from one nobody started. It now holds one, and
  that one is a *refusal* — which is the accurate record and does not move the published count.
