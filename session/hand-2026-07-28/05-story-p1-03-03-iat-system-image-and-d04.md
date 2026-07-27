# Handover 05 — `STORY-P1-03-03`: Import Resolution, the System Image, and the D04 Switch Cost

Follows: [`04-story-p1-03-02-wx-seal-and-first-real-task.md`](04-story-p1-03-02-wx-seal-and-first-real-task.md), which closed with three named open items. This handover closes all three, plus one defect that only became visible on re-reading the previous Story's own passing evidence.

Result: **all Tier 0 fixtures green**, `TEST-P1-03-03-A` written, `REPORT-2026-07-28-02` filed. **`FEAT-P1-03` now meets every exit criterion**, including the `D04` measurement deferred twice.

## The finding: green evidence is not understood evidence

`REPORT-2026-07-28-01` reported, correctly, that `blue-sharc.exe` faulted at `rip == cr2 == 0x7b9d9e` and was contained. What that capture actually showed, read closely: nothing had patched the image's Import Address Table, so every thunk still held the RVA of its own `IMAGE_IMPORT_BY_NAME` record. The CRT's first indirect call jumped to that RVA *taken as an absolute address*, which happened to land on non-executable memory.

The outcome was right. The mechanism was arithmetic. An RVA is an attacker-visible number, and an image laid out so that some thunk's RVA collided with a mapped executable page would have transferred control there, with nothing in the system deciding otherwise.

That is now fixed, and the fix is the shape the capability model always described: every IAT slot is written exactly once at load time — a granted import gets a real, callable `extern "win64"` trampoline; every other import gets `CAPABILITY_TRAP_VIRT` (`0xdead_0000`), deliberately unmapped everywhere. The same real artifact now faults at `cr2=0xdead0000`, diagnosable from the fault line alone. `blue-sharc.exe`'s 205 imports resolve to 7 granted, 198 trapped.

**Worth carrying forward as a working lesson:** a passing test told us containment held; only reading *how* it held revealed it held for the wrong reason. This is the second time in two Stories that re-reading a capture rather than filing it produced the next unit of work.

## The other two gaps, closed

**The system image exists.** `os` is a new top-level binary crate depending on both `kernel` and `exec` — the thing `kernel`'s binary could never be, since `exec` depends on `kernel`. Its real boot path discovers hardware, retires the all-RWX bring-up map, loads a real PE64, resolves its imports under a policy granting exactly what that image declares and nothing more, seals, schedules it under its own `CR3`, and contains and journals the result:

```text
tinyos: loaded image — 2 import(s), 2 granted, 0 trapped, cr3=0x13f000
tinyos: workload returned 0xffffffffffffffff (GetCurrentProcess ok=true), exited via trap 0xdead0000, task_finished=true, spoors=5
```

`0xffffffffffffffff` is the half `blue-sharc.exe` can never demonstrate: a real loaded image's own instruction stream calling a Win32 API through a loader-patched IAT and getting the correct answer back. `G-DX-8`'s gate moved to this image (74,568 bytes against 8MiB).

The embedded workload is a 16KiB generated capability probe rather than `blue-sharc.exe`, deliberately: a system image is the OS, not the applications it runs, and embedding 8.3MiB of third-party binary plus its staging arena to re-prove what the fixture already proves would have been the wrong trade. The probe is a genuine PE32+ (real headers, real split ILT/IAT, hand-assembled code) emitted by `xtask make-probe-pe`, so it is reviewable source rather than an opaque blob, with host tests that re-derive every offset and RIP-relative displacement it claims.

**The `D04` delta is measured.** Release profile, median of three runs: `dispatch_round_same_space` p50 **276** cycles, `dispatch_round_cross_space` p50 **7,452**. The ~7,200-cycle delta is a **TCG artifact, not a hardware cost** — under emulation a `mov cr3` forces QEMU to flush its own software TLB — so it is reported and deliberately *not* gated; thresholding it would gate the emulator. It is the sharpest argument this project has produced for `LE-09`, because it is a case where Tier 0 cannot give even the right order of magnitude.

## Design decisions worth carrying forward

**The ABI is pinned at the boundary, not inherited.** Trampolines are `extern "win64"`: their callers are MSVC-compiled code passing arguments in `RCX`/`RDX`/`R8`/`R9` with a 32-byte shadow store, not the System V convention the kernel is built with. Getting this wrong is neither a compile error nor a fault — it is silently wrong data reaching a capability-mediated call. The probe's hand-assembled prologue allocates the shadow store for the same reason, which is why `sub rsp, 0x28` is correctness rather than padding.

**Patching must precede sealing, and that ordering is load-bearing.** The patch writes through the loader's identity view; sealing closes it. A task that could rewrite its own IAT could grant itself capabilities, so the IAT must be read-only to the task and writable only to the loader, in that order. Getting it backwards fails closed rather than silently — after sealing the write faults under `CR0.WP`.

**The trampolines fail the way Windows fails.** `HeapAlloc` returns `NULL`, `WriteFile` returns `FALSE`, `CreateFileA` returns `INVALID_HANDLE_VALUE` — there is no heap, console driver or filesystem behind them, and fabricating a success would be a wild write in the caller's address space. An honest failure runs the caller's own error path; a fake success corrupts it.

**The parser records `FirstThunk`, not the ILT.** Names are read from the Import Lookup Table when one exists (it is the immutable copy), but the slot to patch always comes from the IAT, with an index counting ordinal-only thunks the parser does not record by name. Both mistakes patch a real but *wrong* cell, silently — so both are pinned by a test using a split-table image with a leading ordinal import.

## What remains open

1. **`ExitProcess` does not cleanly terminate a task.** It routes into the capability trap: contained, but a fault rather than a teardown. A task that finishes by *returning* needs scheduler support that does not exist. **This is the clearest next Story in this area.**
2. **The trampolines are capability-mediated stubs, not subsystems.** A workload needing a heap, console or filesystem still cannot make progress. Each is its own Feature-sized piece of work.
3. **The workload is embedded rather than loaded from storage.** A filesystem Story replaces `include_bytes!` with a real load path — and would also let the system image run `blue-sharc.exe` without embedding it.
4. **The probe is hand-assembled, not compiled.** It is evidence about the loader, the IAT boundary and the ABI, not about a real toolchain's code generation. `blue-sharc.exe` remains the evidence for that; the two images answer different questions on purpose.
5. **Tier 0 only.** `LE-09` unchanged, and sharpened by the D04 result.
6. **Nothing — the "boot-timeout flake" recorded in Handover 04 was never real, and that correction matters more than the item did.**

   Handover 04 reported occasional `exit=2` (`HarnessError`) results from `xtask qemu-x86_64` with no `--fixture`, and attributed them to the 15-second boot budget firing under compile load. That diagnosis was wrong, and it was wrong in the way bad diagnoses usually are: it pattern-matched a plausible cause to a symptom without testing it.

   The actual cause was a defect in the *PowerShell regression sweep* used to drive the fixtures, not in `xtask` or the kernel at all. The sweep splatted an argument array into `cargo run -q -p xtask -- @a`; for every fixture that array had two elements and worked, but for the no-fixture case it had exactly one, and PowerShell's single-element splat mangled the ordering so that `-q` was passed *through* to `xtask` instead of being consumed by `cargo`. `xtask` then did precisely what it should: reported `unknown command 'q'` and returned `HarnessError`. The harness was correct, loudly, every time — and it was being asked the wrong question.

   Two things follow. First, `kernel`'s real boot path has been fine throughout; it passes every time it is invoked correctly. Second — and worth recording as a standing caution — this session briefly raised `BOOT_TIMEOUT` from 15s to 60s on the strength of the wrong diagnosis, and that change has been **reverted**. Loosening a timeout is exactly the sort of change that looks harmless, is hard to argue against later, and permanently weakens a real signal (here: how fast a genuine hang is caught) in exchange for suppressing a symptom that had nothing to do with it. The evidence has to support the specific action before the action is taken.

## Verification

367 host tests across the workspace, all passing. Every Tier 0 fixture re-run for regression, all as expected (`broken-boot` and `idt-apic-unrouted` return exit 1 as their documented pass conditions). `check-assurance-spine`, `check-crate-sizes`, `check-performance-catalogue` and `check-image-size` all pass; `cargo fmt --check` and clippy clean.
