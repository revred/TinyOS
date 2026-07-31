# TEST-P1-03-03-A — Import Resolution, the System Image, and the D04 Switch Cost

Status: **Verified (Tier 0 + Host) — written alongside implementation, per the process note below**
Story: [`STORY-P1-03-03`](../stories/STORY-P1-03-03.md)
Tier: Host unit tests (parser, patcher, trampolines, generated image) **plus** Tier 0 QEMU runs of the system image and the dispatch measurement, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D04`, `D05`, `D08`
Security controls: `SEC-03`, `SEC-14`, `SEC-19`
Containment classes: `C0`, `C1`, `C3`
Boundary tests: `BND-04`, `BND-05`, `BND-20`
Protection Domain contracts: `PD-01`, `PD-04`, `PD-13`
Code admission gates: `RCG-10`, `RCG-11`
Assurance state: `baseline-debt`

## What this test is for

`STORY-P1-03-02` proved a real workload could be *contained*. This one proves it can be *run* — and fixes the reason the previous containment was weaker than it looked.

## Specification

### 1. The containment defect this Story exists to fix

**Given** `REPORT-2026-07-28-01`'s capture, in which `blue-sharc.exe` faulted at `rip == cr2 == 0x7b9d9e`,
**then** the reading is that nothing had patched the image's Import Address Table, so every thunk still held the RVA of its own `IMAGE_IMPORT_BY_NAME` record and the CRT's indirect call jumped to that RVA taken as an absolute address. It landed on non-executable memory. The outcome was correct; the *mechanism* was arithmetic. An RVA is an attacker-visible number, and an image laid out so a thunk's RVA collided with a mapped executable page would have transferred control there instead, with nothing in the system deciding otherwise.

This clause is a specification, not commentary: the test for it is that after this Story the same image faults at `CAPABILITY_TRAP_VIRT` rather than at an image-derived address.

### 2. The parser records the slot a loader must patch

**Given** an image with a separate ILT and IAT — the layout every real linker emits — and an ordinal-only import occupying a slot the parser does not record by name,
**then** `ImportEntry::iat_slot_rva` is taken from the descriptor's `FirstThunk` (never `OriginalFirstThunk`) and its index counts *every* thunk in the array. Both mistakes are silent — each patches a real but wrong cell — so both are pinned: the test asserts the recorded slot is `FirstThunk + 8` for the second thunk, and explicitly asserts it is *not* the corresponding ILT address.

### 3. Every import gets exactly one decision

**Given** `iat::patch_imports` over a mixed import table,
**then** an allowlisted-and-granted import's slot holds its real trampoline address; a non-allowlisted import's holds `CAPABILITY_TRAP_VIRT`; an allowlisted-but-policy-denied import's holds `CAPABILITY_TRAP_VIRT` too, counted separately so the two refusal reasons stay distinguishable in the audit. `PatchSummary::total()` equals the import count — there is no fourth outcome and no unwritten slot. A slot seeded with a realistic unpatched RVA does not retain it.

**And** patching fails closed — `SlotOutOfBounds` for a slot outside the mapped image, `SlotStraddlesPage` for one crossing a frame boundary — rather than writing a partial function pointer.

### 4. The trampolines are real, distinct, and ABI-correct

**Given** the nine allowlisted APIs,
**then** each has a non-null trampoline address, distinct from every other and from the trap (a duplicated entry would silently route one capability's calls into another's implementation). Each is `extern "win64"`: the callers are MSVC-compiled code passing arguments in `RCX`/`RDX`/`R8`/`R9` with a 32-byte shadow store, not the System V convention the rest of this kernel uses. Getting that wrong is neither a compile error nor a fault — it is silently wrong data reaching a capability-mediated call — so the ABI is pinned at the boundary.

**And** a granted trampoline is callable *through its patched slot*: the test reads the function pointer back out of the IAT cell and indirects through it exactly as the image's own code would, asserting `GetCurrentProcess` returns the `(HANDLE)-1` pseudo-handle.

### 5. The generated probe image is a real PE, checked offset by offset

**Given** `xtask make-probe-pe`'s output,
**then** it is four pages with a real DOS header, PE32+ optional header, three sections (`.text` RX, `.rdata` R, `.data` RW — none writable *and* executable), a real import directory with genuinely separate ILT and IAT, and hand-assembled x86-64 code. Every RIP-relative displacement is re-derived from the encoded bytes back to its intended target, so a hand-assembly slip is a failing host test rather than an unexplained triple fault under QEMU. The result slot starts zeroed, so "the probe stored the right value" can never be satisfied by a value the image shipped with. The imported names are asserted to match `win32_shim::resolve`'s spellings exactly — a typo would silently become a trapped import and the probe would prove the opposite of its claim.

### 6. Tier 0: the system image boots, loads, and runs a real task

**Given** `cargo run -p xtask -- qemu-x86_64 --fixture=os`,
**then** the real boot path discovers hardware, retires the all-RWX bring-up map, loads the embedded probe, resolves its two imports (both granted under a policy that grants exactly those two and nothing else), seals, schedules it under its own `CR3` through `run_once_in_space`, and the workload's own code calls `GetCurrentProcess` through its patched IAT and stores what it returned. The supervisor reads that slot back.

**Observed** (`ok=true`, `isa-debug-exit` success):

```text
tinyos: 1 CPU(s), 6 PCI device(s)
tinyos: W^X memory protection active
tinyos: loaded image — 2 import(s), 2 granted, 0 trapped, cr3=0x13f000
tinyos: task 0 terminated — vector=14 rip=0xdead0000 cr2=0xdead0000 (refused capability)
tinyos: workload returned 0xffffffffffffffff (GetCurrentProcess ok=true), exited via trap 0xdead0000, task_finished=true, spoors=5
tinyos: boot complete, ok=true
```

`0xffffffffffffffff` is the evidence that matters: a real loaded image's own instruction stream, executing under its own address space, called a Win32 API through a loader-patched IAT and got the correct answer back. The system image is 74,568 bytes against `G-DX-8`'s 8MiB ceiling, which now measures `os` rather than `kernel`.

### 7. Tier 0: the same real artifact now traps at a named address

**Given** `--fixture=first-task`, unchanged except that it now patches imports before sealing,
**then** `blue-sharc.exe`'s 205 imports resolve to 7 granted and 198 trapped, and its CRT's first reach beyond its grants faults at `CAPABILITY_TRAP_VIRT` rather than at an image-derived RVA:

```text
first-task: IAT resolved — 7 granted, 198 trapped (198 not allowlisted, 0 denied), trap=0xdead0000
first-task contained task 0 vector=14 rip=0xdead0000 cr2=0xdead0000
TOS64-RESULT/1 fixture=first-task ok=true
```

Compare with `TEST-P1-03-02-A` clause 5's `rip == cr2 == 0x7b9d9e`. Same containment, now for a stated reason — and `cr2` alone is enough to diagnose it.

### 8. Tier 0: the `D04` same-space vs cross-space delta

**Given** two dispatch rounds through the production `run_once_in_space` differing only in whether the selected task's address space is already loaded,
**then** the difference between them is what address-space isolation costs per scheduling decision. Release profile, median of three runs:

| Metric | p50 (cycles) |
|---|---|
| `D04/dispatch_round_same_space` | 276 |
| `D04/dispatch_round_cross_space` | 7,452 |

The delta is roughly **7,200 cycles**, a ~27x multiplier on the dispatch round. **That figure is an emulator artifact and must not be quoted as a hardware cost.** Under TCG a `mov cr3` forces QEMU to flush its own software TLB and re-resolve translations, which is enormously more expensive than the real instruction's few-hundred-cycle pipeline and TLB effect. The number is therefore an *upper bound* that establishes the measurement path works and the two arms are distinguishable — and it is the most concrete argument this project has yet produced for the hardware tier (`LE-09`), because it is a case where Tier 0 cannot even give the right order of magnitude.

The metrics are reported and deliberately **not** gated: thresholding them would gate the emulator's TLB model rather than this kernel's dispatch path.

### 9. What this test explicitly does **not** establish

- **No hardware tier.** Tier 0 QEMU only; `LE-09` open, and clause 8 sharpens it.
- **The probe is hand-assembled, not compiled.** It exercises the loader, the IAT boundary and the ABI; it is not evidence about a real toolchain's code generation. `blue-sharc.exe` remains the evidence for that, and the two images answer different questions on purpose.
- **`ExitProcess` does not cleanly terminate a task.** It routes into the capability trap, which is contained but is not a process-teardown path. A task that finishes by *returning* rather than by faulting needs scheduler support that does not exist yet.
- **The trampolines are capability-mediated stubs, not subsystems.** `HeapAlloc` returns `NULL`, `WriteFile` returns `FALSE`, `CreateFileA` returns `INVALID_HANDLE_VALUE` — the shapes real Windows returns when the operation cannot be satisfied, because there is no heap, console driver or filesystem behind them. Returning a fabricated success would be a wild write in the caller's address space.
- **The workload is embedded, not loaded from storage.** There is no filesystem; a storage Story replaces `include_bytes!` with a real load path.
- **No preemption** (`FEAT-P1-04`).

## Process note: how strictly TDD was followed here

As with this Feature's earlier Tests, the document did not precede the code. Each pure seam (parser field, patcher, trampoline table, image generator) was written with its host tests alongside; the Tier 0 behavior was brought up against real QEMU captures. Clause 1's defect was found by *reading the previous Story's own passing evidence* rather than by a failing test, which is worth recording as its own lesson: a green capture is not the same as a understood one.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/exec/src/pe.rs`, `os/src/exec/src/iat.rs`, `os/src/xtask/src/probe_pe.rs`) plus Tier 0 QEMU runs (`qemu-x86_64 --fixture=os`, `--fixture=first-task`, and `measure --fixture=dispatch`).

## Implementation location

- `os/src/exec/src/pe.rs` — `ImportEntry::iat_slot_rva` and its extraction.
- `os/src/exec/src/iat.rs` — `patch_imports`, `PatchSummary`, `CAPABILITY_TRAP_VIRT`, `trampolines`.
- `os/src/os/` — the system image crate (`main.rs`, `Cargo.toml`).
- `os/src/xtask/src/probe_pe.rs` — the probe image generator; `os/src/exec/fixtures/capability-probe.txe` its committed output.
- `os/src/exec/src/fixture_dispatch_measure_main.rs` — the `D04` measurement fixture.
- `os/src/exec/src/fixture_first_task_main.rs` — now patches imports before sealing.
- `os/src/xtask/src/main.rs` — `MeasurableTarget`, `make-probe-pe`, the `os`/`dispatch-measure` fixture mappings, and `check-image-size` retargeted at the system image.
- `.github/workflows/ci.yml` — the two new CI steps.

## Reports

- [`REPORT-2026-07-28-02`](../reports/REPORT-2026-07-28-02.md) — the captures, the accidental-containment finding, the D04 numbers, and what remains open.
