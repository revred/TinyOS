# Handover 10 — `STORY-P1-03-01`: Per-Task `CR3` Switching, Proven Against Two Real Address Spaces

Follows: [`09-le-17-fault-latency-baseline.md`](09-le-17-fault-latency-baseline.md). Implementation session, per that handover's own "next session — start here" directive: `FEAT-P1-03` was the actual next implementation work, unblocked once `FEAT-P1-02` closed.

## What this session did

Implemented and Verified `STORY-P1-03-01` — the mechanism half of `FEAT-P1-03` (active per-task address spaces): a task's `exec::AddressSpace` can now be loaded into `CR3`, with the reload skipped when the incoming task's space is already the one loaded.

**New primitives.** `hal_x86_64::paging` gained `cr3_reload_needed` (a pure, host-tested equality check) and `read_cr3`/`write_cr3` (real `mov cr3` asm, target-only). `exec::AddressSpace::cr3()` exposes the physical PML4 address a caller loads into the register — this kernel's no-higher-half-split memory model means that's simply the address of the caller-owned `pml4` binding, which a host test now pins. `kernel::sched::Tcb` gained an `address_space: Option<u64>` field (`Scheduler::set_address_space`/`Tcb::address_space`), defaulting to `None` so every pre-existing Story's tasks are unaffected. `kernel::context::switch_address_space` wraps the existing `switch` with the compare-and-reload, in that order — the address space must be live before the incoming task's suspended registers resume into it.

**The Tier 0 fixture** (`os/src/exec/src/fixture_address_space_switch_main.rs`, new `address-space-switch-fixture` binary): two genuinely disjoint `AddressSpace` trees (separate `PML4`/frame-pool statics, not two sections in one shared tree), each identity-mapping a low 8 MiB kernel-replica region as RWX so a real `CR3` load into either doesn't immediately fault against its own code/stack. Task A's tree has no entry at all for task B's private virtual address; task A's deliberate read there raises a real `#PF`, contained by the same `kernel::fault` machinery `fixture_fault` already proves (task A terminated, nothing else); task B then runs to completion under its own distinct, hardware-verified `CR3`. Full detail and the finding below: [`REPORT-2026-07-27-08`](../../goals/reports/REPORT-2026-07-27-08.md).

**The finding.** The first several attempts triple-faulted with no diagnostic — `AddressSpace::drop` (built for `STORY-P0-05-02`'s own teardown contract) unconditionally zeroes `pml4`/`frame_pool` the instant an `AddressSpace` value goes out of scope, which was erasing both fixture trees before their `CR3` was ever loaded. Fixed with a documented `core::mem::forget` — a deliberate leak, not a workaround presented as one, since generation-safe teardown that could be used instead doesn't exist yet (`STORY-P1-03-02`'s charge).

**Scope, narrowed honestly from the original draft.** `STORY-P1-03-01.md`'s draft acceptance criteria implied production dispatch-loop wiring and a measured `D04` same-space-vs-cross-space delta. Neither is delivered: wiring `switch_address_space` into `kernel::dispatch::run_once` for every real task needs W^X-correct kernel mappings *shared* across every space (this fixture's own low-memory replica duplicates rather than shares, and is all-RWX — explicitly not that), and there is nothing in production yet to measure a switch-cost delta against. Both gaps are named in the finalized Story, the Test document, and the Report, not left implicit.

**`FEAT-P1-03` is now In Progress** (1 of 2 Stories Verified). It does not exit here — `STORY-P1-03-02` (W^X/NX mappings, generation-safe teardown) is unstarted, and per this Feature's own exit criteria, is what the production dispatch-loop wiring and the D04 measurement both actually depend on.

## What is honestly not true yet

No production task has a dedicated address space — every existing Story's tasks still run on the boot identity map, unaffected. No W^X enforcement exists anywhere; the fixture's own kernel-replica mappings are all-RWX. No teardown exists that is safe to use; the fixture leaks rather than reaching for `AddressSpace::drop`. No `D04`/`D08` timing evidence exists for a real `CR3` switch cost.

## Loose-ends register (canonical as of this handover)

Carried forward from [Handover 09](09-le-17-fault-latency-baseline.md#loose-ends-register-canonical-as-of-this-handover); no items closed, none new.

| ID | Loose end | Origin | Owner / fix path | Status |
|---|---|---|---|---|
| LE-01 | Priority-inheritance behavioral half never verified | `STORY-P0-02-03` | `STORY-P1-04-01` criterion 2 | Open — owned |
| LE-02 | WCET enforcement has no timer and no watchdog behind it | `STORY-P0-02-04` | `STORY-P1-04-02` | Open — owned |
| LE-03 | No real fault handling for the remaining vectors | Handover 32 | `FEAT-P1-02` | Unchanged — `#XF` (19), `#MC` (18), and every other vector still reach the shared fail-closed default |
| LE-04 | No TSS/IST; a fault during fault handling triple-faults | Handover 32 | `STORY-P1-02-02` | Closed (Handover 07) |
| LE-05 | `exec::AddressSpace` built but never installed; system runs all-RWX identity-mapped | `STORY-P0-05-02` | `FEAT-P1-03` | **Narrowed this handover** — installation is now a proven mechanism (`STORY-P1-03-01`), but no production task uses it; the system still runs all-RWX identity-mapped by default |
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
| LE-17 | The fault path has no timing baseline | `STORY-P1-02-01` | Add a fault-latency phase to `fixture_measure` | Closed (Handover 09) |
| LE-18 | The timing gate is host-condition-sensitive | `STORY-P1-02-02` | Needs a decision about what baselines are *of* | Open — unowned, needs a Story |
| LE-19 | `--update-baseline` rewrites every measured row, so adding one metric silently re-records all the others | Handover 09 | Part (a) done (Handover 09); part (b) — refresh a named metric without touching the rest, with the test that would have caught this | Part (a) closed; part (b) open — unowned |

## Next session — start here

1. **`STORY-P1-03-02`** (W^X/NX kernel + task mappings, generation-safe teardown) is the actual next implementation work — it is what lets `switch_address_space` ever be wired into the real dispatch path, and what `STORY-P1-03-01`'s own deferred `D04` measurement depends on. Start from `AddressSpace::drop`'s current non-generation-safe teardown as the concrete thing to replace, since this session's own fixture had to route around it.
2. `LE-19` part (b) — a way to refresh one named baseline metric without rewriting the rest — is a small, unowned `gate.rs` Story, independent of `FEAT-P1-03`.
3. `EPIC-P1_5` (deploy-loop) still awaits decomposition per Handover 08's transport decision — unchanged, not sequenced against `FEAT-P1-03`.
4. If the user acquires the USB-TTL serial cable discussed for `LE-09`, that unblocks `LE-09` pieces 1/2/5 independent of anything in this handover.
