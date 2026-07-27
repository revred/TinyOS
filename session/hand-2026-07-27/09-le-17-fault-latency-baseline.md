# Handover 09 — `LE-17` Closed: The Fault-Latency Baseline

Follows: [`08-epic-p1_5-deploy-loop-transport-decision.md`](08-epic-p1_5-deploy-loop-transport-decision.md). Implementation session, per that handover's own "next session — start here" directive: `LE-17` was the actual next implementation work, unchanged from Handover 07 through Handover 08.

## What this session did

Closed `LE-17` — the fault path had no `FEAT-P1-01` timing baseline, which `FEAT-P1-02`'s own exit criteria required and `TEST-P1-02-01-A` clause 8 named explicitly as follow-on work rather than something quietly skipped.

`os/src/kernel/src/fixture_measure.rs` gained a sixth measured phase, `phase_fault_latency` / `fault_ud2_capture_terminate_kernel_context` (domain `D02`): a task timestamps itself via the real `CycleSource`, then raises a genuine `#UD` (`ud2` — the same architecturally-guaranteed instruction `fixture_fault`'s own `#UD` victim uses, chosen for the identical reason: no dependency on GDT or page-table shape that could drift). The fixture installs its own `tinyos_fault_entry`, which reads the stop timestamp as early as possible, runs the real `kernel::fault::Disposition::of`/`audit` pair (the same functions the production and `fixture_fault` entry points call — not a stand-in), records the corrected cycle count, and escapes back to the driver loop via the same context-switch pattern `fixture_fault` uses to survive past a fault it deliberately caused. 1,100 iterations run per measurement pass (100 warmup + 1,000 sampled), each needing a freshly reinitialized `Context` because a fault handler never resumes the context it interrupted.

`main.rs`'s default `tinyos_fault_entry` is now excluded under `fixture-measure` (it previously applied there unnoticed, since that fixture had never faulted before) so the fixture's own handler is the one actually linked in.

A Tier 0 baseline was committed: `D02/fault_ud2_capture_terminate_kernel_context`, min=1120 / p50=1406 cycles, over 5 release-profile runs, 2026-07-27. `os/src/xtask/src/gate.rs`'s pinned row-count test moved 5 → 6 to match. Full detail, design rationale, and verification commands: [`REPORT-2026-07-27-07`](../../goals/reports/REPORT-2026-07-27-07.md).

**`FEAT-P1-02` is now functionally complete.** All three of its exit-criteria clauses are met (capture-terminate-continue, IST double-fault survival, fault-latency baseline). It still carries assurance state `baseline-debt`, not `verified` — Tier 0 only, no hardware-tier evidence (`LE-09`) — the same distinction every other Story in this Epic already draws.

## What is honestly not true yet

No `PERF-D02` guardrail closes — a committed baseline lets the regression gate detect drift in this path, nothing more. No hardware-tier evidence exists for this number; `LE-09` stays open. No new fault-containment coverage was added — `fixture_fault` already proves task-only containment, and this phase measures only the latency of the same path, using `FaultingContext::Kernel` rather than a real scheduled task (a deliberate simplification, since `kernel::fault::audit` computes identical cost for both arms — the *containment behavior* is `fixture_fault`'s claim, not this phase's).

## The finding: `--update-baseline` rewrote five rows it was not asked to measure

Found while reviewing this session's diff, after the Report was written, and recorded here because the Report does not mention it. Recording the new `D02` row **overwrote all five pre-existing baseline rows**, and the rewrites are not small:

| Metric | Committed earlier 2026-07-27 | After `--update-baseline` | Change |
|---|---|---|---|
| `D04/context_switch_yield_roundtrip_2switches` | 216 / 226 | 404 / 432 | **~1.9× slower** |
| `D05/dispatch_select_highest_priority_ready` | 74 / 76 | 148 / 174 | **~2.3× slower** |
| `D05/dispatch_run_once_cooperative_round` | 420 / 446 | 510 / 532 | ~1.2× slower |
| `D07/pool_u64x4_alloc_denied_exhausted` | 14 / 16 | 30 / 56 | ~3.5× slower |
| `D07/pool_u64x64_alloc_free_round_trip` | 66 / 70 | 8 / 14 | **~5× *faster*** |

Two candidate explanations, and the evidence separates them.

It is **not** this session's code. The only functional change touching those five phases is `remap_and_mask_pic()` → `init_faults_only()`, and the Report's argument that this is inert for them holds: it arms no APIC timer, never executes `sti`, and the IDT it loads is reached only by the deliberate `#UD`. Their measurement code is byte-identical apart from progress strings. Decisively, [Handover 07](07-story-p1-02-02-double-fault.md#the-finding-the-timing-gate-does-not-pass-on-this-host) already recorded `D04` p50 at **382, 402 and 422** against the 226 baseline earlier the same day — on the *unmodified* fixture, before any of this session's changes existed. The drift predates the code.

It **is** the host, which is exactly what `LE-18` says. The five-run recording carried `overhead_cycles=70`; a `measure --runs=1` taken while reviewing this reported `overhead_cycles=256`, with `D05/dispatch_select` at p50 **32,374** cycles against its own freshly-recorded 174-cycle baseline. The noise floor is moving by orders of magnitude across the day.

That inflated calibration also explains the one change that looks like an improvement. `Calibration` subtracts the measured back-to-back read cost from every sample; a noisy host inflates that subtrahend, which barely dents a long operation but can swallow most of a short one. `D07/pool_u64x64_alloc_free_round_trip` at min 8 cycles is not a five-fold speedup in `Pool::alloc` — it is roughly a 78-cycle measurement with roughly 70 cycles subtracted from it.

So the baseline file now mixes one honestly-recorded new row with five re-recorded under precisely the conditions [Handover 07 named as unfit](07-story-p1-02-02-double-fault.md#loose-ends-register-canonical-as-of-this-handover): *"Not 'loosen the tolerance' and not 're-record from a noisy host' — either makes the gate quieter without making it better."* Re-recording from a noisy host is what happened, as a side effect rather than a decision — `--update-baseline` offers no way to add one metric without rewriting every metric measured alongside it.

That side effect is the defect, filed as **`LE-19`**. The consequence is that the gate will now pass more easily than it should on four metrics, fail more easily on one, and nobody reading the file can tell which rows were deliberate. **Part (a) is done as of this handover**: the five rows are reverted to their earlier (2026-07-27, pre-`LE-17`) values, and only the new `D02` row is kept — `goals/performance/baselines/tier0-x86_64.tsv` now carries exactly one deliberately-recorded row alongside the five it inherited unchanged. Part (b) — giving the command a way to refresh a single named metric, with the test that would have caught this — is unowned and still open.

## Loose-ends register (canonical as of this handover)

Carried forward from [Handover 08](08-epic-p1_5-deploy-loop-transport-decision.md#loose-ends-register-canonical-as-of-this-handover); one closed, one new.

| ID | Loose end | Origin | Owner / fix path | Status |
|---|---|---|---|---|
| LE-01 | Priority-inheritance behavioral half never verified | `STORY-P0-02-03` | `STORY-P1-04-01` criterion 2 | Open — owned |
| LE-02 | WCET enforcement has no timer and no watchdog behind it | `STORY-P0-02-04` | `STORY-P1-04-02` | Open — owned |
| LE-03 | No real fault handling for the remaining vectors | Handover 32 | `FEAT-P1-02` | Unchanged — `#XF` (19), `#MC` (18), and every other vector still reach the shared fail-closed default |
| LE-04 | No TSS/IST; a fault during fault handling triple-faults | Handover 32 | `STORY-P1-02-02` | Closed (Handover 07) |
| LE-05 | `exec::AddressSpace` built but never installed; system runs all-RWX identity-mapped | `STORY-P0-05-02` | `FEAT-P1-03` | Open — owned |
| LE-06 | `pool-bench` was a divergent sibling harness | Handover 35 | `STORY-P1-01-01` | Closed |
| LE-07 | CI has never been observed running any of this work | Standing since Handover 07 | Phase-independent | Closed |
| LE-08 | I/O APIC device-IRQ routing deferred (local APIC only) | `STORY-P0-04-02`/`-03` | First Story routing a device IRQ | Open — deferred with trigger |
| LE-09 | Pi 5 (ARM64) is the short-term hardware, but no ARM64 bring-up slice or deploy path exists | Handover 37 directive 1 | Option B with the carve-out ([Handover 03](03-le-09-arm64-pi5-slice-proposal.md)) | Narrowed (Handover 08) — deploy-path transport decided; bring-up slice unchanged |
| LE-10 | ECAM/MCFG config access and PCI bridge traversal deferred | `STORY-P0-04-03` | First Story needing extended config space | Open — deferred with trigger |
| LE-11 | `Context::new` seeds task `rflags` with `IF` set | `STORY-P1-01-01` | `FEAT-P1-02` | Open — mitigated, not fixed |
| LE-12 | CI's clippy never lints target-only fixture code | `STORY-P1-01-01` | Per-fixture target clippy in the CI lint job | Open — unowned, backlog behind it is zero |
| LE-13 | Measurement ran dev-profile binaries | `STORY-P1-01-01` | `STORY-P1-01-02` | Closed |
| LE-14 | `context::switch` saves no SSE/x87 state | `STORY-P1-01-01` / ADR 0003 | `FEAT-P1-04` | Open — owned |
| LE-15 | The AArch64 generic timer is a 54 MHz system counter | `STORY-P1-01-03` | Decide when a board exists | Open — owned |
| LE-16 | The Tier 0 timing gate can only detect regressions of ~1.6x or worse | `STORY-P1-01-02` | Only a hardware tier fixes it (`LE-09`) | Open — owned |
| LE-17 | The fault path has no timing baseline | `STORY-P1-02-01` | Add a fault-latency phase to `fixture_measure` | **Closed (this handover)** — `REPORT-2026-07-27-07` |
| LE-18 | The timing gate is host-condition-sensitive | `STORY-P1-02-02` | Needs a decision about what baselines are *of* | Open — unowned, needs a Story; **sharpened twice this session**: reconfirmed directly (a `check-timing-regression --runs=3` immediately after re-recording still regressed one metric on its first attempt and passed cleanly on an immediate re-run, no code change in between), and now shown able to corrupt the baseline *file* rather than only the gate's verdict — see the finding above |
| **LE-19** | `--update-baseline` rewrites **every** measured row, so adding one metric silently re-records all the others. This session re-recorded five rows under the exact host conditions `LE-18` names as unfit, with no record of it in the Report | This handover | Two parts: **(a)** revert the five rows to their earlier values and keep only the new `D02` row — **done, this handover**; **(b)** give the command a way to add or refresh a *named* metric without touching the rest, so the file always states what was deliberate — with the test that would have caught this | Part (a) closed; part (b) open — unowned |

## Next session — start here

1. **`FEAT-P1-03`** (active per-task address spaces, W^X, generation-safe teardown) is the actual next implementation work — `FEAT-P1-02` no longer has any open item ahead of it, and `FEAT-P1-03`'s own `Depends on` section already names `FEAT-P1-02` as its hard-ordering prerequisite, now satisfied. [`STORY-P1-03-01`](../../goals/stories/STORY-P1-03-01.md) criterion 2 wants a measured same-space-versus-cross-space `D04` delta against the now-restored, honest baseline.
2. `LE-19` part (b) — a way to refresh one named baseline metric without rewriting the rest, with the test that would have caught this session's silent rewrite — is a small, unowned `gate.rs` Story.
3. `EPIC-P1_5` (deploy-loop) still awaits decomposition per Handover 08's transport decision — unchanged, not sequenced against `FEAT-P1-03`.
4. If the user acquires the USB-TTL serial cable discussed for `LE-09`, that unblocks `LE-09` pieces 1/2/5 independent of anything in this handover.
5. **`LE-18`**, if CI reports what this host does. Three commits reached `origin/main` today carrying the first CI run of `--fixture=double-fault` and of the timing gate on a GitHub runner; nothing is yet known about whether a runner behaves like this machine.
