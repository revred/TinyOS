# TEST-P1-04-03-A — The Shipping Image Preempts, Charges and Enforces

Status: **Verified (Tier 0 + Host)** — specification written at Story start, before implementation; captures filled in after the runs
Story: [`STORY-P1-04-03`](../stories/STORY-P1-04-03.md)
Tier: Tier 0 QEMU runs of the **shipping `os` binary** on two embedded workloads, plus host unit tests for the runaway image generator, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D03`, `D04`, `D05`
Security controls: `SEC-14`, `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`, `C2`
Boundary tests: `BND-15`, `BND-16`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-08`
Code admission gates: `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

`TEST-P1-04-01-A` and `TEST-P1-04-02-A` proved preemption and WCET enforcement **in fixtures**. Both of those documents say, in their own "what this does not establish" sections, that the shipping image does not do either. This test is the evidence that it now does — and, because a mechanism that is only ever exercised by its own test harness is a mechanism nobody has evidence for, the enforcement half is proven against a real PE64 image that runs away with the CPU, loaded by the same loader through the same capability gate into the same kind of address space as the workload that ships.

**The one property everything else rests on:** the two Tier 0 runs below differ *only* in which image bytes are embedded. The hook, the dispatcher, the loop bound, the declared policy, the address-space handling and every assertion path are the same compiled code. If the runaway run enforces, the shipping run's hook enforces too.

## Specification

### 1. The hook is installed before the timer is armed, and is live on the real boot path

**Given** `os`'s boot path,
**then** `hal_x86_64::interrupts::set_tick_hook` is called **before** `interrupts::init`, so no tick can be delivered between arming the timer and installing the consumer.

**And** the installation is asserted by reading the pointer back — `interrupts::tick_hook_installed()`, reported as `hook_installed` — not by counting ticks.

**This clause was first written the other way, and the draft was wrong.** It originally asserted `ticks_unattributed >= 1`, on the reasoning that stages 1–4 run with interrupts enabled and no task on the CPU, so a live hook must count *something*. Two consecutive runs of identical binaries reported `ticks_attributed=[1,0,0,0] unattributed=1` and then `ticks_attributed=[0,0,0,0] unattributed=2`: the total held but the split moved, and both numbers are a function of how long this boot took relative to a ~16ms timer period. On a fast enough host the total could legitimately be zero, and a build that installed no hook would be indistinguishable from one that did. A gate whose verdict depends on host speed is `LE-16`/`LE-18`'s failure mode, and this test very nearly acquired a second instance of it.

The serviced-tick count is still **reported** (`ticks_serviced`), and is asserted only in clause 7's run, where a workload that runs to the loop bound guarantees it.

### 2. The attribution rule is checked before the scheduler is touched

**Given** the hook,
**then** its first action is to read the dispatcher-owned current-task cell. On `None` it increments the unattributed counter and returns **without forming any reference to the scheduler**. That ordering is both `wcet::attribute_tick`'s `Nobody` arm and the soundness precondition that makes a tick landing in the dispatcher harmless, and it is the same ordering `fixture_wcet`'s hook holds.

**And** the cell is written only by the dispatch loop, only with `IF` clear.

### 3. The dispatcher runs with `IF` clear, observed rather than asserted

**Given** the dispatch loop,
**then** on every round, before it selects, it reads `RFLAGS` and records whether `IF` was clear. The image reports `dispatcher_if_clear` and it must be `true`.

A comment claiming the dispatcher runs masked is not evidence; a read of the actual flag on the actual path is. The check costs one `pushfq` per round and it is the only thing standing between the re-entrancy argument and a rediscovery of it under a debugger.

### 4. The supervisor's address space is live again after every dispatch round

**Given** that `os` dispatches through `run_once_in_space` — which installs the selected task's `CR3` and does **not** restore the caller's on the way back — and that a preempting or enforcing tick returns control *inside* that function,
**then** the dispatch loop reinstalls the supervisor `PML4` immediately after each round returns, before the next selection, and the image reports `cr3_after_round` equal to the supervisor tree's own address.

This is the fixture pattern's one non-transferable part, and `TEST-P1-04-02-A`'s fixtures could not have caught it: they use `run_once`, which touches no `CR3` at all.

### 5. The workload's overrun declaration is a decision, and it is recorded

**Given** the embedded workload,
**then** it declares `OverrunPolicy::Degrade(PRIORITY_MIN)` against a named budget constant, and the image reports both.

**The reasoning, recorded here so a later change has to argue with it.** The previous declaration was `TripToSafeState` with a 1,000-tick budget and no stated rationale. `TripToSafeState` means a contained, capability-mediated application that merely burns CPU can halt the whole system — which is a strictly *more* severe consequence than this same system gives the same task for a genuine CPU fault, where `kernel::fault::Disposition::of` answers `TerminateTask` and reserves `HaltSystem` for the kernel's own context. It also hands that application exactly the denial-of-service that `PD-07`/`PD-08` temporal isolation and `BND-15` exist to deny it. `Degrade` to `PRIORITY_MIN` is the containment-consistent answer: the offender keeps running, but at the bottom of the priority space, where it can preempt nothing and starve nothing.

`TripToSafeState` remains the right declaration for a task whose *failure* is a system-level event. This workload is not one, and the distinction is the point of the policy being per-task.

### 6. Tier 0: the shipping image, unchanged in every claim it already made

**Given** `cargo run -p xtask -- qemu-x86_64 --fixture=os`,
**then** every claim `TEST-P1-03-03-A` clause 6 recorded still holds verbatim — the granted `GetCurrentProcess` returns `0xffffffffffffffff`, the denied `ExitProcess` is contained at `iat::CAPABILITY_TRAP_VIRT`, the task is `Finished`, the W^X audit reports no violation, and the image exits successfully — **and** clauses 1, 3 and 4 above are reported alongside them.

The workload does not overrun: it executes a handful of instructions. `enforcements` must therefore be `0`, and that is the correct result. **A run in which the shipping workload tripped its own budget would be a defect**, so this clause asserts the absence deliberately rather than leaving it unstated.

**Observed** (`isa-debug-exit` success):

```text
tinyos: 1 CPU(s), 6 PCI device(s)
tinyos: W^X memory protection active
tinyos: loaded image — 2 import(s), 2 granted, 0 trapped, cr3=0x147000
tinyos: task 0 terminated — vector=14 rip=0xdead0000 cr2=0xdead0000 (refused capability)
tinyos: scheduler hook_installed=true if_clear=true cr3_after_round=0x1d9000 rounds=1 rounds_exhausted=false preemptions=0 ticks_serviced=2
tinyos: budget ticks_attributed=[0, 0, 0, 0] unattributed=2 unknown=0 books_agree=true budget=8 policy=degrade_to_0
tinyos: enforcements=0 ticks_at_first_enforce=0 (bound 9) first_enforce_tick=0 spacing_ok=true priority_after_enforce=None wrong_task=false wrong_disposition=false
tinyos: workload returned 0xffffffffffffffff (GetCurrentProcess ok=true), exited via trap 0xdead0000, task_finished=true, spoors=5
tinyos: boot complete, ok=true
```

`cr3=0x147000` against `cr3_after_round=0x1d9000` is clause 4 in two numbers: the round ran under the image's `PML4` and the supervisor's own was live again before the loop could select anything else. `enforcements=0` against `budget=8` is clause 6's deliberate absence — this workload used no measurable budget at all, which is what a system image's workload should do.

### 7. Tier 0: a real image that will not give up the CPU is caught and degraded

**Given** `cargo run -p xtask -- qemu-x86_64 --fixture=os-runaway`,
**then** the same `os` binary is built with the same hook, dispatcher and declaration, embedding `runaway-probe.txe` instead of `capability-probe.txe`. That image is a genuine PE32+ with the same two imports, admitted by the same `check_imports` gate, mapped by the same `AddressSpace::create` and patched by the same `iat::patch_imports` — and its `.text` is an unconditional two-byte self-jump. It calls nothing, faults nothing, and yields nothing.

**Then**:

- **detection**: `enforcements >= 1`, and the first one lands on the workload's `budget + 1`-th attributed tick — no earlier (that would mean the budget was not honoured) and no later than `budget + MAX_TICKS_TO_ENFORCE`, a bound fixed as a constant in this document before the code existed: **`MAX_TICKS_TO_ENFORCE = 1`**;
- **the declared consequence actually happened**: the task's live priority is `PRIORITY_MIN` after the first enforcement, having been `8` before it. The image reads that back out of the scheduler. Nothing else in `os` writes a priority, so this cannot be produced by the reporting path;
- **the budget window was reset**: successive enforcements are spaced a full `budget + 1` attributed ticks apart, never one per tick. This is the assertion `TEST-P1-04-02-A`'s falsification proved was the only externally visible consequence of the kernel's own reset, and it is carried over here for the same reason;
- **the system stayed in control**: the run ends through the dispatch loop's own bound and reports, rather than being killed by the harness. `rounds_exhausted=true` is the expected and correct outcome for a workload that never terminates.

**Observed** (`isa-debug-exit` success):

```text
tinyos: 1 CPU(s), 6 PCI device(s)
tinyos: W^X memory protection active
tinyos: loaded image — 2 import(s), 2 granted, 0 trapped, cr3=0x148000
tinyos: scheduler hook_installed=true if_clear=true cr3_after_round=0x1da000 rounds=4 rounds_exhausted=true preemptions=0 ticks_serviced=37
tinyos: budget ticks_attributed=[36, 0, 0, 0] unattributed=1 unknown=0 books_agree=true budget=8 policy=degrade_to_0
tinyos: enforcements=4 ticks_at_first_enforce=9 (bound 9) first_enforce_tick=10 spacing_ok=true priority_after_enforce=Some(0) wrong_task=false wrong_disposition=false
tinyos: workload returned 0x0 (GetCurrentProcess ok=false), exited via trap 0x0, task_finished=false, spoors=15
tinyos: boot complete, ok=true
```

Four numbers carry the whole clause. `ticks_at_first_enforce=9` against `budget=8` is detection on the *first* tick that could possibly have crossed the budget — the bound this document fixed at `budget + 1` before the code existed, met exactly rather than approached. `priority_after_enforce=Some(0)` is the declared consequence read back out of the scheduler, from a task created at priority 8; nothing else in this binary writes a priority. `ticks_attributed=[36,...]` against four enforcements is `4 × (8 + 1)` exactly, which is the budget window having been reset every time and is the one thing no amount of reporting machinery can fake. And `rounds_exhausted=true` with `task_finished=false` says what it should: the workload is still perfectly willing to run forever, and the system stopped it and reported anyway.

`ticks_serviced=37` against `unattributed=1` is also worth reading: exactly one tick landed while no task was on the CPU across the whole run, because the dispatch loop runs masked. That is clause 3 from the other side.

### 8. Enforcement charges nobody else

**Given** either run,
**then** the hook's own per-slot tick count — kept independently of the scheduler's books — agrees with `Scheduler::wcet_state`'s consumed count for every live task, and the image reports `books_agree=true`. A tick charged twice, charged to the wrong task, or charged to whoever ran last breaks the equality in one direction or the other.

`ticks_unknown` must be `0`: a tick attributed to a task the scheduler does not know is a fail-closed path that must never be taken here.

### 9. Host: the runaway image is a real image, and is the intended one

**Given** `xtask`'s host tests,
**then** the runaway generator's output is re-derived rather than trusted: it is four pages, starts with a real DOS header, declares the same three sections with the same permissions and no W+X section, imports the same two names by name with a separate ILT and IAT, and its `.text` begins with exactly `EB FE` (`jmp $`) — asserted as bytes, because "it loops" is the one property the whole clause 7 run depends on and a mis-assembled jump would present as an unexplained fault under QEMU instead.

**And** the two images are asserted to differ only in `.text`, so nothing about the loader path, the import surface or the capability decision can silently change between the shipping run and the runaway one.

### 10. Falsification

A fixture that passes is not evidence until something has been broken and it has been observed failing. The mutations run, and what each must break:

| Mutation | Must fail | Observed |
|---|---|---|
| `set_tick_hook` call removed | clause 1, **and** clause 7 must not enforce | `hook_installed=false ... ticks_serviced=0`, `ok=false`, exit 1 — and the runaway arm **timed out** |
| `interrupts::disable_interrupts` before the loop removed | clause 3 | `if_clear=false`, `ok=false`, exit 1 |
| the supervisor `CR3` reinstatement removed | clause 4 | `cr3_after_round=0x147000` — the *image's* `PML4` — `ok=false`, exit 1 |

**The first mutation's runaway result is the one to read twice.** Removing the hook and changing nothing else does not produce a failed assertion against the runaway workload. It produces:

```text
xtask: kernel did not reach the isa-debug-exit port within the 15s boot time budget
```

That is `LE-20` stated as a capture. The shipping image, holding a capability-admitted workload whose entire content is `EB FE`, had no way to take the CPU back — and no existing evidence in this project would have shown it, because nothing ran the shipping binary against a workload that declined to yield.

**The third mutation is the trap Handover 06 named**, and it could only have been caught here. `cr3_after_round` reporting the image's `PML4` means the supervisor was running, and would have made its next scheduling decision, with the workload's address space live. It is survivable rather than fatal — the image space attaches the shared kernel directories, so the supervisor's own code and stack stay mapped — which is precisely the kind of containment defect that survives a green test run. `TEST-P1-04-02-A`'s fixtures could not have found it: they dispatch through `run_once`, which touches no `CR3` at all.

### 11. What this test explicitly does **not** establish

- **No hardware tier.** Tier 0 QEMU only; `LE-09` open. Every tick count here is a count of ticks, not a latency.
- **No measured enforcement cost.** `D03` still has no baseline. That the shipping image enforces *correctly* is this test's claim; that it enforces *within a bounded real time* is not.
- **No multi-task shipping workload.** The boot path carries one task, so the degrade is observed as a priority actually dropping, not as a competitor winning a selection it previously lost. That second claim is `fixture_wcet_degrade`'s and is unaffected.
- **No equal-priority rotation**, unchanged from `STORY-P1-04-01`.
- **No teardown path.** `ExitProcess` still routes into the capability trap.
- **Nothing about `LE-22`.** Degrade and priority inheritance are still unreconciled, and this image takes no locks.
- **The `os` image's dispatch loop bound is not a scheduling policy** and is not evidence about one.

## Test type

Tier 0 QEMU runs (`qemu-x86_64 --fixture=os`, `--fixture=os-runaway`) plus host unit tests (`#[cfg(test)]` in `os/src/xtask/src/probe_pe.rs`).

## Implementation location

- `os/src/os/src/main.rs` — the tick hook, the `IF`-clear bounded dispatch loop, the supervisor `CR3` reinstatement, the workload's declaration, and the reported evidence.
- `os/src/os/Cargo.toml` — the `fixture-os-runaway` feature.
- `os/src/xtask/src/probe_pe.rs` — `build_runaway`, and the host tests re-deriving it.
- `os/src/xtask/src/main.rs` — `make-probe-pe --runaway`, and the `--fixture=os-runaway` mapping.
- `os/src/exec/fixtures/runaway-probe.txe` — the generated image.
- `.github/workflows/ci.yml` — the new CI step.

## Reports

- [`REPORT-2026-07-28-05`](../reports/REPORT-2026-07-28-05.md)
