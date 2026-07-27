# Handover 04 — `STORY-P1-03-02` Implemented and Verified: W^X, Sealing, Teardown, and the First Real Task

Follows: [`03-story-p1-03-02-hardening-review.md`](03-story-p1-03-02-hardening-review.md), which rewrote this Story's acceptance criteria against nine pre-implementation defects. This handover records what was built against those criteria and what the two Tier 0 runs actually showed.

Result: **both fixtures green**, `TEST-P1-03-02-A` written, `REPORT-2026-07-28-01` filed, assurance state `specified` → `baseline-debt`. `FEAT-P1-03` is functionally complete; it does not formally exit only because the measured `D04` same-space-vs-cross-space delta is still deferred.

## The headline

TinyOS ran something. `blue-sharc.exe` — a real, externally-authored executable, repacked through the real `xtask pack-txe` pipeline, parsed by the real PE64 loader, mapped into its own real W^X address space, and scheduled as a genuine task through the production `CR3`-aware dispatcher.

What it did is better than what the Story predicted. Its real MSVC CRT startup called **`GetSystemTimeAsFileTime`** — not among the nine calls this shim allowlists — through its **unpatched** import address table. An unpatched thunk still holds the *RVA* of its `IMAGE_IMPORT_BY_NAME` record, so the indirect call took that RVA as an absolute address and fetched instructions from `0x7b_9d9e`, where the image's own bytes read `00 00 "GetSystemTimeAsFileTime"`: a hint word and an ASCII name, not code. That address is *present* in the task's tree (it falls inside the kernel's mapped low memory) but **NX**, so the fetch raised `#PF` (`rip == cr2`), the task was terminated, and the system carried on. Verified against the image's bytes, not inferred from the address.

Two things follow that are worth keeping in front of whoever picks this up:

- **The fault is not staged in any sense.** Every fault this project has caught before today came from a `ud2` someone placed or an address someone deliberately left unmapped. This one came from a real program reaching for a real capability it did not hold, by the exact mechanism ungoverned code reaches for one.
- **`EFER.NXE` and the W^X kernel map are load-bearing.** Without them that fetch would have *succeeded*, and this kernel would have begun executing an API name string as machine code. Neither bit was set anywhere in this codebase before today — `map_4k` has been writing PTE bit 63 since `STORY-P0-05-02` with no hardware honoring it. That is the single most concrete argument this project has produced for why W^X is not hygiene.

## What was built

| Component | Location |
|---|---|
| `__kernel_exec_start`/`_end`, `__kernel_rodata_start`/`_end`, `__kernel_image_end` | `os/targets/x86_64-tinyos.ld` |
| `enable_nx_and_wp`, `protect_4k`, `install_shared_pd`, `directory_addr`, `for_each_leaf` | `os/src/hal-x86_64/src/paging.rs` |
| `kernel_map::build_shared_directories`, `KernelLayout` | `os/src/exec/src/kernel_map.rs` (new) |
| `attach_shared_pd`, `seal_kernel_alias`/`unseal_kernel_alias`, `teardown`, `TeardownGeneration` | `os/src/exec/src/address_space.rs` |
| `SwitchPlan`, `switch_plan`, `run_once_in_space` | `os/src/kernel/src/dispatch.rs` (`run_once` untouched) |
| `Scheduler::address_space_of` | `os/src/kernel/src/sched.rs` |
| `SPOOR_JOURNAL_CAPACITY`; the shipping binary's fault-audit journal | `os/src/kernel/src/capacities.rs`, `os/src/kernel/src/main.rs` |
| `wx-seal-fixture`, `first-task-fixture` | `os/src/exec/src/fixture_wx_seal_main.rs`, `fixture_first_task_main.rs` |
| `--fixture=wx-seal`, `--fixture=first-task`, two CI steps | `os/src/xtask/src/main.rs`, `.github/workflows/ci.yml` |

Both Tier 0 captures are quoted verbatim in `TEST-P1-03-02-A` and `REPORT-2026-07-28-01`; they are not reproduced a third time here.

## Design decisions worth carrying forward

**Kernel mappings are shared at page-directory granularity, and that granularity is forced.** The obvious sharing unit — one top-level entry — is wrong here: the image base `0x1_4000_0000` and kernel low memory both live under PML4 slot 0, so sharing a PML4 entry or a whole PDPT would share the image across every space and undo the isolation `STORY-P1-03-01` proved. Each space keeps its own PML4 and PDPT and links two shared *directories* (kernel low memory; the local-APIC MMIO page, so the armed boot timer keeps working under every space). Sharing is proven by reading the directory's physical address back through each tree, not asserted.

**Sealing is what makes a per-entry W^X audit mean anything.** The loader copies image bytes through a writable staging alias in kernel memory, so before this Story every "immutable" executable page had a live writable view and a per-entry audit would have passed anyway. `seal_kernel_alias` re-protects the kernel view of every frame the task maps non-writable, and the audit gained an alias clause that would catch the hole's return. Teardown unseals first, because with `CR0.WP` on the wipe would otherwise fault.

**Teardown deliberately keeps the shared directories linked.** A stale-mapping probe needs a `CR3` that is still loadable; unlinking the kernel directories would make the probe fault on the instruction fetch of the probe code itself and prove nothing. Page-table frames also stay pool-allocated — documented as such, never counted as wiped. The probe runs *as a task*, because a supervisor-context `#PF` is `HaltSystem` by policy and would have ended the fixture rather than demonstrating containment.

**`run_once` was not modified.** Its existing tests are the no-regression guard for every pre-existing Story's tasks (all `None`). The `CR3` decision is a pure `switch_plan` function, host-tested on both arms, and `run_once_in_space` is the new consumer — necessary because `mov cr3` can never retire in a host process.

## What remains open

1. **The shipping `kernel` binary still discovers topology and halts.** `first-task-fixture` *reproduces* `kernel_main`'s discovery path (same calls, same success gates) rather than living inside it, because `exec` depends on `kernel` and the reverse link is a cyclic crate dependency — the constraint that created `exec-fixture` back in `STORY-P0-05-02`. **A top-level `os` binary crate that depends on both is the natural next structural step**, and it is what would let the real boot path schedule a real task. This is the largest remaining gap between what is proven and what ships.
2. **No IAT patching**, so no allowlisted Win32 call has yet resolved and returned from *inside* a loaded image — the image faults on its first reach through the IAT. This is the next thing between "a real workload is contained" and "a real workload runs," and it is a well-shaped Story: patch the IAT for allowlisted imports at load time, leave non-allowlisted thunks pointing at a deliberate trap.
3. **The deferred `D04` delta.** Measuring same-space vs cross-space dispatch cost is now a matter of running it, since a production dispatch path finally installs per-task address spaces. It is the only thing keeping `FEAT-P1-03` from a formal exit.
4. **Tier 0 only.** `LE-09` unchanged.
5. ~~**A flake worth knowing about.** Two `xtask qemu-x86_64` invocations returned exit 2 (harness error — the 15-second boot budget) when run back-to-back immediately after a full workspace compile; both pass repeatedly standalone. If CI ever shows this, it is the boot timeout under load, not a kernel regression.~~

   **Retracted 2026-07-28 (Handover 05).** This was a misdiagnosis. The `exit=2` came from a defect in the PowerShell regression sweep, not from the boot budget: a single-element argument splat leaked `-q` past `cargo` into `xtask`, which correctly reported `unknown command 'q'` and returned `HarnessError`. It only ever affected the no-`--fixture` invocation, because that is the only one whose argument array has one element. `xtask` and the kernel boot path were both correct throughout. See Handover 05's open-items list for why the "fix" this misdiagnosis nearly justified — loosening the boot timeout — was reverted.

## Verification

353 host tests across the workspace, all passing. Every Tier 0 fixture re-run for regression, all as expected (`broken-boot` and `idt-apic-unrouted` return exit 1 as their own documented pass conditions). `cargo run -p xtask -- check-assurance-spine` passes.
