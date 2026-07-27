# TEST-P1-03-02-A — W^X, Sealing, Generation-Safe Teardown, and the First Real Task

Status: **Verified (Tier 0 + Host) — written alongside implementation, per the process note below**
Story: [`STORY-P1-03-02`](../stories/STORY-P1-03-02.md)
Tier: Host unit tests (every pure seam) **plus** two Tier 0 QEMU fixtures, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D04`, `D05`, `D08`
Security controls: `SEC-03`, `SEC-14`, `SEC-19`
Containment classes: `C0`, `C1`, `C3`
Boundary tests: `BND-04`, `BND-05`, `BND-20`
Protection Domain contracts: `PD-01`, `PD-04`, `PD-13`
Code admission gates: `RCG-10`, `RCG-11`
Assurance state: `baseline-debt`

## What this test is for

Two things at once, and the second is why the first matters.

**Part A** replaces the last all-RWX memory in this system. Every mapping since `STORY-P0-01-01` has been readable, writable and executable — including `boot.rs`'s 1GiB identity map, which is what the kernel itself has been running on. `STORY-P1-03-01`'s fixture even duplicated an all-RWX kernel replica into each task's tree, explicitly as a stand-in. This test proves the replacement: a real W^X kernel tree built from the linker's own section boundaries, shared (not copied) into every space, with both W^X directions faulting for real and with the loader's own writable aliases sealed shut.

**Part B** is the first time this project runs something. Every mechanism `EPIC-P0`/`EPIC-P1` built — scheduler, context switch, PE64 loader, capability shim, fault containment, per-task `CR3`, spoor — was proven in its own isolated fixture and never together, and never against a real workload. This test schedules `blue-sharc.exe` as a genuine task in its own W^X address space, through the production dispatcher, and contains what it actually does.

## Specification

### 1. The `CR3` dispatch decision is pure and host-tested on both arms

**Given** `kernel::dispatch::switch_plan(address_space)`,
**then** `None` yields `SwitchPlan::Plain` and `Some(cr3)` yields `SwitchPlan::InstallAddressSpace(cr3)`, and `Scheduler::address_space_of` distinguishes a live task without a space, a live task with one, and an unknown task. `run_once` itself is **unchanged** — its existing tests are the no-regression guard for every pre-existing Story's tasks, all of which are `None` — and `run_once_in_space` is the new consumer. The decision is factored out precisely because the install arm's `mov cr3` can never retire in a host process (`switch_address_space` is `cfg`-gated off on Windows hosts), so the arm that *can* be host-tested is, and the hardware arm is proven under Tier 0.

### 2. The enforcement bits are enabled, not assumed

**Given** `hal_x86_64::paging::enable_nx_and_wp`,
**then** `EFER.NXE` and `CR0.WP` are set before any W^X claim is made. This clause exists because without it the rest of Part A is vacuous: at CPL 0 a write to a read-only page **succeeds** unless `CR0.WP` is set, and PTE bit 63 is a *reserved* bit rather than no-execute unless `EFER.NXE` is set. `map_4k` has been setting that bit since `STORY-P0-05-02` with nothing making the hardware honor it.

### 3. Host: the paging, kernel-map, and address-space seams

**Given** the pure primitives this Story adds, **then**:

- `protect_4k` rewrites a present leaf's permissions keeping its frame, round-trips both ways (seal and unseal), and fails closed on unmapped/unaligned input.
- `install_shared_pd` links **one** directory into two trees such that a mapping added through one is visible through the other, `directory_addr` reads the same physical address back from both, and misaligned or already-occupied installs fail closed. Sharing is at page-directory granularity deliberately: the image base (`0x1_4000_0000`) and kernel low memory share PML4 slot 0, so a coarser sharing unit would leak one task's image into every other space.
- `for_each_leaf` visits exactly the present leaves with their real permission bits, handles huge-page entries at their own granularity, and **finds** a deliberately-planted W+X mapping — the audit is falsifiable, not decorative.
- `kernel_map::build_shared_directories` produces exec pages RX, rodata RO-NX and everything else RW-NX from a layout, maps only the image's own rounded extent, yields zero W+X leaves under the audit walk, and fails closed on a malformed layout or an exhausted pool.
- `AddressSpace::seal_kernel_alias` strips the writable identity-view alias of every non-writable image page and leaves writable pages' aliases alone; `unseal_kernel_alias` restores them; sealing without a covering kernel tree fails closed.
- `AddressSpace::teardown` revokes every image mapping, wipes the staged frames, advances the generation exactly once, and **leaves an attached shared directory linked** so the torn tree stays loadable.

### 4. Tier 0 (`wx-seal`): W^X in both directions, shared, sealed, and torn down

**Given** the enforcement bits enabled and the boot-time RWX identity map retired for a W^X kernel tree built from `__kernel_exec_start`/`__kernel_exec_end`/`__kernel_rodata_start`/`__kernel_rodata_end`/`__kernel_image_end`,
**then**:

- Both the supervisor's tree and the task's tree read back the *same physical* low page directory — shared, proven by address rather than asserted.
- A walk of every leaf of both live trees finds zero mappings that are simultaneously writable and executable, **and** zero executable frames carrying a writable alias in the kernel view.
- A task that writes its own RX page raises a real `#PF` and is terminated; a task whose entry point *is* its own RW page raises a real `#PF` on instruction fetch and is terminated. Both directions of `BND-05`, both from scheduled tasks (a supervisor-context fault would be `HaltSystem` by policy and would prove nothing about containment), both dispatched through the production `run_once_in_space`.
- After unsealing, teardown wipes staging (checked non-zero before, all-zero after — residue, not just a claim) and advances the generation to 1, and a third task then probes the stale image address under the still-loadable torn tree and faults for real.

**Observed** (`ok=true`, `isa-debug-exit` success):

```text
wx-seal: NXE+WP enabled
wx-seal: layout exec=0x101000..0x1213ab rodata=0x122000..0x12c458 image_end=0x34b000
wx-seal: boot RWX map retired, supervisor W^X tree live
wx-seal: task space built and sealed, cr3=0x240000
wx-seal: shared low PD 0x139000 (supervisor=Some(1282048) task=Some(1282048))
wx-seal: audit supervisor leaves=1025 exec=33 violations=0; task leaves=1027 exec=34 violations=0
wx-seal contained task 0 vector=14 rip=0x1099af cr2=0x140000000
wx-seal contained task 1 vector=14 rip=0x140001000 cr2=0x140001000
wx-seal: write-to-RX vector=14 execute-RW vector=14 (14 = #PF)
wx-seal: teardown complete, generation=1 staging wiped
wx-seal contained task 2 vector=14 rip=0x10995f cr2=0x140000000
wx-seal: stale probe vector=14 (14 = #PF)
TINYOS-RESULT/1 fixture=wx-seal ok=true
```

The two W^X faults are distinguishable in the capture, which is the point: task 0 faults at a `rip` inside kernel text with `cr2` at the RX page (a *write* violation), while task 1 faults with `rip == cr2 == 0x140001000` (an *instruction fetch* violation on the NX page). One mechanism could not produce both shapes.

### 5. Tier 0 (`first-task`): the first real task, contained and audited

**Given** the real boot path reproduced — the same `discover_topology` and `enumerate_bus_zero` calls against the same success gates `kernel_main` applies, run *before* the CR3 retirement because firmware tables lie outside the kernel tree's extent —
**when** `blue-sharc.exe` is parsed by the real `pe::parse`, mapped by the real `AddressSpace::create` into its own W^X, kernel-sharing space, sealed, and scheduled at its own real entry point through `run_once_in_space`,
**then**:

- The load-time capability gate refuses its real 205-import surface, and the refusal is journaled as a spoor rather than only asserted.
- Every leaf of the live image tree audits clean for W+X and for writable aliases.
- The image, run anyway under an explicit override policy (defense in depth: containment must not *depend* on the gate having held), raises a real `#PF` from its own code, is terminated, and the system continues.
- The spoor journal holds the boot, refusal, dispatch, and containment records — spoor's first production consumption.

**Observed** (`ok=true`):

```text
first-task: real boot path complete — 1 CPU(s), 6 PCI device(s)
first-task: boot RWX map retired, W^X kernel tree live
first-task: load-time gate refused the real 205-import surface: true
first-task: image space built/sealed, cr3=0x91d000 entry=0x14071fe00
first-task: W^X audit of the live image tree, violations=0
first-task: dispatching the real task into its own CR3
first-task contained task 0 vector=14 rip=0x7b9d9e cr2=0x7b9d9e
first-task: captured=true vector=14 rip=0x7b9d9e cr2=0x7b9d9e task_finished=true spoor_journal_len=5
TINYOS-RESULT/1 fixture=first-task ok=true
```

**What that fault actually was**, established against the image's own bytes rather than inferred. `rip == cr2 == 0x7b9d9e` is an instruction-fetch fault, and at file offset `0x7b9d9e` in `blue-sharc.txe` the bytes are `00 00 47 65 74 53 79 73 74 65 6d 54 69 6d 65 41 73 46 69 6c 65 54 69 6d 65 00` — a zero hint word followed by the ASCII string `GetSystemTimeAsFileTime`. That is an `IMAGE_IMPORT_BY_NAME` record. So `blue-sharc.exe`'s real MSVC CRT startup called `GetSystemTimeAsFileTime` — a Win32 API this shim's nine-call allowlist does not contain — through its **unpatched** import address table, whose thunk still holds the RVA of that record; the indirect call took the RVA as an absolute address and fetched from a string. The address was *present* (it falls inside the kernel's own mapped low memory) but **NX**, so the fetch faulted and the task was contained.

Two things follow, and both are stronger than what the Story originally claimed. First, the fault is not staged in any sense: no `ud2`, no deliberately-unmapped probe, no fixture-authored victim — a real program reached for a real capability it did not hold, by the exact mechanism ungoverned code reaches for one. Second, `EFER.NXE` and the W^X kernel map are load-bearing rather than hygienic: without them that fetch would have **succeeded**, and this kernel would have begun executing an API name string as machine code.

### 6. What this test explicitly does **not** establish

- **No hardware tier.** Tier 0 QEMU only; `LE-09` stays open.
- **No unified boot binary.** `first-task-fixture` *reproduces* `kernel_main`'s discovery path rather than living inside it: `exec` depends on `kernel`, so `kernel`'s binary cannot link `exec` back without a cyclic crate dependency (the same constraint that created `exec-fixture`). The shipping `kernel` binary still discovers topology and halts. Unifying them behind a top-level `os` binary crate is named follow-on work.
- **No IAT patching, and therefore no successful Win32 call from inside a task.** The image faults on its *first* reach through the IAT. Making an allowlisted call actually resolve and return from inside a loaded image needs import-table patching this Story does not add — which is why the capability boundary is proven at its two real layers (load-time refusal, runtime containment) rather than as the single "an out-of-allowlist call from inside the task faults" the original criteria assumed.
- **No preemption.** Still cooperative dispatch (`FEAT-P1-04`'s charge); the real task is contained by a fault, not preempted by a timer.
- **No page-table frame reclamation.** Teardown wipes the *staged image frames*; intermediate page-table frames stay pool-allocated until the space is dropped, deliberately, so the torn tree stays loadable for the stale probe.
- **No measured `D04` same-space-vs-cross-space delta.** Deferred with `STORY-P1-03-01`'s own deferral of it.

## Process note: how strictly TDD was followed here

As with `TEST-P1-03-01-A`, this document did not precede the code. The pure seams (`switch_plan`, `protect_4k`, `install_shared_pd`, `for_each_leaf`, `build_shared_directories`, sealing, teardown) were each written with their host tests alongside, and both Tier 0 fixtures were brought up against real QEMU captures. The nine design defects the criteria were rewritten against were found by a pre-implementation review of the Story rather than by tests — recorded in [`session/hand-2026-07-28/03-story-p1-03-02-hardening-review.md`](../../session/hand-2026-07-28/03-story-p1-03-02-hardening-review.md) rather than presented as a clean Red run that did not happen.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-x86_64/src/paging.rs`, `os/src/exec/src/kernel_map.rs`, `os/src/exec/src/address_space.rs`, `os/src/kernel/src/dispatch.rs`) plus two Tier 0 QEMU fixtures (`cargo run -p xtask -- qemu-x86_64 --fixture=wx-seal` and `--fixture=first-task`).

## Implementation location

- `os/targets/x86_64-tinyos.ld` — the kernel section-boundary symbols the W^X map is built from.
- `os/src/hal-x86_64/src/paging.rs` — `protect_4k`, `install_shared_pd`, `directory_addr`, `for_each_leaf`, `enable_nx_and_wp`.
- `os/src/exec/src/kernel_map.rs` — the shared W^X kernel directories.
- `os/src/exec/src/address_space.rs` — `attach_shared_pd`, `seal_kernel_alias`/`unseal_kernel_alias`, `teardown`, `TeardownGeneration`.
- `os/src/kernel/src/dispatch.rs` — `SwitchPlan`, `switch_plan`, `run_once_in_space`.
- `os/src/kernel/src/sched.rs` — `Scheduler::address_space_of`.
- `os/src/kernel/src/capacities.rs` — `SPOOR_JOURNAL_CAPACITY`.
- `os/src/kernel/src/main.rs` — the shipping binary's fault-audit spoor journal.
- `os/src/exec/src/fixture_wx_seal_main.rs`, `os/src/exec/src/fixture_first_task_main.rs` — the two Tier 0 fixtures.
- `os/src/exec/Cargo.toml`, `os/src/xtask/src/main.rs`, `.github/workflows/ci.yml` — binary targets, `--fixture` mappings, CI steps.

## Reports

- [`REPORT-2026-07-28-01`](../reports/REPORT-2026-07-28-01.md) — the two Tier 0 captures, the `GetSystemTimeAsFileTime` finding, and what remains open.
