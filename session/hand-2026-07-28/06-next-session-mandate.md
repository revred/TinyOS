# Handover 06 — Next-Session Mandate

Written at the close of 2026-07-28, after `FEAT-P1-03` completed. This is the start-here document for the next session: what state the project is actually in, what to do first, and what not to be misled by.

## Where the project stands

**`EPIC-P1` is half done: `FEAT-P1-01`, `-02` and `-03` are complete; `-04`, `-05`, `-06` are Specified and untouched.**

The material change this session is that TinyOS stopped being a collection of individually-proven mechanisms and became a system that runs something. The shipping image is now `os` (a new top-level binary crate), and its real boot path discovers hardware, retires the boot-time all-RWX identity map for a W^X kernel tree, loads a real PE64 through the real loader, resolves its imports against the capability allowlist, and schedules it as a task under its own `CR3` — contained and audited. Both directions of the capability contract hold against real images: a granted call resolves and returns correctly from inside the loaded image's own code, and a refused one faults at a named trap address.

Read, in this order, before starting anything:

1. [`04-story-p1-03-02-wx-seal-and-first-real-task.md`](04-story-p1-03-02-wx-seal-and-first-real-task.md) — W^X, sealing, teardown, the first real task.
2. [`05-story-p1-03-03-iat-system-image-and-d04.md`](05-story-p1-03-03-iat-system-image-and-d04.md) — IAT resolution, the `os` image, the `D04` number, and the retracted misdiagnosis at the end.
3. [`03-story-p1-03-02-hardening-review.md`](03-story-p1-03-02-hardening-review.md) — the nine-defect pre-implementation review, as a worked example of the review standard this project now expects.

## Start here: `STORY-P1-04-01` (timer-driven preemption)

`FEAT-P1-04` is the next Feature in the Epic's own ordering, and its dependency (`FEAT-P1-02`, real fault handling) has been met since 27 July. Start with `STORY-P1-04-01` — timer-driven preemption — following the established process: **Test document first**, host tests before fixtures, one piece verified before the next.

Three things make this Story harder than it looks, and all three are already documented rather than discovered:

- **`LE-14` becomes live the moment preemption does.** `kernel::context::switch` saves callee-saved integer registers and flags — no SSE/x87 state. That is sound today only because every switch is *cooperative*, so a task is never suspended mid-computation. A timer interrupt can suspend anywhere, including between two halves of an SSE operation, and `boot.rs` deliberately enables SSE (ADR 0003). **Preemption without extended-state save/restore is silent data corruption, not a fault.** Treat this as part of `-04-01`'s scope or split it out explicitly; do not let it be implied.
- **The dispatcher does not restore its own address space.** `run_once_in_space` installs the *incoming* task's `CR3` and does not put the dispatcher's back on return — correct today, because the shared kernel directories keep the supervisor mapped under any task's tree. Under preemption, re-entering the dispatcher from an interrupt in an arbitrary task's space needs that property to hold for the *interrupt* path too. It does, but it is now load-bearing rather than incidental, and it should be stated in the Story rather than relied on quietly.
- **`LE-01`/`LE-02` are this Feature's to close.** Priority inheritance has never been proven under real preemption, and the WCET budget has no timer or watchdog behind it.

## Standing constraints — do not relax these

- **TDD.** Test document when a Story starts, never pre-written; Red before Green where the seam allows it, and where it does not (a Tier 0 fixture debugged against real hardware behaviour), say so plainly in the Test document's process note, as the last three have.
- **Tier 0 is not hardware evidence.** `LE-09` remains open. Every timing claim carries release-blocking hardware debt. This session sharpened the argument: the `D04` cross-space figure (~7,452 cycles vs ~276 same-space) is dominated by TCG's own TLB-flush emulation, so it is a case where Tier 0 cannot give even the right order of magnitude. It is reported and deliberately **not** gated — thresholding it would gate the emulator.
- **Never call TinyOS a hobby OS.** It targets data-centre, local-AI, UAV, medical, edge and consumer deployment.
- **No assurance state may be claimed beyond its evidence.** Every Verified Story is still `baseline-debt`. The first `baseline-debt → verified` conversion is `EPIC-P1`'s explicit charge and has not happened.

## Two lessons this session earned, worth carrying

**A passing capture is not an understood capture.** `STORY-P1-03-02` reported containment of a real workload and was correct. Re-reading *how* it was contained showed the containment was arithmetically accidental — the loaded image's IAT was never patched, so a refused capability jumped to an unrelocated RVA that merely happened to land on non-executable memory. That reading produced `STORY-P1-03-03`. Both of this session's substantial findings came from re-reading green evidence rather than from a failing test.

**Check that the evidence supports the specific action.** This session diagnosed an intermittent `exit=2` as a boot-timeout flake and raised `xtask`'s boot budget from 15s to 60s. The diagnosis was wrong: the cause was a defect in the PowerShell regression sweep (a single-element argument splat leaked `-q` past `cargo` into `xtask`), which only ever affected the no-`--fixture` invocation. The timeout change was reverted and the claim retracted in Handover 04. Loosening a timeout looks harmless, is hard to argue against later, and permanently weakens how fast a genuine hang is caught — in exchange for suppressing a symptom that had nothing to do with it.

## Open items, by owner

**Owned by `FEAT-P1-04`:** `LE-01` (priority-inheritance behavioural proof), `LE-02` (WCET has no timer/watchdog), `LE-14` (extended-state save/restore).

**Owned by a future Story, unscheduled:**

- **Clean task termination.** `ExitProcess` currently routes into the capability trap — contained, but a fault rather than a teardown. A task that finishes by *returning* needs scheduler support that does not exist. This is the clearest next Story in the `exec` area and is small enough to slot in beside `FEAT-P1-04` if preemption stalls.
- **Real Win32 subsystems.** The trampolines are capability-mediated stubs: `HeapAlloc` returns `NULL`, `WriteFile` returns `FALSE`, `CreateFileA` returns `INVALID_HANDLE_VALUE`. They fail the way real Windows fails rather than fabricating success, but a workload needing a heap, console or filesystem still cannot make progress. Each is Feature-sized.
- **Loading from storage.** The system image embeds its workload via `include_bytes!` because there is no filesystem. A storage Story replaces that and would also let the system image run `blue-sharc.exe` without embedding 8.3MiB into a kernel.
- **A compiled probe.** The capability probe is hand-assembled, so it is evidence about the loader, the IAT boundary and the ABI — not about a real toolchain's code generation. `blue-sharc.exe` remains the evidence for that. If a Windows cross-toolchain ever becomes available in CI, a compiled probe would close the gap.
- **The `D04` baseline is ungated.** `check-timing-regression` only drives the `measure` fixture; wiring the `dispatch` fixture's metrics into a committed baseline needs gate work, and should only be done for a tier where the number means something.

**Open, unowned:** `LE-08`, `LE-10`, `LE-12`, `LE-18`, `LE-19` part (b). **Open, owned:** `LE-03`, `LE-11`, `LE-15`, `LE-16`. **Closed this session:** `LE-05`.

## How to verify you have a good starting state

```
cd os
cargo test --workspace                                  # 367 passing
cargo fmt --all -- --check
cargo clippy --workspace --lib --tests -- -D warnings
cargo run -p xtask --quiet -- check-assurance-spine     # 14 Features / 37 Stories / 32 Tests / 39 Reports
cargo run -p xtask --quiet -- check-image-size          # os, 74,568 bytes
cargo run -p xtask --quiet -- qemu-x86_64 --fixture=os  # the system image boots and runs its workload
```

Every Tier 0 fixture should pass, with exactly two exceptions that are *supposed* to return exit 1: `broken-boot` and `idt-apic-unrouted`, each of whose documented pass condition is a distinguishable failure. When sweeping fixtures from PowerShell, pass arguments literally rather than splatting an array — see the second lesson above for what a splat cost this session.
