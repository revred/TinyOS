# Handover 06 — `STORY-P1-02-01`: A Fault Stops Being the End of the System

Follows: [`05-story-p1-01-02-timing-gate.md`](05-story-p1-01-02-timing-gate.md). Evidence: [`REPORT-2026-07-27-05`](../../goals/reports/REPORT-2026-07-27-05.md). Opens `FEAT-P1-02`.

## What this session did

Delivered `STORY-P1-02-01`: real `#UD`/`#GP`/`#PF` handlers. Before today every CPU exception reached `STORY-P0-04-02`'s shared fail-closed default — correct, but terminal for the whole machine and silent about what happened. Now a fault is a **captured, attributable, contained event**.

- **Test document first** ([`TEST-P1-02-01-A`](../../goals/tests/TEST-P1-02-01-A.md)), eight clauses.
- **Capture** (`hal_x86_64::fault`): one `FaultFrame` shape for all three vectors — `#UD`'s stub pushes a synthetic error code so nothing downstream needs to know which vectors carry one — with `size_of` and all eight field offsets pinned by host tests against the stubs' push order.
- **Defensive decoding**: `#PF` and `#GP` error codes decode into named bits as pure functions. `CR2` is reported **only** for `#PF`; the type refuses to attach a stale address to another vector.
- **Policy** (`kernel::fault`): two arms, both reachable, both tested — terminate the faulting task, or halt when the kernel itself faulted.
- **Audit**: two spoors per fault (capture, then disposition), via one new `Category` and two new `Action` variants, carrying no address and no error code (`PD-12`).
- **Tier 0 fixture** (`--fixture=fault`), wired into CI the same day.
- **A default handler for every other build**: the normal boot path still halts fail-closed on a fault, but now prints the vector, error code, `RIP`, `RFLAGS`, `RSP` and (for `#PF` only) the faulting address first. That exact situation — a fault on a path with no task — is what cost `STORY-P1-01-01` two debugging cycles as a silent triple fault.

## The load-bearing evidence

```
fixture-fault captured #UD vector=6  error_code=0x0  rip=0x103e11 cr2=0
fixture-fault captured #GP vector=13 error_code=0x18 rip=0x103e45 cr2=0
fixture-fault captured #PF vector=14 error_code=0x0  rip=0x10364f cr2=549755813888
fixture-fault survivor ran 3 times after three contained faults
fixture-fault captured=3 contained=3 fault_spoors=6
TINYOS-RESULT/1 fixture=fault ok=true
```

Three real instructions, three faults, three tasks `Finished`, and a fourth task that ran three times **afterwards**. `0x18` is the out-of-range selector the `#GP` victim loaded; `549755813888` is exactly 512 GiB, the unmapped address the `#PF` victim read; `cr2=0` on the other two is the type system declining to report a meaningless register. The verdict rides `STORY-P1-01-02`'s `TINYOS-RESULT/1` line — the Pi 5's future pass/fail channel getting its second user.

## The finding: Tier 0 sides with the emulator, silently

The `#GP` victim was first a `wrmsr` to reserved MSR `0xFFFF_FFFF` — an architecturally guaranteed `#GP`. **QEMU/TCG accepted the write.** No fault, no capture; the task fell through its own `unreachable!()` and the run died as a panic.

> "Architecturally must fault" and "faults under this emulator" are different claims, and a Tier 0 fixture can only ever check the second.

Same class of gap as `STORY-P1-01-01`'s from the other direction (host-verified code that could not execute on target). Replaced with a segment-selector load past the boot GDT's three-descriptor limit — guaranteed, observed, and it yields a non-zero error code so the decoder is exercised rather than the zero path. Both facts are recorded in the fixture's source so nobody repeats the experiment.

## Two design decisions worth defending

**There is no `Resume` arm, and that is the point.** The Story's second criterion forbids speculative "maybe recoverable" paths. This kernel has *no* recoverable fault case — no demand paging, no copy-on-write, no guard-page growth — so building an unreachable resume arm would put untested code in a fault path. The entry stubs correspondingly save no registers and never `iretq`. This forced a correction to `FEAT-P1-02`'s own exit criteria, which had assumed a "capture-resume path" to prove; that line now says why there isn't one.

**A terminated task is abandoned, not unwound.** With no IST, a same-privilege fault runs the handler on the victim's own stack. The handler marks the task `Finished` and switches to the supervisor context, abandoning the victim's stack mid-frame — sound precisely because that context is never resumed.

## Process note, stated rather than glossed

The Test document came first, and the Tier 0 fixture failed for real reasons twice before it passed. But the two new modules' **unit tests were written in the same pass as their implementations**, not as a recorded Red run — unlike the three preceding Stories, each of which has a Red count in its Report. There is no Red measurement to quote here, and manufacturing one after the fact would be worse than saying so.

## Verification

`cargo test --workspace --lib` 227 (exec 51, hal 13, hal-arm64 12, hal-x86_64 60, kernel 91) · `cargo test -p xtask` 79 · fmt clean · scoped clippy `-D warnings` clean · `--fixture=fault` exit 0 · timing gate still exit 0 · spine 14 Features / 36 Stories / 28 Tests / 34 Reports.

## Loose-ends register (canonical as of this handover)

Carried forward from [Handover 05](05-story-p1-01-02-timing-gate.md#loose-ends-register-canonical-as-of-this-handover); one new item, none closed, two narrowed.

| ID | Loose end | Origin | Owner / fix path | Status |
|---|---|---|---|---|
| LE-01 | Priority-inheritance behavioral half never verified | `STORY-P0-02-03` | `STORY-P1-04-01` criterion 2 | Open — owned |
| LE-02 | WCET enforcement has no timer and no watchdog behind it | `STORY-P0-02-04` | `STORY-P1-04-02` | Open — owned |
| LE-03 | No real `#PF`/`#GP`/`#UD` handling; every fault is terminal diverge-and-report | Handover 32 | `FEAT-P1-02` | **Narrowed 2026-07-27** — those three vectors are now captured, contained and audited (`STORY-P1-02-01`). What remains: `#XF` (vector 19, sharpened by `STORY-P1-01-01`'s SSE work) and every other vector still reach the shared fail-closed default |
| LE-04 | No TSS/IST; a fault during fault handling triple-faults | Handover 32 | `STORY-P1-02-02` | Open — owned, and now the sharpest item here: real handlers exist, so the handler itself is new code that can fault |
| LE-05 | `exec::AddressSpace` built but never installed; system runs all-RWX identity-mapped | `STORY-P0-05-02` | `FEAT-P1-03` | Open — owned. `STORY-P1-02-01` is the containment `FEAT-P1-03` was waiting for |
| LE-06 | `pool-bench` was a divergent sibling harness | Handover 35 | `STORY-P1-01-01` | **Closed 2026-07-27** |
| LE-07 | CI has never been observed running any of this work | Standing since Handover 07 | Phase-independent | **Closed 2026-07-27** |
| LE-08 | I/O APIC device-IRQ routing deferred (local APIC only) | `STORY-P0-04-02`/`-03` | First Story routing a device IRQ | Open — deferred with trigger |
| LE-09 | Pi 5 (ARM64) is the short-term hardware, but no ARM64 bring-up slice or deploy path exists | Handover 37 directive 1 | Option B with the carve-out ([Handover 03](03-le-09-arm64-pi5-slice-proposal.md)) | Open — pieces 3 and 4 delivered; pieces 1, 2 and 5 waited on `FEAT-P1-02`, whose **first half is now done**. Today added fresh evidence for why the board matters: an architecturally-guaranteed fault that TCG simply did not raise |
| LE-10 | ECAM/MCFG config access and PCI bridge traversal deferred | `STORY-P0-04-03` | First Story needing extended config space | Open — deferred with trigger |
| LE-11 | `Context::new` seeds task `rflags` with `IF` set, so switching into a task enables interrupts even with no IDT installed | `STORY-P1-01-01` | `FEAT-P1-02` | Open — **mitigated, not fixed**: the fault fixture loads a real IDT, so `IF` going high is no longer fatal there. The fail-open seam itself is untouched |
| LE-12 | CI's clippy never lints target-only fixture code | `STORY-P1-01-01` | Per-fixture target clippy in the CI lint job. Now a **fourth** unlinted fixture feature (`fixture-fault`) | Open — unowned, needs a Story |
| LE-13 | Measurement ran dev-profile binaries | `STORY-P1-01-01` | `STORY-P1-01-02` | **Closed 2026-07-27** |
| LE-14 | `context::switch` saves no SSE/x87 state | `STORY-P1-01-01` / ADR 0003 | `FEAT-P1-04` | Open — owned |
| LE-15 | The AArch64 generic timer is a 54 MHz system counter (~18.5 ns/tick), so hardware metrics will be quantization-limited | `STORY-P1-01-03` | Decide when a board exists | Open — owned |
| LE-16 | The Tier 0 timing gate can only detect regressions of ~1.6x or worse | `STORY-P1-01-02` | Only a hardware tier fixes it (`LE-09`) | Open — owned, bounded by `LE-09` |
| **LE-17** | The fault path has **no timing baseline**, which `FEAT-P1-02`'s own exit criteria require ("fault latency has a `FEAT-P1-01` baseline"). The harness, the gate and the baselines all exist; the fault path is simply not on a measured path yet | `STORY-P1-02-01` (this handover) | Add a fault-latency phase to `fixture_measure` and a baseline row — small, and it uses machinery that already exists. Needed before `FEAT-P1-02` can exit | Open — owned |

## Next session — start here

1. **`STORY-P1-02-02` — TSS/IST double-fault survival.** `FEAT-P1-02`'s other half and the sharpest item in the register (`LE-04`): real handlers now exist, which means the handler is itself new code that can fault, and today a fault inside it triple-faults with no diagnostic. Needs a TSS, a GDT descriptor for it (`hal_x86_64::boot`'s GDT currently holds exactly three entries — the `#GP` victim above depends on that limit, so changing it changes that fixture), `ltr`, and an IST-backed known-good stack for `#DF`.
2. **`LE-17` — the fault-latency baseline**, small and mechanical, using the harness/gate/baseline machinery that already exists. `FEAT-P1-02` cannot exit without it.
3. Then **`FEAT-P1-03`** (per-task address spaces), which this Story unblocked: a live `CR3` switch with no fault containment behind it was the thing Handovers 32/33/35 kept refusing to do, and the containment now exists.

## What this handover does not do

No hardware ran anything. A fault inside the fault handler still triple-faults. Nothing here establishes a privilege boundary — everything is still CPL 0 in one identity-mapped address space, so "the fault happened in a task" means which context the kernel had switched into, not a hardware ring transition. No `PERF-D02` guardrail closes, and `STORY-P1-02-01` is `baseline-debt`, not `verified`.
