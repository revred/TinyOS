# TEST-P0-02-02-A — Context Switch Preserves Two Tasks' Independent State Under QEMU

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-02-02`](../stories/STORY-P0-02-02.md)
Tier: Tier 0 (QEMU x86_64), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D04`, `D08`
Security controls: `SEC-03`, `SEC-19`
Containment classes: `C1`, `C2`, `C3`, `C4`
Boundary tests: `BND-04`, `BND-15`, `BND-16`, `BND-17`, `BND-20`
Protection Domain contracts: `PD-01`, `PD-02`, `PD-07`, `PD-08`, `PD-09`, `PD-12`, `PD-13`
Code admission gates: `RCG-10`, `RCG-12`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

**Given** the `kernel` crate built against `os/targets/x86_64-tinyos.json` with the `fixture-context-switch` Cargo feature enabled, linking `kernel::context::{Context, switch}`,
**when** it is booted under QEMU x86_64 (`q35` machine type, PVH direct-kernel-boot) via `xtask qemu-x86_64 --fixture=context-switch`,
**then**:
- `kernel_main` initializes two task `Context`s (`task_a`, `task_b`), each on its own statically-allocated stack, via `Context::new`,
- switches into `task_a`, back to the boot context, into `task_b`, back, into `task_a` again, back, into `task_b` again, back — four switches into tasks, interleaved, each followed by a switch back to the boot context so it can drive the next step,
- each task increments its own stack-local counter (seeded to a distinct value per task) exactly once per resume and records the result before switching back,
- and reaches the `isa-debug-exit` success code only if **both** tasks' recorded values are exactly what two independent, correctly-resumed counters would produce (`task_a`: `11`, `12`; `task_b`: `1005`, `1010`) — a context switch that clobbers a callee-saved register, restores the wrong stack, or resumes a task from the wrong point would desynchronize at least one of these values, so this Test cannot pass by accident (e.g. a `switch` that is a no-op, or that always resumes the same task, fails it).

## Test type

Integration test (Tier 0, QEMU-based), per [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md#test-driven-development-mandatory)'s requirement that every driver/kernel path targets at minimum a Tier 0 test — this is exactly the "context-switch register save/restore" carve-out in the Language policy's [Boot/entry assembly](../../agent/CODING_STANDARDS.md#language-policy) bullet, so its correctness depends on real hardware/QEMU register semantics a host test alone cannot fully stand in for. Complements (does not replace) `context.rs`'s own host unit test, `switch_preserves_each_of_two_tasks_own_state_across_interleaving`, which runs the identical two-task, four-switch sequence on the host `cargo test` toolchain — safe to do only because `context_switch_asm` is pinned to the `sysv64` calling convention explicitly (not `extern "C"`, which follows the host OS's own default convention — Windows x64 on this dev machine), so the same assembly is exercised, just not under the kernel's real `no_std`/bare-metal environment.

## Implementation location

- `os/src/kernel/src/context.rs` — `Context`, `Context::new` (initial-frame construction), `switch` (the `context_switch_asm` wrapper), and the `global_asm!` routine itself.
- `os/src/kernel/src/context_switch_fixture.rs` — the two-task fixture this Test drives, only compiled under `fixture-context-switch`.
- `os/src/kernel/src/main.rs` — `kernel_main`'s `fixture-context-switch` branch, mapping the fixture's pass/fail `bool` to the `isa-debug-exit` code.
- `os/src/xtask/src/main.rs` — `qemu-x86_64 --fixture=context-switch` builds the kernel with the feature enabled and boots it, exactly as `--fixture=broken-boot` already does for `TEST-P0-01-03-A`.

## Reports

- [`REPORT-2026-07-26-08`](../reports/REPORT-2026-07-26-08.md) — Pass (local, Tier 0/QEMU, plus 2 host unit tests in `context.rs` and 61 host tests workspace-wide).
