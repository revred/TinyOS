# STORY-P1-03-03 — IAT Resolution, the System Image, and the D04 Switch Cost

Status: **Verified (Tier 0 + Host) 2026-07-28**
Feature: [`FEAT-P1-03`](../features/FEAT-P1-03.md)
Introduced in: [`session/hand-2026-07-28/05-story-p1-03-03-iat-system-image-and-d04.md`](../../session/hand-2026-07-28/05-story-p1-03-03-iat-system-image-and-d04.md)

## Description

Closes the three gaps [`REPORT-2026-07-28-01`](../reports/REPORT-2026-07-28-01.md) named as open when `STORY-P1-03-02` landed. They are one Story because they are one problem seen from three sides: the system could contain a real workload but could not *run* one, could not run one from its own boot path, and could not say what running one cost.

**Part A — IAT resolution, and a defect the previous Story's own evidence revealed.** `STORY-P1-03-02` contained `blue-sharc.exe` when its CRT called an unallowlisted Win32 API. Reading that capture closely, the containment was *arithmetically accidental*: nothing had patched the image's Import Address Table, so every thunk still held the RVA of its own `IMAGE_IMPORT_BY_NAME` record, and the indirect call jumped to that RVA taken as an absolute address. It happened to land on non-executable memory. An RVA is an attacker-visible number; an image laid out so a thunk's RVA collided with a mapped executable page would have transferred control there, and nothing in the system was deciding otherwise. This Story makes the outcome a decision: every IAT slot is written exactly once at load time — granted imports get a real, callable `extern "win64"` trampoline; everything else gets `iat::CAPABILITY_TRAP_VIRT`, an address deliberately unmapped in every space. A denied call now faults at one known address that names the cause.

**Part B — the system image.** The shipping binary was `kernel`, whose real boot path discovered ACPI topology, enumerated PCI bus 0, and halted; it had never created a task, because `exec` depends on `kernel` and the reverse link is a cyclic crate dependency. A new top-level `os` binary crate depends on both, so the real boot path now discovers hardware, retires the all-RWX bring-up map for the W^X kernel tree, loads a real PE64 through the real loader, resolves its imports against the capability allowlist, schedules it under its own `CR3` through the production dispatcher, and contains and audits what it does. `G-DX-8`'s image-size gate moves with it.

**Part C — the `D04` switch cost.** `FEAT-P1-03`'s last unmet exit criterion, deferred twice for the same honest reason: nothing in the dispatch path installed a per-task address space, so any number would have been fixture overhead misreported as a scheduling cost. `STORY-P1-03-02` made it measurable; this Story measures it.

## Depends on

`STORY-P1-03-02` (hard — Verified 2026-07-28); `FEAT-P0-05` (the PE64 loader and Win32 shim).

## Acceptance criteria

1. **Every import gets a decision.** `pe::parse` records each import's IAT slot RVA, taken from the descriptor's `FirstThunk` (never the ILT) with a slot index that counts ordinal-only thunks the parser does not otherwise record. `iat::patch_imports` writes every slot exactly once — a granted import's real trampoline address, or `CAPABILITY_TRAP_VIRT` — and fails closed on a slot outside the mapped image or straddling a page boundary. No slot retains its unpatched RVA.
2. **A granted call resolves, executes, and returns.** An allowlisted, policy-granted import called from inside a loaded image's own code, through its patched IAT, reaches a real `extern "win64"` trampoline and returns the value real Windows returns. The Microsoft x64 ABI is pinned at that boundary rather than inherited from the kernel's System V default.
3. **A denied call is contained at a named address.** An import the allowlist rejects, or the policy denies, faults with `CR2` equal to `CAPABILITY_TRAP_VIRT` — contained by the unmodified `kernel::fault` policy, and diagnosable as a refused capability rather than as a wild jump.
4. **Patching precedes sealing, and the IAT is read-only to the task.** The patch is applied through the loader's identity view before `seal_kernel_alias` closes it, so the task can never rewrite its own IAT to grant itself capabilities.
5. **The real boot path runs a real task.** The `os` binary performs the same ACPI/PCI discovery with the same success gates, brings up W^X memory protection, loads and schedules the embedded workload, and reports what it returned — all under `G-DX-8`'s image-size ceiling, which now measures `os` rather than `kernel`.
6. **The `D04` delta is measured.** Two dispatch rounds through the production `run_once_in_space` differing only in whether the selected task's address space is already loaded, reported through the standard `TINYOS-MEAS/1` envelope, with the difference between them stated as what isolation costs per scheduling decision.

## Tests

[`TEST-P1-03-03-A`](../tests/TEST-P1-03-03-A.md) — host tests for the parser's IAT extraction, the patcher's three outcomes and two failure modes, the trampoline table, and the generated probe image's every offset; plus Tier 0 runs of the `os` system image, the `first-task` fixture (re-proving containment now lands on the trap), and the `dispatch` measurement.

## Goals verified

G-PC-2, G-PC-3 (the capability boundary reaches the one place a loaded image can actually exercise it), G-SEC-2, G-SEC-14, G-DX-8 (now measured against the real system image).
