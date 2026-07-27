# FEAT-P1-03 — Active Per-Task Address Spaces, W^X & Teardown

Status: **Complete — all three Stories Verified (Tier 0 + Host): `STORY-P1-03-01` 2026-07-27, `STORY-P1-03-02` and `STORY-P1-03-03` 2026-07-28. Every exit criterion met, including the `D04` switch-cost measurement deferred twice. Assurance state `baseline-debt` (Tier 0 evidence only; `LE-09` open).**
Epic: [`EPIC-P1`](../epics/EPIC-P1.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)

## Description

Make **G-SEC-2** ("process memory is private *by construction*") an active runtime fact instead of dormant machinery: `EPIC-P0` built `exec::AddressSpace` page tables that are constructed and verified but never installed — every task still runs on the boot identity map, all-RWX. This Feature switches `CR3` per task in the context switch, replaces the all-RWX identity map with W^X/NX mappings (executable memory never writable, writable never executable — the Security Charter's `PD-04` executable sealing, boundary test `BND-05`), and implements generation-safe teardown (`PD-13`: revoke, wipe, advance generation before any reuse). This closes three named items on the Security Charter's runtime-evidence gap list: active per-task address spaces, executable sealing, and teardown.

## Crate(s) involved

`os/src/hal-x86_64/` (CR3 install, kernel-mapping W^X split), `os/src/kernel/` (per-task address-space handle in the TCB, context-switch integration, teardown), `os/src/exec/` (`AddressSpace` becomes the *installed* space, not just a verified artifact)

## Depends on

`FEAT-P1-02` (**hard ordering** — no live CR3 switch before a real fault handler exists, per Handovers 32/33/35), `FEAT-P1-01` (context-switch-with-CR3 cost gets a measured baseline against the D04 budget).

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P1-03-01`](../stories/STORY-P1-03-01.md) | Per-task `CR3` switching mechanism, proven against two real address spaces | Verified (Tier 0 + Host; assurance `baseline-debt`) |
| [`STORY-P1-03-02`](../stories/STORY-P1-03-02.md) | W^X/NX kernel + task mappings, shared kernel directories, loader-alias sealing, generation-safe teardown — and the first real scheduled task | Verified (Tier 0 + Host; assurance `baseline-debt`) |
| [`STORY-P1-03-03`](../stories/STORY-P1-03-03.md) | IAT resolution (granted calls callable, refused calls trapped at a named address), the `os` system image, and the `D04` switch-cost measurement | Verified (Tier 0 + Host; assurance `baseline-debt`) |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0/C1** · subject **C1/C2/C3/C4** · boundary tests **BND-04, -05, -20**.

Cross-domain reads/writes/executes/remaps must *fault* (landing in `FEAT-P1-02`'s handlers), not merely be absent from the happy path — the adversarial fixtures actively attempt them. No mapping is ever writable and executable; no teardown-freed frame is reusable before wipe + generation advance; the boot identity map survives only as the early-boot bootstrap, never as a running task's view.

## Exit criteria

Both Stories **Verified** at Tier 0 with adversarial evidence:

- A task provably cannot touch another task's memory, the attempt faulting and being contained by `FEAT-P1-02` — **done, `STORY-P1-03-01`**, as a proven mechanism; **and now as a production integration**, since `STORY-P1-03-02`'s `dispatch::run_once_in_space` installs per-task address spaces on the path a real task is actually scheduled through.
- W^X violations fault in both directions, with the enforcement bits (`CR0.WP`, `EFER.NXE`) genuinely enabled rather than assumed — **done, `STORY-P1-03-02`**. The boot identity map now survives only as the bring-up bootstrap, exactly as this Feature's containment contract requires: it is retired at run time for a W^X kernel tree built from the linker's own section symbols, shared at page-directory granularity into every space.
- Teardown-then-probe proves generation safety (revoke, wipe, advance, then a *task* probing a stale mapping faults) — **done, `STORY-P1-03-02`**.
- The `D04`/`D08` timing baselines absorb the `CR3`-switch cost — **done, `STORY-P1-03-03`**. Deferred twice for an honest reason (nothing in the dispatch path installed a per-task address space, so any number would have been fixture overhead misreported as a scheduling cost), measurable once `STORY-P1-03-02`'s integration existed, and now measured: a dispatch round costs ~276 cycles when the selected task's space is already loaded and ~7,452 when it is not. **That delta is a Tier 0 upper bound dominated by TCG's own TLB-flush emulation, not a hardware cost**, and it is deliberately reported rather than gated — thresholding it would gate the emulator. It is also the sharpest argument this Feature produced for `LE-09`: a case where Tier 0 cannot give the right order of magnitude.

Beyond the stated criteria, `STORY-P1-03-03` closed the gap that made the containment in `STORY-P1-03-02` weaker than it looked — an unpatched IAT meant a refused capability faulted at an image-derived RVA by arithmetic accident rather than by decision — and moved the shipping image to a top-level `os` binary whose real boot path actually loads and schedules a task, which is what this Feature's `G-SEC-2` claim ("process memory is private *by construction*") needs in order to be about something the system really runs.
