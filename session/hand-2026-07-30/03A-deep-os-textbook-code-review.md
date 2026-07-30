# Handover 03A — Deep OS Textbook-to-Code Review

**Review date:** 2026-07-30
**Reviewed baseline:** `8cf3e46c42e3dac5f649adbfc298976bb3b1f84e` on `main`.
**Enhancement state:** this document also reviews the post-baseline hardening
tranche in the working tree. It is intentionally not described as committed;
the owner retains commit/merge control.
**Review posture:** adversarial assurance review, not a style review. Source and
executed evidence take precedence over status prose.
**Primary textbook spine:** Remzi and Andrea Arpaci-Dusseau, *Operating Systems:
Three Easy Pieces* (OSTEP). Its three organizing ideas — virtualization,
concurrency, and persistence — are used below. Silberschatz, Galvin, and Gagne,
*Operating System Concepts*, supplies the protection/security vocabulary that
OSTEP deliberately does not emphasize. Jane W. S. Liu, *Real-Time Systems*,
supplies the real-time scheduling, admission, and resource-sharing standard
needed for TinyOS's hard-RT claims.

## 1. Executive verdict

TinyOS is already an unusually good **mechanism-learning kernel**. It has compact,
readable implementations of bounded allocation, task control blocks, context
switching, active `CR3` changes, fixed-priority dispatch, timer preemption,
priority inheritance bookkeeping, budget-overrun dispositions, page-table
construction, W^X mapping, PE64 parsing, IAT patching, bounded IPC, generational
shared-memory grants, exception reporting, a deterministic DOS-flavoured shell,
and an executable assurance graph. Most mechanisms have both host tests and
QEMU fixtures. The repository is exceptionally disciplined about recording what
has not yet been proved.

It is **not yet a secure process-isolating operating system**, a hard-real-time
system, or a release-qualified platform. The central reason is architectural,
not cosmetic:

> Every scheduled workload still executes at x86 CPL 0, while every task page
> table deliberately maps writable kernel memory and the writable local-APIC
> page. Active `CR3` switching and W^X therefore separate layouts, but do not
> form a protection boundary against admitted code.

That one fact changes how almost every textbook term must be used:

- `Tcb`, `Context`, and `TaskState` currently describe **kernel threads**, not
  protected user processes.
- page tables provide **address-space mechanism**, but not user/kernel
  isolation;
- a task fault can be *classified* as task-context and the TCB can be retired,
  but a hostile task has already had kernel authority before the fault;
- PE admission controls which bytes the kernel chooses to start, but started
  bytes can execute privileged instructions and modify the kernel;
- capabilities, typed IPC, quotas, and containment classes are strong designs
  and useful models, but are not yet the sole paths to authority.

The correct release decision today is:

| Claim | Review verdict |
|---|---|
| Tier-0 learning kernel and OS-mechanism demonstrator | **Supported** |
| Deterministic, heap-free mechanism implementations under bounded tests | **Substantially supported** |
| One embedded PE probe executes under its own active page-table root | **Supported** |
| W^X permissions are constructed and audited in the tested trees | **Supported as a mapping property** |
| Protected user processes / hostile-code containment | **Not supported** |
| Complete capability OS | **Not supported** |
| Hard real-time temporal isolation | **Not supported** |
| Hardware-qualified x86_64 or ARM64 platform | **Not supported** |
| Safe ingestion or execution of external/untrusted executables | **Release-blocked** |
| Zero-zero-day or takeover-resistant system | **An objective, not a present property** |

The assurance checker agrees with the cautious interpretation: it passes
structurally, but reports **31 open loose ends, 0 qualified platforms, and 0
bound claims**. That is healthy honesty. The mistake would be to treat a green
traceability graph as proof of a security or timing property that the hardware
execution path cannot yet enforce.

### 1.1 Outcome of the review: defects converted into code

This review did not stop at diagnosis. The post-review hardening tranche changes
the kernel, loader, HAL, shell policy, tests, and hardware evidence:

| Finding | Outcome in this tranche | Evidence |
|---|---|---|
| DR-01, missing hardware privilege foundation | **Foundation implemented; boundary still open.** The GDT now contains CPL-3 code/data descriptors, the TSS has a real budgeted `RSP0` stack, page-table construction carries effective U/S through every level, task image/shared pages are user-accessible, and kernel/APIC pages are asserted supervisor-only. The scheduler still enters workloads through a CPL-0 `ret`; `iretq`, a complete user trap frame, and the syscall/reference-monitor path remain release blockers. | [`gdt.rs`](../../os/src/hal-x86_64/src/gdt.rs), [`tss.rs`](../../os/src/hal-x86_64/src/tss.rs), [`paging.rs`](../../os/src/hal-x86_64/src/paging.rs), [`address_space.rs`](../../os/src/exec/src/address_space.rs), [`kernel_map.rs`](../../os/src/exec/src/kernel_map.rs), [`capacities.rs`](../../os/src/kernel/src/capacities.rs) |
| DR-02, arbitrary PE entry | **Closed in code.** Parsing now proves that the entry lies in an executable, non-writable section; stores the checked virtual entry; rejects virtual-range overflow; address-space construction rejects non-canonical ranges; and the shipping path returns immediately if any import, entry, patch, or W^X proof fails. | [`pe.rs`](../../os/src/exec/src/pe.rs), [`address_space.rs`](../../os/src/exec/src/address_space.rs), [`os/main.rs`](../../os/src/os/src/main.rs) |
| DR-03, ABA handles | **Closed for Pool/Task identity.** Slot and handle carry a checked non-wrapping generation; exhausted slots retire; stale handles and stale `TaskId`s cannot affect a replacement. Teardown generation now fails explicitly instead of saturating. | [`mem.rs`](../../os/src/kernel/src/mem.rs), [`sched.rs`](../../os/src/kernel/src/sched.rs), [`address_space.rs`](../../os/src/exec/src/address_space.rs) |
| DR-04, omitted ordinal imports / loose PE bounds | **Closed by explicit modelling.** `ImportSymbol` represents names and ordinals. Every thunk becomes one dependency and one patched IAT cell; unsupported ordinals fail the allowlist and receive the capability trap. Import descriptors are bounded by the declared directory, thunk/name reads stay inside file-backed section bytes, virtual zero-fill is never parser input, and reserved thunk bits are rejected instead of truncated. The real artifact is now measured correctly as **205 named + 15 ordinal = 220 imports**. | [`pe.rs`](../../os/src/exec/src/pe.rs), [`iat.rs`](../../os/src/exec/src/iat.rs), [`win32_shim.rs`](../../os/src/exec/src/win32_shim.rs), [`fixture_blue_sharc_main.rs`](../../os/src/exec/src/fixture_blue_sharc_main.rs) |
| DR-05, intermittent PI evidence | **Closed with repeated hardware evidence.** `LOW_RELEASED` is published inside the interrupt-masked state transition before low becomes `Finished`. The repaired fixture passed **100/100** QEMU boots; the review baseline reproduced only **42/50**. This does not close the separate multi-lock donation limitation. | [`fixture_priority_inversion.rs`](../../os/src/kernel/src/fixture_priority_inversion.rs) |
| DR-07, shell priority/authority reversal | **Closed in the model.** Higher numeric priority now matches the kernel convention, while `KillAuthority::{Ordinary, SupervisorOnly, Unkillable}` is a separate field. Tests prove priority zero carries no implicit protection. | [`verbs.rs`](../../os/src/shell/src/verbs.rs), [`parity.rs`](../../os/src/shell/src/parity.rs), [golden transcript](../../os/src/shell/golden/parity-smoke.golden.txt) |
| DR-09, non-transactional revoke / stale TLB | **Closed for the present single-core model.** Revoke authenticates and preflights the complete range before mutation, retains registry authority until teardown succeeds, and has a repair/retry regression. Leaf removals and sealing invalidate the local translation. The documented future SMP obligation is a remote shootdown protocol. | [`shared_memory.rs`](../../os/src/exec/src/shared_memory.rs), [`address_space.rs`](../../os/src/exec/src/address_space.rs), [`paging.rs`](../../os/src/hal-x86_64/src/paging.rs) |
| DR-10, Windows parity divergence | **Closed.** Golden text is EOL-pinned/normalized and host plus QEMU parity agree. The current gate reports 64 matching lines and corroborates one denial with one kernel-format spoor. | [`.gitattributes`](../../.gitattributes), [`parity.rs`](../../os/src/shell/src/parity.rs), [`shell_parity.rs`](../../os/src/xtask/src/shell_parity.rs) |

DR-06 (kernel-derived current identity), DR-08 (period/deadline/replenishment
and admission), DR-11/12 (unsafe-boundary governance and shell handler
decomposition), DR-13 (service integration), and DR-14 (committed fuzz/model
checking campaigns) remain open. They are not hidden by the changes above.

The key architectural distinction is:

```text
implemented now                         still required for hostile code
──────────────────────────────────      ──────────────────────────────────
CPL-3 selectors in GDT                  initial iretq into CPL 3
TSS.RSP0 kernel-entry stack             complete user-origin trap frame
effective U/S page permissions          syscall/trap ABI
kernel/APIC supervisor-only proof       current actor derived from active TCB
checked PE dependency + entry proof     capability space enforced on every entry
generational object identity            full task/object/capability teardown
local TLB invalidation                  SMP shootdown before more CPUs
```

This is deliberately a staged-state design. Possessing the ingredients of a
protection boundary is not the same state as executing behind that boundary.

## 2. Evidence ladder: what is proved at each level

TinyOS is easiest to understand if its evidence is read as a ladder, not a
single green/red state.

| Level | What exists | What it proves | What it does not prove |
|---|---|---|---|
| Specification | `SeedMVP.md`, `SECURITY_CHARTER.md`, `goals/` catalogues | Intended invariants and a common vocabulary | That code enforces any invariant |
| Structural assurance | `xtask::assurance`, 27 Features, 68 Stories, 625 performance cells | IDs, joins, statuses, catalogues, dashboard claims, evidence references are internally consistent | Semantic correctness of the referenced code or evidence |
| Host model | 86 `exec`, 143 `kernel`, 115 ARM HAL, 101 x86 HAL, 210 `xtask`, 33 shell, and 13 shared-HAL tests (**701 passed, 1 deliberately ignored**) | Pure decisions, parsers, bounded structures, permission construction, failure branches | Real interrupt timing, CPU privilege, TLB behaviour, hardware integration |
| QEMU fixture | 30 catalogued x86_64 fixtures | Mechanisms execute on the target ISA under an emulator; negative fixtures can fail intentionally | Hardware qualification, SMP, DMA/IOMMU, adversarial external ingress |
| Shipping composition | [`os/src/os/src/main.rs`](../../os/src/os/src/main.rs) | Boot, W^X kernel map, embedded PE parse/admission/patch/seal, active task `CR3`, timer-driven dispatch compose in one image | General process launch, user mode, task teardown, complete capabilities, multiple hostile processes |
| Host console | `external/tauri/tinyos-poc` | Signed host manifest, runtime-derived webview identity, deny-by-default commands, isolated host shell worlds, multi-tab UX | On-target shell, kernel capability enforcement, kernel spoors, secure attention on TinyOS |
| Hardware/release | Registers and reports reserve the category | The project knows what qualification must mean | Nothing yet: 0 platforms are qualified |

This ladder is one of the project's best teaching devices. Preserve it. Do not
collapse “a host test exists,” “a QEMU fixture passed,” and “a platform is
qualified” into the word *verified*.

## 3. End-to-end architecture, mapped to the code

### 3.1 The current x86_64 shipping path

The executable system in [`os/src/os/src/main.rs`](../../os/src/os/src/main.rs)
is a compact vertical slice:

```text
boot entry
  → serial / topology / interrupt setup
  → build shared kernel page directories
      kernel text RX
      kernel rodata RO-NX
      kernel data/stacks/statics RW-NX
      local APIC MMIO RW-NX
  → load supervisor CR3 and enable CR0.WP + EFER.NXE
  → parse embedded PE64 probe
  → model every named/ordinal import and check the complete surface
  → create task page tables with U/S on task-owned pages and copy sections
    through supervisor staging aliases
  → attach the shared kernel and APIC directories
  → patch the IAT
  → seal writable aliases and audit W^X
  → create TCB + Context + task CR3
  → fixed-priority dispatch
  → APIC timer tick may preempt back to the dispatcher
  → budget policy restarts, degrades, or trips the workload
```

The corresponding code seams are:

- boot composition: [`os/src/os/src/main.rs`](../../os/src/os/src/main.rs);
- x86 entry, descriptor tables, interrupts, APIC, TSS/IST:
  [`hal-x86_64`](../../os/src/hal-x86_64/src);
- task model: [`kernel/src/sched.rs`](../../os/src/kernel/src/sched.rs);
- saved stack/register context:
  [`kernel/src/context.rs`](../../os/src/kernel/src/context.rs);
- selection and switch:
  [`kernel/src/dispatch.rs`](../../os/src/kernel/src/dispatch.rs);
- timer decision:
  [`kernel/src/preempt.rs`](../../os/src/kernel/src/preempt.rs);
- budget accounting:
  [`kernel/src/wcet.rs`](../../os/src/kernel/src/wcet.rs);
- page-table primitives:
  [`hal-x86_64/src/paging.rs`](../../os/src/hal-x86_64/src/paging.rs);
- process mapping and sealing:
  [`exec/src/address_space.rs`](../../os/src/exec/src/address_space.rs);
- shared kernel mappings:
  [`exec/src/kernel_map.rs`](../../os/src/exec/src/kernel_map.rs);
- PE description:
  [`exec/src/pe.rs`](../../os/src/exec/src/pe.rs);
- import policy and trampolines:
  [`exec/src/win32_shim.rs`](../../os/src/exec/src/win32_shim.rs) and
  [`exec/src/iat.rs`](../../os/src/exec/src/iat.rs).

This composition is small enough for a learner to trace in one sitting, which
is a major strength. It also exposes the present boundary cleanly: the same
`Context::new` path that enters ordinary kernel fixture functions enters the PE
workload. There is no `iretq`/`sysret` transition to ring 3.

### 3.2 The scheduler path

```text
Scheduler::create_task
  → Pool<Tcb, N> allocates first free slot
  → TaskId wraps the pool index
  → TCB starts Ready with base priority, budget, policy, entry, optional CR3

dispatcher
  → highest_priority_ready scans live TCBs
  → selected task becomes Running
  → switch_in_space loads CR3 if different
  → context switch swaps saved RSP and stack-resident registers/RFLAGS

timer interrupt
  → find strictly higher-priority Ready task
  → if found, save running context and return to dispatcher
  → equal priority never preempts
```

This is a textbook fixed-priority scheduler core. It is intentionally not yet a
complete production scheduler: there are no per-priority ready queues,
round-robin among equals, periods, deadlines, replenishment timers, CPU
affinity, admission analysis, blocking wait queues, or multiprocessor rules.

### 3.3 The shell path

```text
DOS text
  → dos parser / percent expansion
  → typed Request enum
  → Request::kind
  → deny-by-default VerbPolicy
  → canonical execute interpreter
  → RamVolume / Env / task-view mutation
  → inert deterministic rendering through fmt::Write
```

The `.TCB` batch runner feeds every line through the same parser, policy, and
executor. The host tab system embeds one independent `World` per tab. This is a
good “one semantic core, multiple front ends” architecture.

It is not yet the on-target interactive shell. The target fixture is batch-only,
the x86 serial driver is transmit-only, and the host tabs execute the shell
crate in the Windows host process. `LE-55` and `LE-56` correctly prevent this
from being presented as a kernel console or kernel-spoor path.

## 4. Findings, ordered by risk

Severity means impact **if the affected surface is claimed or enabled**. Some
critical findings are not externally reachable in today's embedded-probe
composition. They are still release blockers because the roadmap explicitly
intends to admit applications and hostile data.

### DR-01 — Critical — scheduled workloads have full kernel privilege

**Post-review status: open, with hardware foundation landed.** U/S permissions,
CPL-3 descriptors, and TSS.RSP0 now exist and are tested, but no shipping task
crosses to CPL 3. The consequence below therefore remains the primary release
blocker.

**Evidence**

- [`kernel/src/context.rs`](../../os/src/kernel/src/context.rs) creates a stack
  frame containing `RFLAGS`, callee-saved registers, and a normal return
  address. The assembly restores it with `popfq`/`ret`; it does not construct a
  ring-transition frame.
- [`hal-x86_64/src/gdt.rs`](../../os/src/hal-x86_64/src/gdt.rs) contains DPL-0
  code/data descriptors and a DPL-0 TSS descriptor. There are no user
  descriptors.
- [`hal-x86_64/src/paging.rs`](../../os/src/hal-x86_64/src/paging.rs) models
  `PRESENT`, `WRITABLE`, and `NO_EXECUTE`; it never sets or reports the x86
  user/supervisor bit.
- [`kernel/src/fault.rs`](../../os/src/kernel/src/fault.rs) explicitly says the
  faulting context is kernel bookkeeping, not a `CS`-derived hardware privilege
  level, and that everything runs at CPL 0. Its additional phrase “one
  identity-mapped address space” is now stale because per-task `CR3` switching
  has since landed, but the CPL-0 statement remains true.
- [`exec/src/kernel_map.rs`](../../os/src/exec/src/kernel_map.rs) shares kernel
  text, rodata, writable data, stacks/statics, and writable APIC MMIO into every
  task page-table tree.
- [`os/src/os/src/main.rs`](../../os/src/os/src/main.rs) attaches those shared
  directories and enters the PE entry point through the ordinary kernel
  `Context`.

**Consequence**

An admitted workload can write kernel globals, scheduler state, TCBs, page
tables, stacks, the spoor journal, and the local APIC. It can execute `cli`,
`mov cr3`, `wrmsr`, or any other privileged instruction. It can bypass IPC,
capabilities, budget accounting, W^X APIs, fault attribution, and every
software-only authority check. Supervisor-only PTEs do not protect supervisor
memory from CPL 0.

This breaks the enforcement premise behind at least `PD-01`, `PD-02`, `PD-03`,
`PD-05`, `PD-07`, `PD-10`, `PD-12`, `PD-13`, and `RCG-13`. It does not make the
current embedded probe malicious, but it means an “untrusted admitted
application” claim would be false.

**Required repair**

1. Add ring-3 code/data descriptors and per-task user stacks.
2. Set U/S only on task-owned user pages; keep all kernel and APIC pages
   supervisor-only.
3. Use the TSS `RSP0` (and the existing IST discipline where appropriate) for
   controlled entry.
4. Build an initial `iretq` frame and preserve the complete user register
   context on interrupts/exceptions.
5. Introduce a narrow syscall/trap ABI; user code must have no direct kernel
   function or raw device address.
6. Derive current identity from the active TCB/domain on every entry.
7. Add negative QEMU fixtures proving a task cannot write kernel data, another
   task, page tables, or APIC MMIO and cannot execute `cli` or load `CR3`.
8. Until those tests pass, use **kernel task** or **kernel thread**, not
   *protected process*, in implementation claims.

This is the highest-priority architectural milestone. More capability policy
above a missing privilege boundary cannot compensate for it.

### DR-02 — Critical — the loader can dispatch an entry point outside the admitted image

**Post-review status: corrected and regression-tested.** The evidence below
describes the baseline defect. The parser now creates a checked entry proof and
the shipping composition treats every failed proof as an immediate control-flow
stop.

**Evidence**

- [`exec/src/pe.rs`](../../os/src/exec/src/pe.rs) reads
  `entry_point_rva` but does not require it to lie inside an admitted executable
  section.
- [`os/src/os/src/main.rs`](../../os/src/os/src/main.rs) calculates
  `image_base + entry_point_rva` with unchecked addition.
- The shipping path checks that `space.translate(entry_virt)` is executable and
  not writable by folding the result into `ok`, but it does not return on
  failure. It later transmutes the address to `TaskEntry` and schedules it.
- The task address space includes the shared, executable kernel mapping.

**Consequence**

A crafted PE can describe acceptable sections while selecting an entry outside
them. Because the low kernel directory is attached before the check, an entry
that resolves to shared RX kernel text can satisfy the check even though it is
not part of the admitted object. One concrete shape is `image_base = 0`, all
declared sections at or above the allowed `0x4000_0000` boundary, and
`entry_point_rva` equal to an executable address in the attached low kernel
directory. An unmapped or NX entry is still activated and only fails after
scheduling. In release builds, the unchecked addition can wrap to a low
address.

This violates the admission rule “reject before executable mapping or first
instruction,” and it combines catastrophically with DR-01.

**Required repair**

- use `checked_add` for every base/RVA calculation;
- require the entry byte to be wholly inside exactly one admitted,
  non-empty, executable, non-writable section;
- explicitly reject entry points in the kernel-reserved range, shared
  directories, IAT/data, gaps, zero-fill tails when policy forbids them, and
  non-canonical addresses;
- make the RX read-back check a control-flow gate (`return Err`), not a
  diagnostic boolean;
- add hostile PE tests for entry-before-section, entry-at-end,
  entry-in-writable-section, entry-in-kernel-map, overflow, and unmapped entry;
- encode the loader sequence in staged types:
  `Parsed → Canonical → Mapped → Patched → Sealed → Runnable`. A raw
  `LoadDescriptor` should not be sufficient to construct a runnable task.

### DR-03 — High — `PoolHandle` and therefore `TaskId` have an ABA identity defect

**Post-review status: corrected and regression-tested.** Pool/Task identity is
now `(index, generation)`, generation wrap retires the slot, and stale IDs
cannot observe or mutate the replacement.

**Evidence**

[`kernel/src/mem.rs`](../../os/src/kernel/src/mem.rs) stores only `index` in
`PoolHandle`; a slot stores only `occupied`. The following sequence is valid:

```text
h1 = alloc(A)       // slot 0
free(h1)
h2 = alloc(B)       // slot 0 again; h1 == h2
free(h1)            // frees B through a stale handle
```

The tests prove immediate double-free rejection and slot reuse separately, but
do not combine them into the stale-handle-after-reuse case. `TaskId` is a
newtype over this handle. Scheduler task destruction is test-only today, so the
shipping composition does not yet expose task-slot reuse; the defect becomes
live as soon as production teardown is added.

**Consequence**

A stale task, object, or endpoint identifier can alias a new occupant. That is
the classic ABA problem and directly contradicts `PD-13` “advance generation
before reuse.” The handle being unconstructable by safe callers prevents
forgery; it does not prevent reuse of a formerly valid token.

**Required repair**

Store a generation in both slot and handle, compare `(index, generation)` on
every access/free, and fail closed on generation exhaustion. The shared-memory
grant registry already demonstrates the correct local pattern with checked,
non-reused generations. Add the exact four-operation regression test above and
apply the same identity type to TCB slots, contexts, queues, grants, and audit
targets before adding general task teardown.

`TeardownGeneration::advance` in
[`exec/src/address_space.rs`](../../os/src/exec/src/address_space.rs) currently
saturates at `u64::MAX`. Saturation prevents wrap, but repeated teardowns then
share one generation. Generation exhaustion should make reuse unavailable,
just as the shared-memory registry already does.

### DR-04 — High — ordinal PE imports are silently admitted but never patched

**Post-review status: corrected by complete dependency modelling.** The parser
now emits `ImportSymbol::Ordinal`, the allowlist rejects unsupported ordinals,
and the IAT patcher overwrites their cells with the trap. The real artifact
contains 220 dependencies, not merely the previously visible 205 named ones.

**Evidence**

[`exec/src/pe.rs`](../../os/src/exec/src/pe.rs) counts ordinal thunks when it
computes later IAT slot addresses, but deliberately omits an `ImportEntry` when
the ordinal flag is set. [`exec/src/iat.rs`](../../os/src/exec/src/iat.rs)
patches only the returned named imports. The ordinal IAT cell therefore retains
its file value. The import allowlist sees no object to reject.

There is a second activation gap in the shipping composition: rejection by
`win32_shim::check_imports` is folded into `ok` and execution continues through
mapping, IAT patching, task creation, and dispatch. A denied named import gets a
trap trampoline, but the image is still activated. That may be useful as a
diagnostic fixture policy; it is not the documented load-time rejection policy.

The parser also walks import descriptors and thunks until a zero terminator
within the file rather than bounding the walk to the declared import directory
and containing section. Memory safety is preserved by file bounds, but
canonical interpretation is not as tight as the charter requires.

**Consequence**

An unsupported import is accepted at load time and fails later at execution
time, usually through a non-canonical thunk value. This violates the stated
“unsupported import is a load-time rejection, never best effort” guarantee.
More importantly, omitted dependency facts undermine the “complete dependency
closure” concept in `RCG-04`/`RCG-07`.

**Repair applied and remaining work**

This tranche chose complete modelling over erasure or parser-level rejection:
every ordinal is an `ImportSymbol::Ordinal`, reaches the allowlist, and receives
one explicit trap IAT patch. Named or ordinal policy denial now terminates the
shipping load path before activation. Descriptor, thunk, and name walks are
bounded by both declared directories and file-backed section windows, with
missing terminators rejected. Structure-aware PE mutation/fuzzing and a staged
loader typestate remain required assurance work.

### DR-05 — High evidence defect — the recurring priority-inversion failure is an interrupt-window bug in the fixture

**Post-review status: corrected; 100/100 repeated QEMU boots passed.** The
multi-lock donation limitation described at the end of this finding remains.

The long soak recorded two intermittent failures. A fresh review sample made
the issue easy to reproduce:

```text
50 priority-inversion runs
42 exit 0
 8 exit 1
 0 harness errors
```

A retained failing serial trace was:

```text
fixture-inversion: acquired=true contended=true boost=Some(25)
released=false priority_after_release=Some(5) high_completed=true
fixture-inversion: dispatch order=[0, 2, 0, 2, 1]
preemptions=1 low_preempted=true
fixture-inversion: medium ready_in_window=true
counter_at_block=0 counter_at_resume=0 counter_final=1000
TINYOS-RESULT/1 fixture=priority-inversion ok=false
```

This combination diagnoses the failure more precisely than `LE-45`:

1. `low_task` unlocks, records the restored priority, makes high Ready, and
   marks low `Finished` **inside** `without_interrupts`.
2. `without_interrupts` executes `sti` before it returns its `true` result.
3. `LOW_RELEASED = released` is written **after** that return.
4. A pending timer can fire between `sti` and the assignment.
5. Low is already `Finished`, so it is never selected again to complete the
   assignment. High and medium complete normally.

The trace therefore shows a false-negative evidence flag, not a failed unlock
or priority-inheritance decision. This is also an excellent teaching example:
an interrupt-safe state transition cannot publish part of its evidence after
interrupts are restored, especially after making its own continuation
unschedulable.

**Required repair**

Write `LOW_RELEASED` inside the same interrupt-masked transaction before
setting low `Finished`, or return a result to a context that is guaranteed to
remain runnable. Then run a high-volume repetition gate that preserves the
first mismatch. Do not close `LE-45` merely because the race is understood;
close it only after the repaired fixture is stable.

This repair does **not** close `LE-49`. The implementation still stores one
`Option<Priority>` per task, not inheritance contributions per held lock. If a
task holds two contended locks, releasing either clears the entire inherited
priority even if a higher-priority waiter remains on the other. A production
PIP implementation needs per-lock waiter state and an effective priority equal
to the maximum contribution across all held resources.

### DR-06 — High — identity and authority are still supplied by callers in core model APIs

`PD-02` requires the kernel to derive actor identity from the running TCB.
Several current interfaces instead accept decision-relevant identity:

- [`kernel/src/ipc.rs`](../../os/src/kernel/src/ipc.rs) accepts a `TaskId` on
  send and receive;
- [`kernel/src/lock.rs`](../../os/src/kernel/src/lock.rs) accepts both the
  contender `TaskId` and a caller-provided `task_priority`;
- shell policy uses a supplied session string;
- host console code is stronger: tab commands derive the tab label from the
  invoking webview rather than a request argument, but that is a host-runtime
  property, not the kernel path.

The private `TaskId` constructor limits accidental fabrication in safe Rust,
but the kernel API can still be called with another live task's ID, and DR-01
removes even that language-level protection from hostile scheduled code.
Supplying priority separately from identity also permits the two facts to
disagree.

**Required repair**

Kernel entry points should read `CurrentDomain { task, generation, class }`
from scheduler/CPU state and derive live priority from that TCB. Delegated
operations should accept an unforgeable endpoint/capability token, never an
actor ID. Retain explicit-actor variants only as private pure functions for
host tests.

### DR-07 — High integration defect — shell kill authority reverses the kernel priority convention

**Post-review status: corrected.** `KillAuthority` is now explicit and
orthogonal to the kernel-consistent numeric priority.

The kernel defines **higher number = more urgent**, from 0 through 31.
[`shell/src/verbs.rs`](../../os/src/shell/src/verbs.rs) documents
`TaskInfo.priority` as **0 = RT-critical** and requires supervisor scope only
when `priority == 0`. Its tests reinforce that second convention.

If the future on-target task table is populated directly from scheduler values,
an ordinary shell kill grant could signal priority 31 — the kernel's most
urgent task — while priority 0, the least urgent task, receives special
protection.

The deeper design problem is using scheduling priority as authorization.
`SECURITY_CHARTER.md` explicitly says containment class, capability authority,
scheduling criticality, and provenance are independent.

**Required repair**

Do not translate numeric conventions. Replace the inference with an explicit
field such as `kill_authority: Ordinary | SupervisorOnly | Unkillable`, or ask
the kernel to authorize a task-control capability. Use `Priority` only for
display/scheduling. Add a cross-crate conformance test before connecting the
shell to live TCBs.

### DR-08 — High claim gap — tick budgets are not yet temporal isolation or WCET evidence

The current real-time code is valuable:

- each TCB carries a declared budget and mandatory overrun disposition;
- timer ticks are attributed to the running task;
- trip, restart, and degrade are total enum arms;
- priority inheritance and degrade use separate fields;
- actuation checks authority and deadline before writing the output line.

But a textbook hard-RT task model needs, at minimum, execution budget `C`,
period or minimum inter-arrival time `T`, deadline `D`, blocking bound `B`,
release/replenishment rules, and an admission/response-time argument. TinyOS's
TCB holds only tick budget and accumulated ticks. `reset_budget_window` is an
explicit caller action. A degraded task remains Ready and, if no higher task is
runnable, may continue executing. Equal-priority non-yielding tasks do not
preempt one another.

Consequently:

- `WcetBudgetTicks` is an execution **budget**, not a measured or proved WCET;
- “depleted domain is not runnable until replenishment” (`PD-07`) is not
  implemented;
- there is no schedulability/admission test;
- no priority-ceiling/resource-sharing bound is computed;
- no platform-qualified tick-to-time conversion supports a deadline claim;
- QEMU timing cannot establish a hardware WCET bound.

Use the existing code to teach budget enforcement and failure policy, but do
not call it temporal isolation or hard-RT admission yet.

### DR-09 — High — shared-memory revocation is not transactional, and page-table mutation has no translation-invalidation protocol

**Post-review status: corrected for the current single-core composition.**
Validation precedes mutation, the registry record is retired last, local
invalidation is explicit, and a damaged-range repair/retry test preserves
authority. Remote shootdown remains mandatory before SMP.

[`exec/src/shared_memory.rs`](../../os/src/exec/src/shared_memory.rs) removes a
grant from `GrantRegistry` **before** unmapping its pages. If any later
`unmap_page` fails — for example, page two of a multi-page grant was already
missing — earlier pages have been removed, later pages can remain mapped, and
the registry token needed to retry has already been destroyed. The function
returns an error in exactly the state its documentation says cannot exist: a
partial stale mapping with no live grant record.

With the current exclusive `&mut AddressSpace`, the repair can be deterministic:
preflight every sharee mapping against the registered owner frame and rights
without mutation; only after the whole range agrees should it unmap; retain a
`Revoking` record until every leaf removal and invalidation completes. An
unexpected failure must quarantine the address space rather than discard the
only cleanup authority. Add a regression test that removes a middle page before
revoke and proves that the registry and all remaining mappings are left in a
recoverable, explicitly failed state.

[`hal-x86_64/src/paging.rs`](../../os/src/hal-x86_64/src/paging.rs) mutates
ordinary page-table memory. `unmap_4k` and `protect_4k` do not execute `invlpg`
or coordinate a TLB shootdown. A `CR3` write flushes non-global entries, and the
current single-core composition usually changes to the supervisor tree before
mutating another task's tree, so a later switch reloads that task's `CR3`.
That makes this primarily a future-composition defect, not a demonstrated
current exploit.

However, the public semantic claims “revoked afterward” and “deterministic
teardown” will become false if:

- an active address space mutates one of its own translations;
- PCIDs/global pages are introduced;
- another CPU can run the affected address space;
- task teardown and shared-grant revocation become concurrent.

The same layer leaves empty intermediate tables allocated after unmap.
Shared-memory rollback removes leaf mappings but can consume intermediate frame
pool capacity across repeated failed grants to different virtual regions.

**Required repair**

Make mapping mutation return an invalidation obligation, or require an
`AddressSpaceActivation`/CPU context that can perform local `invlpg` and remote
shootdown. Specify single-core rules now and SMP rules before SMP. Track and
reclaim empty intermediate tables, or charge their irreversible consumption to
the caller's quota and test exhaustion/rollback as resource state, not only as
leaf-map state.

### DR-10 — Medium — Windows parity surfaces disagree on line-ending semantics

**Post-review status: corrected.** Golden EOL policy and normalized comparison
now agree across the host and QEMU parity lanes.

On this Windows checkout:

```text
cargo test -p shell --lib
  21 passed, 1 failed, 1 ignored
  p1_transcript_matches_golden failed at line 61

cargo run -p xtask -- check-shell-parity
  passed: 61 lines and in-guest signal
```

The committed golden is checked out as CRLF because the repository has no root
`.gitattributes`. The host transcript is built with LF. The shell unit test
uses raw `include_str!` equality; the QEMU parity checker normalizes both
inputs. Thus two tests described as the same golden contract apply different
semantics. The workspace test suite is not green on the documented Windows
development host.

**Required repair**

Prefer a root `.gitattributes` pinning text fixtures to LF, and also place
normalization in one shared comparison helper used by the unit test, target
gate, and host console parser. Add a CRLF regression test analogous to the
dashboard checker's existing line-ending test.

This is distinct from the known Windows all-targets Clippy problem: the latter
also reproduced, because fixture binaries import x86 modules that are compiled
out on `target_os = "windows"`.

### DR-11 — Medium governance defect — the documented unsafe boundary does not match the code

[`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md) says `unsafe` is
permitted only in HAL, drivers, and `-sys` binding crates. Production,
non-fixture code contains unsafe operations in:

- `kernel::context`, `kernel::dispatch`, `kernel::preempt`, and `kernel::mem`;
- `exec::address_space` and `exec::iat`.

Those uses are often necessary and generally have good local safety comments.
The problem is that the enforced architecture and the written policy disagree.
A policy that necessarily fails is not a safety boundary; it trains reviewers
to ignore it.

Choose one:

1. move raw context/paging/patch operations behind HAL or narrowly named
   `-sys` crates; or
2. amend the policy to allow specifically enumerated kernel/loader boundary
   modules, then add an unsafe-inventory gate that fails on any new location.

Do not weaken `shell`'s `forbid(unsafe_code)`. Its zero-unsafe core is a valuable
teaching and security property.

### DR-12 — Medium maintainability defect — the shell executor violates the project's strict SOLID rule

The shell uses a good typed Command/Interpreter pattern, but
[`shell/src/verbs.rs`](../../os/src/shell/src/verbs.rs) has one roughly
314-line `execute` function that performs authorization, validation, state
mutation, and rendering for all 22 verbs. Adding a verb changes `VerbKind`,
`Request`, `Request::kind`, parser bindings, the central match, policies, and
tests.

The repository's own standard flags functions over roughly 50 lines and permits
a central match only when it is a thin dispatch point. This one is not thin.
It is therefore a direct Single Responsibility/Open-Closed exception that has
not been acknowledged as one.

Refactor toward small command handlers over segregated interfaces:

```text
parse → typed request → policy → dispatch table → handler
                                          ├─ reads VolumeView
                                          ├─ mutates VolumeEdit
                                          ├─ reads TaskView
                                          └─ writes RenderSink
```

The central dispatcher may remain exhaustive, but each arm should delegate in
one line. This also removes the non-test `unreachable!` at the end of the
current interpreter and makes per-verb WCET/authority testing possible.

### DR-13 — Medium — IPC, filesystem, audit, and host console are models, not integrated kernel services

These are not surprise failures; they are important maturity boundaries:

- [`kernel/src/ipc.rs`](../../os/src/kernel/src/ipc.rs) is a fixed-capacity,
  directional FIFO with a policy seam. It has no blocking/wakeup,
  cancellation, endpoint teardown, queue charging, delegation, or live
  capability space. Its default `AllowAllPolicy` is suitable for tests, not a
  production default.
- [`shell/src/volume.rs`](../../os/src/shell/src/volume.rs) is a deterministic
  in-memory repository. TinyOS has no persistent filesystem, block cache,
  crash consistency, storage driver, mount namespace, or recovery path.
- [`kernel/src/spoor_journal.rs`](../../os/src/kernel/src/spoor_journal.rs)
  overwrites the oldest record when full. That is a useful bounded ring, but
  critical audit evidence needs overflow policy, durable export, integrity,
  actor generation, and loss signalling.
- The host Tauri console uses signed grant tables, refuses remote origins, and
  derives tab identity from the invoking webview. Those are strong inner
  defences and a good capability-UI lesson. The tabs are host threads/worlds,
  not TinyOS processes; the dev key is not a custody model; the reserved
  webview is not an on-target secure-attention path; no console run has yet
  captured a kernel spoor.

The documentation mostly states these boundaries correctly. Keep them explicit
in feature names and dashboards.

### DR-14 — Medium — executable parsing and unsafe boundaries need adversarial tools, not only examples

PE parsing, ACPI, ESR decoding, DOS parsing, manifest verification, timing
envelopes, and assurance TSV parsing have thoughtful example/property tests,
but the cover note's requested hostile campaign is not yet present:

- no committed fuzz targets or minimized corpus;
- no sanitizing host harness around unsafe IAT/page-table operations;
- no differential PE parser/canonicalizer;
- no mutation campaign over lengths, overlaps, terminators, imports, and RVAs;
- no model checking of scheduler/lock state transitions;
- no loom-style host concurrency campaign for console state;
- no supply-chain/key-custody qualification.

The next assurance gain should come from generating cases the authors did not
choose, not adding more hand-selected happy and negative examples.

## 5. Textbook concept map: textbook → TinyOS → code → maturity

The “TinyOS interpretation” column is deliberately exact about whether the code
is a model, mechanism, or enforced abstraction.

| Textbook concept | TinyOS interpretation | Code to read | Review maturity |
|---|---|---|---|
| **Process abstraction** (OSTEP: The Process) | `Tcb` contains identity, state, entry, priority, budget, and optional address-space root | [`sched.rs`](../../os/src/kernel/src/sched.rs) | Strong task model; currently a kernel thread, not a protected process |
| **Process creation** (OSTEP: Process API) | fixed-capacity TCB creation; no fork/exec; embedded PE creates one task | [`sched.rs`](../../os/src/kernel/src/sched.rs), [`os/main.rs`](../../os/src/os/src/main.rs) | Bounded creation mechanism; no general lifecycle |
| **Limited direct execution** | save/restore a task stack and registers; active CR3 before resume | [`context.rs`](../../os/src/kernel/src/context.rs), [`dispatch.rs`](../../os/src/kernel/src/dispatch.rs) | Direct execution exists; “limited” does not, because code remains CPL 0 |
| **Mode switch / system calls** | not implemented; normal `ret` enters workload | [`context.rs`](../../os/src/kernel/src/context.rs), [`gdt.rs`](../../os/src/hal-x86_64/src/gdt.rs) | Critical missing protection mechanism |
| **CPU scheduling** (OSTEP: Scheduling) | static priority 0–31, highest Ready wins, O(N) scan | [`sched.rs`](../../os/src/kernel/src/sched.rs), [`dispatch.rs`](../../os/src/kernel/src/dispatch.rs) | Clear fixed-priority teaching implementation |
| **Preemption** | local-APIC timer compares running task with strictly higher Ready task | [`preempt.rs`](../../os/src/kernel/src/preempt.rs), [`interrupts.rs`](../../os/src/hal-x86_64/src/interrupts.rs) | Real QEMU mechanism; no equal-priority fairness |
| **Multiprocessor scheduling** | ACPI topology records CPUs; scheduler and globals assume one CPU | [`acpi.rs`](../../os/src/hal-x86_64/src/acpi.rs), [`topology.rs`](../../os/src/hal/src/topology.rs) | Discovery only; no SMP scheduler, locks, IPIs, shootdown, affinity |
| **Real-time task model** (Liu) | priority, execution budget, overrun policy; separate actuation deadline object | [`wcet.rs`](../../os/src/kernel/src/wcet.rs), [`actuation.rs`](../../os/src/kernel/src/actuation.rs) | Budget/failure-policy model; no `C,T,D,B` admission or replenishment |
| **Priority inversion / PIP** | contended lock boosts holder; effective priority is `max(base,inherited)` | [`lock.rs`](../../os/src/kernel/src/lock.rs), [`fixture_priority_inversion.rs`](../../os/src/kernel/src/fixture_priority_inversion.rs) | Good classic three-task lesson; the publication race is repaired and 100/100 repetitions passed; inheritance is still per-task rather than per-lock contribution |
| **Threads and context** | callee-saved state lives on each task stack; extended x86 state has separate save/restore | [`context.rs`](../../os/src/kernel/src/context.rs), [`extended_state.rs`](../../os/src/hal-x86_64/src/extended_state.rs) | Compact and readable; full user/interrupt frame still missing |
| **Synchronization** (OSTEP: Locks) | non-blocking `try_lock`; caller parks/retries; interrupt masking protects scheduler mutations | [`lock.rs`](../../os/src/kernel/src/lock.rs), [`interrupts.rs`](../../os/src/hal-x86_64/src/interrupts.rs) | Lock bookkeeping, not complete blocking mutex/condition-variable system |
| **Deadlock** | no wait-for graph, lock ordering, ceiling protocol, timeout, or deadlock detection | lock fixtures and design docs | Not implemented; multiple-lock support exposes the need |
| **Free-space management** (OSTEP) | fixed array of `MaybeUninit` slots, first-fit scan, typed exhaustion, generational handles, retirement on generation exhaustion | [`mem.rs`](../../os/src/kernel/src/mem.rs) | Excellent bounded object-pool lesson: capacity and temporal identity are now separate, and stale handles fail after slot reuse |
| **Address spaces** (OSTEP) | one PML4 per task, exact section mappings, shared kernel directories | [`address_space.rs`](../../os/src/exec/src/address_space.rs), [`paging.rs`](../../os/src/hal-x86_64/src/paging.rs) | Active mechanism; not a user protection boundary |
| **Paging** | four-level 4 KiB page tables, create/walk/translate/protect/unmap | [`paging.rs`](../../os/src/hal-x86_64/src/paging.rs) | Good minimal x86 paging implementation; no huge-page policy or table reclamation |
| **TLBs** (OSTEP: Faster Translations) | CR3 compare/reload plus `invlpg` after leaf unmap or permission tightening | [`paging.rs`](../../os/src/hal-x86_64/src/paging.rs), [`context.rs`](../../os/src/kernel/src/context.rs) | Correct local invalidation foundation for the present single CPU; no PCID policy, global-page policy, or remote SMP shootdown protocol |
| **Memory protection** | NX, supervisor write-protect, per-section W/X flags, alias sealing, user/supervisor leaf and ancestor construction | [`kernel_map.rs`](../../os/src/exec/src/kernel_map.rs), [`address_space.rs`](../../os/src/exec/src/address_space.rs) | Page tables now distinguish task-user mappings from supervisor kernel/APIC mappings; there is still no CPL-3 transition, syscall boundary, or guard-page policy |
| **Page faults / exceptions** | IDT stubs capture selected x86 faults; pure disposition retires task-context or halts kernel-context; double-fault IST | [`fault.rs`](../../os/src/hal-x86_64/src/fault.rs), [`kernel/fault.rs`](../../os/src/kernel/src/fault.rs), [`tss.rs`](../../os/src/hal-x86_64/src/tss.rs) | Strong capture/decision separation; only selected vectors; software task attribution |
| **Interrupts and devices** | IDT/APIC timer, serial TX, ACPI, read-only PCI bus-0 enumeration | [`interrupts.rs`](../../os/src/hal-x86_64/src/interrupts.rs), [`pci.rs`](../../os/src/hal-x86_64/src/pci.rs) | Real Tier-0 mechanisms; no I/O APIC, device drivers, RX, DMA/IOMMU |
| **IPC / message passing** | bounded directional ring, fixed payload, policy seam | [`ipc.rs`](../../os/src/kernel/src/ipc.rs) | Strong data-structure model; no live capability endpoints or blocking |
| **Shared memory** | owner/sharee, rights subset, generational token, transactional grant and revoke, local translation invalidation | [`shared_memory.rs`](../../os/src/exec/src/shared_memory.rs) | One of the strongest PD models; failed multi-page revoke is retryable without partial authority loss, but task-death integration, intermediate-table reclamation, and SMP shootdown remain |
| **Executable loading** | PE64 → descriptor, bounded entry/import validation, section mapping, complete named/ordinal import model, allowlist, IAT traps, W^X seal | [`pe.rs`](../../os/src/exec/src/pe.rs), [`iat.rs`](../../os/src/exec/src/iat.rs), [`win32_shim.rs`](../../os/src/exec/src/win32_shim.rs) | Loader admission is materially hardened and fail-closed; the larger RCG chain, signature verification, disposable parser domain, and staged typestate are still absent |
| **Dynamic linking / ABI** | named Win32 subset maps to fixed kernel trampolines; ordinal imports remain explicit and are trapped/rejected by policy | [`win32_shim.rs`](../../os/src/exec/src/win32_shim.rs), [`iat.rs`](../../os/src/exec/src/iat.rs) | Complete import accounting is now teachable; direct CPL-0 trampolines are still not a protected syscall ABI |
| **Protection / capabilities** (Operating System Concepts) | deny-by-default policy traits, grant sets, signed host manifest, Security Charter PD model | [`shell/lib.rs`](../../os/src/shell/src/lib.rs), [`SECURITY_CHARTER.md`](../../SECURITY_CHARTER.md), host `authority.rs` | Strong design vocabulary and host model; kernel capability space absent |
| **Provenance / information flow** | immutable labels on in-memory files; copy/move preserve labels; host origins and signer/trust fields | [`shell/lib.rs`](../../os/src/shell/src/lib.rs), [`volume.rs`](../../os/src/shell/src/volume.rs) | Good taint/provenance teaching model; not system-wide or persistent |
| **Audit** | 64-bit `Spoor` atom plus bounded ring journal; console denial log is separate | [`spoor.rs`](../../os/src/kernel/src/spoor.rs), [`spoor_journal.rs`](../../os/src/kernel/src/spoor_journal.rs) | Typed and compact; overwrite/loss/durability/integration remain |
| **Files and directories** (OSTEP: Persistence) | fixed-capacity RAM volume with dirs/files/labels | [`volume.rs`](../../os/src/shell/src/volume.rs) | Deterministic repository model only; no persistent storage stack |
| **Shell / command interpreter** | DOS grammar compiles to typed request IR shared by batch and host tabs | [`dos.rs`](../../os/src/shell/src/dos.rs), [`verbs.rs`](../../os/src/shell/src/verbs.rs), [`batch.rs`](../../os/src/shell/src/batch.rs) | Excellent adapter lesson; central executor too large; no target-interactive path |
| **I/O architecture / HAL** | architecture-neutral topology/device/time/actuation traits; x86 and ARM backends | [`hal`](../../os/src/hal/src), [`hal-x86_64`](../../os/src/hal-x86_64/src), [`hal-arm64`](../../os/src/hal-arm64/src) | Good ports-and-adapters split and shared conformance; ARM is compile/host-tested, not run |
| **ARM exception levels** | firmware handoff, EL2→EL1 drop, PL011 TX, generic timer read, vector table/fault report | [`hal-arm64`](../../os/src/hal-arm64/src) | Rich host-tested implementation; never executed on target; CPACR/emergency-stack loose ends |
| **Boot trust / updates** | fully specified C0/RCG model | [`SECURITY_CHARTER.md`](../../SECURITY_CHARTER.md), [`code-admission-gates.tsv`](../../goals/security/code-admission-gates.tsv) | Specification only: no verified boot, signature chain, revocation, rollback, A/B recovery |
| **Assurance / V-model** | Goal→Epic→Feature→Story→Test→Report graph and executable consistency checker | [`goals`](../../goals), [`xtask/src/assurance.rs`](../../os/src/xtask/src/assurance.rs) | Project-defining strength; structural evidence must not be confused with platform proof |

## 6. Design-pattern map and what each pattern teaches

TinyOS is most pedagogically successful when a pattern makes a textbook
boundary explicit.

| Pattern | TinyOS example | Learning value | Review note |
|---|---|---|---|
| **Strategy / Policy** | `VerbPolicy`, `ChannelPolicy`, `CapabilityPolicy`, `OverrunHandler`, `CycleSource`, `Timebase`, `OutputLine`, `ConfigSpace`, `Mmio` | Separates mechanism from policy; makes denials and fake hardware host-testable | One of the project's strongest recurring patterns |
| **Adapter** | DOS syntax → `Request`; x86/ARM time sources → `CycleSource`; PCI port I/O → `ConfigSpace`; Tauri runtime identity → authority resolver | Shows that compatibility syntax or hardware details need not contaminate the semantic core | Preserve for POSIX/RT shell flavours |
| **Command + Interpreter** | `Request` enum and `verbs::execute` | Makes parsed commands typed, exhaustive, deterministic | Split large handlers so the interpreter remains thin |
| **Ports and Adapters / Hexagonal architecture** | architecture-neutral `hal`; `fmt::Write` sinks; policy traits; host fake MMIO | Keeps unsafe/device I/O at the edge and decisions in a host-testable core | The actual unsafe inventory should be made to match this architecture |
| **Functional core, imperative shell** | pure `tick_outcome`, `switch_plan`, fault disposition, ESR decode; unsafe wrapper performs hardware effect | Lets learners test the decision table without emulating the device/CPU | Excellent pattern for safety-critical code |
| **Object Pool** | `Pool<T, N>` with `PoolHandle { index, generation }` | Bounded allocation, explicit exhaustion, no heap, and lifetime-safe lookup after slot reuse | The pool now teaches both spatial capacity and temporal identity; exhausted generations retire rather than alias |
| **Newtype** | `Priority`, generational `TaskId`, `WcetBudgetTicks`, `TeardownGeneration`, grant tokens | Prevents unit/identity confusion and centralizes validation | `TaskId` now carries slot generation; the remaining lesson is that typed identity still needs kernel-derived authority |
| **State Machine** | `TaskState`, WCET dispositions, tab kinds, manifest verification, batch echo state | Makes legal states and total decisions visible | Task transitions are distributed among callers; a transition API would make invariants stronger |
| **RAII / ownership** | `Pool::Drop`, `AddressSpace::Drop`, verified-manifest type | Uses Rust lifetime/ownership to make cleanup and “verified only” construction local | `AddressSpace::teardown` uses `ManuallyDrop`; production lifecycle needs one auditable owner |
| **Transactional update / rollback** | shared-memory grant rolls back a mapped prefix; revoke validates and preflights the whole range before any leaf or registry mutation | Teaches atomic failure, authority retention, and retryable repair | Extend the transaction to intermediate page-table-frame reclamation and, before SMP, coordinated remote invalidation |
| **Generation token** | `SharedGrant`, `GrantRegistry`, `PoolHandle`, `TaskId`, and `TeardownGeneration` | Shows revoke-before-reuse, stale-token rejection, and explicit exhaustion | This pattern is now consistently reused at the reviewed identity boundaries |
| **Repository** | `RamVolume` | Isolates filesystem semantics from storage implementation | Good for shell teaching; do not mistake it for a persistent filesystem |
| **Decorator** | committed journaling `VerbPolicy` wrapper | Adds audit without changing authorization semantics | The policy seam now records denials while preserving the wrapped verdict; parity evidence corroborates one denial and one journal entry |
| **Template/conformance suite** | shared `CycleSource` tests used by x86 and ARM | Gives a concrete Liskov-substitution test | Expand this approach to task priority display, page-table backends, and shell front ends |
| **Facade / Application service** | `World`, `AddressSpace`, host `ConsoleState` | Offers a coherent use-case surface over several structures | Watch facade growth; `World` currently mixes several capabilities |
| **Event log / ring journal** | `SpoorJournal` | Makes security/scheduling decisions observable and ordered | Overwrite must be explicit evidence loss, not silent durability |
| **Typestate** | `SignedManifest → VerifiedManifest` | Invalid unverified state cannot call verified-only APIs | Apply the same pattern to the executable loader sequence |
| **Dependency inversion** | scheduler stores CR3 value without depending on `AddressSpace`; HAL consumes traits | Reduces cross-crate cycles and permits fakes | Do not let “parallel external array keyed by index” replace safe ownership |

Two anti-patterns deserve explicit teaching labels:

1. **Parallel-array task state.** Shipping/fixture composition keys contexts,
   stacks, entry data, counters, and task metadata by the same raw slot index.
   This is compact but makes lifetime/generation agreement a global invariant.
   A generational task slot owning or borrowing all task-local state is safer.
2. **Boolean accumulation after a failed gate.** Diagnostic fixtures may collect
   several independent checks, but the shipping loader must not continue after an
   admission failure. This review changed its policy denial, entry proof, patch
   summary, and W^X audit paths to return before activation or later effects.

## 7. Protection Domain contracts mapped back to implementation

The Security Charter's `PD-*` table is an excellent target architecture. The
following table is the implementation truth at the reviewed commit.

| Contract | Present mechanism | Missing enforcement / verdict |
|---|---|---|
| `PD-01` private active address spaces | active per-task CR3, exact user-marked section/shared mappings, supervisor-only kernel/APIC mappings | **Not met as isolation:** the U/S page-table foundation exists, but workloads still enter at CPL 0 and there are no guard pages or negative CPL-3 boundary fixtures |
| `PD-02` kernel-derived caller identity | TCB/`CURRENT_TASK` bookkeeping; host webview identity is runtime-derived | **Not met in kernel API:** IPC/lock accept actor facts; fault task context is software state |
| `PD-03` empty authority first | deny-all/grant policies; signed host manifest | **Model only:** no kernel capability space installed before user entry |
| `PD-04` sealed executable memory | NX/WP, W^X section checks, checked executable entry, alias seal and page-table walk | **Strong mapping mechanism, incomplete boundary:** entry bypass is closed, but there is no user mode and privileged workload code can still rewrite page tables |
| `PD-05` typed bounded IPC | fixed payload, bounded directional channel, policy trait | **Model only:** no live endpoints/capability lookup, blocking, teardown, charging |
| `PD-06` generation-safe shared memory | owner/sharee/rights/generation, atomic grant rollback, preflighted transactional revoke, local `invlpg` | **Strong partial for one CPU:** failed revoke preserves authority and mapped leaves; no task-death integration, intermediate-table reclamation, or remote SMP shootdown |
| `PD-07` temporal isolation | priorities, tick budgets, overrun dispositions | **Not met:** no periods, replenishment, affinity, ceilings, admission; depleted task may remain Ready |
| `PD-08` finite charged resources | fixed pools/capacities and loud exhaustion | **Partial:** capacities are global structures, not consistently per-domain charged quotas |
| `PD-09` caller-funded broker work | no production broker path | **Absent** |
| `PD-10` device-bound DMA/IRQ/MMIO | PCI enumeration; APIC mapping | **Absent as isolation:** no C2 driver domain, IOMMU/bounce buffer, generation/revoke |
| `PD-11` non-increasing provenance | shell labels survive in-memory transformations | **Model only:** no IPC/storage/system-wide enforcement or durable origin partition |
| `PD-12` fault containment | x86/ARM capture, pure disposition, task-retire vs kernel-halt, double-fault IST | **Partial:** selected vectors only; hostile task already CPL 0; no parser-domain boundary |
| `PD-13` revoke/wipe/generate before reuse | address-space staging wipe; grant revocation/generation; generational pool/task identity; exhaustion-safe teardown generation | **Partial:** stale slot identity is closed, but there is no unified task teardown that revokes capabilities, IPC, devices, and all mappings before reuse |
| `PD-14` no ambient namespace/class authority | shell and host policies deny by default; shell kill authority is explicit and independent of priority | **Model only:** the priority/authority conflation is closed in the shell model, but there is still no kernel object capability namespace |

The same conclusion applies to the `RCG-*` remote-code chain. TinyOS implements
useful pieces of `RCG-04` (bounded PE structure and executable-entry proof),
`RCG-07` (complete named/ordinal import accounting with fail-closed policy),
`RCG-10` (fresh section mapping), and `RCG-11` (W^X seal). It does not yet have
remote ingress/quarantine, disposable C4 parsing, signature/trust-path,
revocation/anti-rollback, policy intersection with a kernel capability space,
destroy-and-recreate promotion, ring-3 activation, or runtime blast-radius
containment. Call the current code an **executable-mechanism slice**, not the
complete code-admission chain.

## 8. Subsystem-specific review

### 8.1 Boot, HAL, and fault architecture

Strengths:

- unsafe hardware actions are generally wrapped by ordinary data
  construction/decoding that is host-tested;
- x86 descriptor/table layouts, fault frames, TSS, IST, APIC timer, TSC/PIT,
  ACPI, PCI CAM, and serial output are small and documented;
- ARM ESR decoding is unusually thorough and treats unknown encodings
  explicitly;
- ARM PL011 uses an MMIO trait, bounded polling, checked baud computation, and
  tests ordering/timeout behaviour;
- the shared time conformance suite demonstrates Liskov substitution rather
  than merely asserting it.

Boundaries:

- x86 handles a selected fault set and lacks I/O APIC routing;
- UART is TX-only;
- ARM64 has host tests and a target build contract, but no executed QEMU or
  hardware record; `LE-27`, `LE-37`, and `LE-38` remain material;
- neither architecture has a complete user-entry/return ABI;
- topology discovery is not SMP support;
- no DMA/IOMMU/device-domain lifecycle exists.

### 8.2 Scheduler and real-time path

Strengths:

- priority is a validated newtype and never silently clamps;
- effective priority is derived as `max(base, inherited)`, which correctly
  prevents WCET degrade and inheritance from overwriting one another;
- overrun policy is a total enum with no “ignore” default;
- tick decisions are split from unsafe switching;
- fixtures exercise real timer preemption, runaway containment, trip,
  restart, degrade, inheritance, and actuation.
- the priority-inversion fixture now publishes release state atomically with
  task completion; the repaired path passed 100/100 repeated boots.

Boundaries:

- first-free pool allocation and O(N) ready scan are bounded but scale with
  configured capacity; their worst cases must be included in response-time
  analysis;
- equal priorities have no fairness;
- lock contention does not itself park/wake a waiter;
- multiple locks are not modeled correctly for PIP;
- caller-supplied priority/identity weakens the API;
- budget windows are not periodic replenishment;
- global `static mut` state and single-core interrupt masking are the
  concurrency model.

### 8.3 Memory, paging, and teardown

Strengths:

- no heap in the shipped crates;
- fixed pools fail with typed exhaustion;
- page mapping is exact and read-back is used as evidence;
- W^X checks include writable-alias sealing;
- address-space construction rolls the whole tree back through ownership when
  creation fails;
- shared grants check owner mapping and permissions and reject partial leaf
  grants;
- pool/task handles reject stale generations after slot reuse and retire an
  identity slot instead of wrapping its generation;
- multi-page revoke preflights before mutation, retains registry authority
  until every leaf is removed, and invalidates local translations.

Boundaries:

- user/supervisor construction is present, but no CPL-3 transition consumes it
  yet and guard-page policy remains absent;
- page-table frames are represented by pool objects whose raw addresses are
  treated as physical addresses — valid for the current identity-mapped
  Tier-0 environment, not a general physical-memory manager;
- no page ownership database, demand paging, copy-on-write, swap, NUMA, or
  cache-coloring exists;
- intermediate table reclamation and remote SMP TLB shootdown are unspecified;
- task teardown is a partial address-space/staging operation, not `PD-13`'s
  complete lifecycle.

### 8.4 Executable admission and compatibility

Strengths:

- bounded capacities avoid allocation and reject excess;
- PE signatures, machine type, truncation, section file bounds, section count,
  import count, import-directory bounds, executable entry, canonical ranges,
  and W+X sections are checked;
- sections receive exact declared mapping permissions;
- IAT slots are derived from `FirstThunk`, while names can come from ILT;
- real and trap trampolines have a policy seam;
- named and ordinal thunks are represented explicitly, so no import cell
  disappears from accounting;
- the loader seals staging aliases and walks mappings for W^X;
- TXE repacking and deterministic probe generation make useful canonical
  teaching artifacts.

Boundaries:

- the parser/loader sequence is not yet encoded as typestates, so call ordering
  is still a review invariant;
- no signature, content hash, signer purpose, revocation, anti-rollback, or
  dependency hash closure;
- Win32 trampolines are kernel calls at CPL 0, not mediated syscalls;
- fuzzing and hostile-corpus evidence are absent.

### 8.5 Shell and provenance

Strengths:

- `#![forbid(unsafe_code)]`, `no_std`, fixed capacities;
- parser totality tests and typed requests;
- deny-by-default policy seam;
- kill authority is an explicit field independent of scheduling priority;
- the policy decorator records denial spoor without changing the verdict;
- one semantic core for batch and host tabs;
- hostile output is rendered inert;
- labels survive file transformations;
- deterministic directory and parity transcripts;
- capacity/traversal/read-only failures are typed and loud.

Boundaries:

- central executor violates the project's own size/SOLID guidance;
- shell denial evidence is still a bounded in-memory model, not a durable,
  exported kernel audit channel;
- `RamVolume` is not storage;
- no live UART input or on-target tab host;
- POSIX and RT front ends remain future work.

### 8.6 Host Tauri fork and console

Strengths:

- upstream behaviour remains the default when the TinyOS hook is absent;
- local vs remote origin is explicit;
- manifest verification produces a construction-gated `VerifiedManifest`;
- command grants and tab identities are signed and enumerated;
- remote origins are unconditionally refused;
- tab commands derive their session from the invoking webview label;
- chrome and tab grants are disjoint;
- reserved-region identity carries no verbs;
- navigation revokes/cancels in-flight authority in the tested stages;
- 48 host tests passed in this review (2 hardware/QEMU tests ignored).

Boundaries:

- this is a Windows host application and optional application lane, not the
  TinyOS tab host;
- development signing key material is present by design;
- `Mutex::lock().expect(...)` and similar panic paths are acceptable only under
  an explicit host-PoC policy, not the kernel fail-safe rule;
- the test build emits unused-import warnings in stages A/C/D;
- generated Tauri permission/schema files are build outputs and need a clean
  reproducibility gate;
- the console parses Cargo human output;
- no kernel spoor is in the tab evidence;
- webview isolation is defence in depth, not the future kernel boundary.

### 8.7 Assurance system

Strengths:

- traceability is machine-checked rather than a spreadsheet convention;
- status vocabulary, joins, dashboard badges, loose ends, evidence links,
  platform qualification, bound provenance, external isolation, crate sizes,
  and performance catalogue completeness have executable checks;
- negative unit tests demonstrate that the checker rejects inconsistent
  claims;
- reports generally state non-claims prominently.

Boundaries:

- structural consistency cannot inspect whether a test actually proves its
  prose claim;
- all 67 Feature/Story status rows agreeing can coexist with 0 qualified
  platforms;
- “11 release gates with evidence” is not a complete release;
- the security current-state review is already stale: it predates live task
  CR3/W^X and generational shared grants, while the CPL-0 boundary remains;
- new review findings should become explicit loose ends/stories, otherwise the
  graph will remain green without tracking them.

## 9. Verification performed for this review

Commands were executed on the reviewed Windows host unless stated otherwise.

### 9.1 Baseline audit before the enhancement tranche

This first table records the repository exactly as found. Its failures are not
current failures; they are the evidence that drove the repairs.

| Verification | Result |
|---|---|
| `cargo fmt --all -- --check` | **Pass** |
| `cargo run -p xtask -- check-assurance-spine` | **Pass:** 27 Features, 67 Stories, 52 Tests, 55 Reports, 56 loose ends / 32 open, 0 qualified platforms, 0 bound claims |
| `cargo run -p xtask -- check-crate-sizes --ceiling=20000` | **Pass:** largest `kernel` 9,955; `xtask` 7,487; all below 20,000 |
| `cargo run -p xtask -- check-performance-catalogue` | **Pass:** 625/625 cells |
| `cargo run -p xtask -- check-shell-parity` | **Pass:** QEMU fixture result plus normalized 61-line golden |
| `cargo test -p xtask` | **Pass:** 204 |
| `cargo test --workspace` | **Fail:** all reported `exec` 75, `hal` 13, ARM 115, x86 97, kernel 139 passed before shell golden failure stopped the workspace |
| `cargo test -p shell --lib` | **Fail:** 21 passed, 1 failed, 1 ignored; CRLF/LF mismatch |
| `cargo clippy --workspace --all-targets -- -D warnings` | **Fail:** known Windows target-fixture imports compiled-out x86 modules |
| Focused QEMU fixtures: context switch, address-space, address-space-switch, W^X seal, shared memory, first task, OS, runaway OS, shell batch | **9/9 passed** |
| `priority-inversion` repetition | **42/50 passed; 8/50 failed**, all exit 1; retained trace diagnoses publication race |
| `external/tauri/tinyos-poc: cargo test --workspace` | **Pass:** 48 passed, 2 ignored; unused-import warnings |

### 9.2 Verification after implementing the review

| Verification | Result |
|---|---|
| `cargo fmt --all -- --check` | **Pass** |
| `cargo clippy --workspace --lib --tests -- -D warnings` | **Pass** |
| `cargo test --workspace` | **Pass:** 701 passed, 0 failed, 1 deliberately ignored: `exec` 86, shared HAL 13, ARM HAL 115, x86 HAL 101, kernel 143, shell 33, `xtask` 210 |
| Focused QEMU fixtures: double fault, address space, address-space switch, W^X seal, shared memory, Blue Sharc, first task, OS, runaway OS, priority inversion, shell batch | **11/11 passed** |
| `priority-inversion` repaired-path repetition | **100/100 passed**, replacing the baseline 42/50 result |
| Real Blue Sharc import census | **Pass:** 220 imports remain visible to policy—IAT: 205 named and 15 ordinal |
| Real first-task containment | **Pass:** 7 imports granted, 213 explicitly trapped; the first unsupported call reached `0xDEAD0000` and retired the task |
| `cargo run -p xtask -- check-assurance-spine` | **Pass:** 27 Features, 68 Stories, 52 Tests, 56 Reports, 56 loose ends / 31 open, 0 qualified platforms, 0 bound claims |
| `cargo run -p xtask -- check-shell-parity` | **Pass:** normalized 64-line golden, in-guest assertions, and `len=1 denials=1` spoor corroboration |
| `cargo run -p xtask -- check-crate-sizes --ceiling=20000` | **Pass:** largest `kernel` 10,003; all crates below 20,000 |
| `cargo run -p xtask -- check-performance-catalogue` | **Pass:** 625/625 cells |
| Review cross-reference validation | **Pass:** 138 local Markdown links resolve; `git diff --check` passes |

The review did not rerun all 30 fixtures because the 11-fixture set covers the
changed shipping, memory, admission, fault, shell, and scheduler paths, while
the existing soak records the full catalogue. No hardware qualification,
multi-core invalidation proof, or timing bound was attempted; QEMU cannot make
those claims valid.

## 10. Recommended repair order

### P0 — correct the trust boundary before expanding ingress

1. Record DR-01 through DR-10 as owned loose ends with tests and claim impact.
2. **Completed in this review:** state explicitly that current scheduled
   workloads are trusted CPL-0 kernel tasks.
3. **Foundation completed, boundary still open:** U/S mappings, CPL-3 GDT
   selectors, and a TSS ring-0 entry stack now exist. Add the first `iretq`
   transition, full user-origin trap frame, and a minimal syscall ABI.
4. Prove negative isolation in QEMU: kernel write, peer write, APIC access,
   privileged instruction, forged syscall actor, and task fault.
5. **Completed in this review:** reject entry points outside an admitted
   executable section and terminate the shipping pipeline on every admission,
   entry, patch, or W^X failure.
6. **Completed in this review:** model ordinal imports explicitly, keep every
   IAT slot in the policy surface, and bound import-directory and file-backed
   walks.
7. **Completed in this review:** make `PoolHandle`/`TaskId` generational and
   retire exhausted generations rather than wrap.

No external executable, remote payload, general application, or hostile parser
should be activated before this phase closes.

### P1 — make lifecycle and authority complete

1. Introduce a kernel capability space that starts empty.
2. Derive current actor from the running TCB/domain generation.
3. Convert IPC and lock entry points from supplied identity to current-domain
   plus endpoint capability.
4. Implement full task teardown: stop scheduling/ingress, revoke IPC/grants,
   invalidate translations, wipe, advance generations, then reuse.
5. **Single-core half completed:** local leaf invalidation is explicit. Design
   and prove remote SMP shootdown before enabling a second CPU.
6. Add per-domain object/page/message/table-frame quotas.
7. Keep containment class, priority, provenance, and authority separate in
   types and policy. The shell priority/kill-authority conflation is repaired;
   carry the separation into the future kernel capability namespace.

### P2 — turn scheduling mechanisms into a real-time model

1. Add release/period/deadline/replenishment/affinity fields and states.
2. Make depleted tasks ineligible until replenishment.
3. Add admission/response-time analysis including blocking and scheduler/IRQ
   overhead.
4. Replace single inherited priority with per-resource contributions or a
   proven ceiling protocol.
5. Add wait queues and deterministic wakeup/timeout/cancellation.
6. **Completed in this review:** fix the priority-inversion publication race;
   the repaired fixture passed 100/100 repeated boots.
7. Qualify clock/timer/platform evidence on real hardware before binding WCET
   or deadline claims.

### P3 — complete adversarial assurance

1. Structure-aware PE/TXE/ACPI/DOS/manifest fuzz targets with retained corpus.
2. Differential canonicalization and malformed overlap/length campaigns.
3. Unsafe inventory gate and explicit kernel/loader boundary policy.
4. Workspace panic/unwrap/expect inventory with justified exceptions.
5. Cross-platform line-ending and all-target build fixes.
6. Supply-chain lock/provenance review and production key-custody design.
7. Hardware fault injection, DMA/IOMMU tests, and platform qualification.
8. Evidence-loss tests for spoor overflow/export/durability.

### P4 — integrate shell, storage, and console only over the kernel boundary

1. Split command handlers and fix task-control authorization.
2. Add persistent storage as an isolated C2 service with crash/recovery tests.
3. Add UART/input path only through bounded, capability-checked endpoints.
4. Journal shell denials as kernel spoors without coupling the shell core to
   the kernel; the `04A` decorator plan is the right shape.
5. Build the on-target tab host only after the reserved-region and secure-input
   boundary can be enforced below tab content.
6. Keep the Tauri console as host tooling/application evidence, not kernel
   qualification.

## 11. A learning path through the code

For a reader using TinyOS as an OS textbook companion, this order builds one
concept at a time and makes each limitation visible.

1. **Bounded allocation:** read OSTEP free-space management, then
   [`mem.rs`](../../os/src/kernel/src/mem.rs). Exercise exhaustion, double free,
   and the missing stale-handle-after-reuse test.
2. **Task abstraction:** read OSTEP “The Process,” then
   [`sched.rs`](../../os/src/kernel/src/sched.rs). Identify which PCB/TCB fields
   exist and which process resources do not.
3. **Context switching:** read OSTEP “Limited Direct Execution,” then
   [`context.rs`](../../os/src/kernel/src/context.rs). Draw the exact stack
   before and after `ret`. Then explain why this remains ring 0.
4. **Scheduling:** read OSTEP scheduling, then `highest_priority_ready`,
   [`dispatch.rs`](../../os/src/kernel/src/dispatch.rs), and
   [`preempt.rs`](../../os/src/kernel/src/preempt.rs). Predict equal-priority
   starvation.
5. **Concurrency:** read OSTEP locks and Liu's resource-sharing section, then
   [`lock.rs`](../../os/src/kernel/src/lock.rs). Model one and two held locks;
   the second exposes `LE-49`.
6. **Real-time enforcement:** read Liu's task model and response-time analysis,
   then [`wcet.rs`](../../os/src/kernel/src/wcet.rs) and
   [`actuation.rs`](../../os/src/kernel/src/actuation.rs). Separate budget,
   WCET, deadline, and admission.
7. **Paging:** read OSTEP paging and TLB chapters, then
   [`paging.rs`](../../os/src/hal-x86_64/src/paging.rs). Hand-walk one address,
   list every permission bit, and identify the absent U/S and invalidation
   operations.
8. **Address-space construction:** read
   [`address_space.rs`](../../os/src/exec/src/address_space.rs) and
   [`kernel_map.rs`](../../os/src/exec/src/kernel_map.rs). Trace physical
   aliases before and after sealing.
9. **Exceptions:** read the x86 IDT/TSS code and
   [`kernel/fault.rs`](../../os/src/kernel/src/fault.rs). Contrast captured
   evidence, disposition policy, and actual containment.
10. **Executable loading:** read PE structure, then
    [`pe.rs`](../../os/src/exec/src/pe.rs) → `win32_shim` → `iat` →
    `AddressSpace`. Try to construct the DR-02 and DR-04 hostile images.
11. **IPC and sharing:** read OSTEP concurrency/message concepts, then
    [`ipc.rs`](../../os/src/kernel/src/ipc.rs) and
    [`shared_memory.rs`](../../os/src/exec/src/shared_memory.rs). Compare copy
    semantics with explicit page grants.
12. **Protection:** read Operating System Concepts protection chapters, then
    the 14 [`PD contracts`](../../goals/security/protection-domain-contracts.tsv).
    For each, name the hardware state that enforces it; prose and a policy
    trait are not enough.
13. **Shell architecture:** read DOS parser → `Request` → policy → executor →
    volume. Add a hypothetical POSIX adapter without changing the semantic
    core.
14. **Assurance:** trace one Story through Test and Report, then deliberately
    break a join and observe `xtask` reject it. Finally ask the crucial
    assurance question: does the test prove the claim, or only exist beside
    it?

## 12. Definition of “ready for hostile application code”

TinyOS should not use “sandboxed application” for a release until all of these
are machine-evidenced:

- first instruction begins at CPL 3 in an admitted executable section;
- kernel/peer/APIC/page-table writes from the task fault without mutation;
- privileged instructions fault and the task alone is retired;
- syscall actor is derived from the active TCB generation;
- empty capability space is observable before grants;
- every mapping is U/S-, W/X-, alias-, and guard-correct;
- every unsupported import and invalid entry is rejected before task creation;
- stale task/object/grant tokens fail after reuse;
- termination revokes and wipes all memory/IPC/capabilities before generation
  advance and reuse;
- translation invalidation is correct for every active CPU;
- CPU and service work are charged to enforceable budgets with replenishment;
- hostile parser fuzz corpus and boundary-negative fixtures pass;
- the platform running the evidence is qualified and the report binds the
  exact binary/toolchain/hardware.

Until then, TinyOS is doing something valuable and honest: building and
teaching the mechanisms from which an OS boundary is made. The next leap is not
more surface area. It is making the CPU enforce the boundary the Security
Charter already describes.

## 13. Final assessment

The project’s strongest idea is not any single kernel mechanism. It is the
combination of:

- small mechanisms that a learner can actually read;
- pure decision logic separated from unsafe effects;
- bounded structures and typed failure;
- negative QEMU fixtures;
- a machine-readable assurance spine that preserves non-claims.

The most important improvement is equally singular: **move workloads out of
CPL 0 and make every authority path cross a kernel-derived, capability-checked
entry boundary**. Once that exists, the page tables, W^X loader, IPC model,
shared grants, scheduler policies, and fault disposition can become
enforcement rather than demonstration.

This review did more than identify the defects around that boundary. It closed
the loader-entry bypass, erased-import blind spot, stale pool/task identity,
partial shared-memory revoke, local stale-TLB window, loader gate
fall-through, priority-fixture race, and shell authority/priority conflation.
It also constructed the U/S paging, CPL-3 descriptor, and TSS entry-stack
foundations needed for the next step.

Do not broaden executable ingress yet. Complete ring-3 entry/return and the
kernel-derived syscall/reference-monitor boundary first; then unify task
teardown and capability revocation, add hostile boundary fixtures and parser
fuzzing, finish the real-time task model, and qualify the result on hardware.
That sequence turns the new foundations into enforced isolation without
invalidating the excellent learning structure TinyOS already has.
