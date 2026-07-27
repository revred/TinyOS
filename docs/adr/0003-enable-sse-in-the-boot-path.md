# ADR 0003 — Enable SSE in the Boot Path Rather Than Compiling the Kernel Soft-Float

Status: **Accepted**
Date: 2026-07-27
Introduced in: [`session/hand-2026-07-27/`](../../session/hand-2026-07-27/) (`STORY-P1-01-01`, the `EPIC-P1` measurement harness)

## Context

`STORY-P1-01-01`'s Tier 0 measurement fixture triple-faulted the moment it measured `kernel::sched::Scheduler::highest_priority_ready` — a function with no floating point anywhere, whose host unit tests pass. QEMU's own trace (`-d int,cpu_reset`) named the cause exactly:

```
0: v=06 e=0000 i=0 cpl=0 IP=0008:000000000010a140 pc=000000000010a140 ...
1: v=0d e=0032 ...
2: v=08 e=0000 ...
```

`v=06` is `#UD` (invalid opcode), escalating to `#GP` and then `#DF` — a triple fault, which QEMU turns into a silent shutdown. Disassembling that address showed the offending instruction:

```
10a140: 0f 10 00        movups (%rax), %xmm0
10a143: 0f 29 44 24 40  movaps %xmm0, 0x40(%rsp)
```

SSE2 is architecturally guaranteed on every x86_64 CPU, so LLVM freely uses `movups`/`movaps` to move 16 bytes at a time in perfectly integer code — here, copying an iterator's state in `highest_priority_ready`'s `filter`/`max_by_key` chain. But an SSE instruction raises `#UD` while `CR4.OSFXSR` is clear, and nothing in this kernel's boot path had ever set it.

Two things this uncovered are worth stating plainly:

- **A production scheduler function could not execute on the real target binary at all.** It passed every host test, and Tier 0 had never exercised it (`STORY-P0-02-05`'s dispatch work was proven by host tests; `EPIC-P0`'s QEMU fixtures never called it). The bug was found only because `EPIC-P1` started measuring on target.
- **[`ADR 0001`](0001-nightly-toolchain-for-build-std.md)'s stated rationale is inaccurate on this point.** It justifies the custom target spec partly by "no SIMD in kernel context", but [`os/targets/x86_64-tinyos.json`](../../os/targets/x86_64-tinyos.json) carries no `features` key at all — it never disabled SIMD, unlike the upstream `x86_64-unknown-none` target, which sets `-mmx,-sse,+soft-float`. The build has been emitting SSE since the first commit; only the absence of any code path reaching such an instruction on target hid it.

## Decision

**Enable SSE in the boot path**, in [`os/src/hal-x86_64/src/boot.rs`](../../os/src/hal-x86_64/src/boot.rs)'s long-mode entry, immediately before the first call into Rust code (`kernel_main`):

- `CR0`: clear `EM` (bit 2) so SSE/x87 instructions are not trapped as emulated; set `MP` (bit 1) so `WAIT`/`FWAIT` honors `TS` as the SDM specifies for a machine with a real FPU.
- `CR4`: set `OSFXSR` (bit 9), the OS's declaration that it manages `FXSAVE`/`FXRSTOR`-style state — which is what actually permits SSE instructions — and `OSXMMEXCPT` (bit 10), so an unmasked SIMD floating-point error raises `#XF` (vector 19) instead of the ambiguous `#UD` this change exists to eliminate.

The rejected alternative was to add `"features": "-mmx,-sse,+soft-float"` to the target spec, matching upstream `x86_64-unknown-none`.

## Rationale

- **Performance is a first-class goal, not an afterthought.** [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md#priority-ordering) puts squeezing maximum throughput out of the target hardware at priority 4 — above development convenience — and TinyOS's roadmap explicitly includes local AI inference (Phase 6), which is vector work by nature. Compiling the whole kernel soft-float to dodge one missing control-register bit would trade a permanent, system-wide performance ceiling for a one-time boot-code change.
- **The hardware guarantees the unit; refusing to enable it is the anomaly.** Every x86_64 CPU has SSE2. Every production x86_64 kernel enables it early. Leaving `OSFXSR` clear does not make the kernel simpler — it makes it a machine where the compiler's ordinary code generation is a latent triple fault, which is exactly what happened.
- **Fail-loud beats fail-silent.** Setting `OSXMMEXCPT` means a real SIMD arithmetic error arrives as `#XF` rather than as `#UD`, which is a distinguishable, routable signal once `FEAT-P1-02` installs real exception handling.

## Consequences

- Every binary booting through `hal_x86_64::boot` (the kernel and every Tier 0 fixture) now runs with SSE enabled. The full 12-fixture QEMU sweep was re-run after this change: every fixture's documented exit code is unchanged (`broken-boot` and `idt-apic-unrouted` still fail as designed; the other ten still pass).
- **Context switching does not yet save or restore SSE/x87 state.** `kernel::context::switch` saves the callee-saved general-purpose registers and `rflags` only. That is sound today because no task uses floating point across a switch and the compiler's `xmm` usage is confined within a function, but it becomes unsound the moment a task holds live vector state across a yield. This is named, tracked debt (`LE-14` in the `EPIC-P1` loose-ends register), owned by `FEAT-P1-04`'s preemption work, where a preempted task's state is no longer under the compiler's control.
- `FEAT-P1-02` should route `#XF` (vector 19) alongside the other real exception vectors it installs.
- ADR 0001's "no SIMD in kernel context" phrasing is superseded by this ADR. The custom target spec's real justifications remain: the disabled red zone, the small code model, static relocation, and the linker script.
