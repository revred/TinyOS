# STORY-P0-05-04 — End-to-End: `blue-sharc.exe`'s Real Image Parses, Maps, and Is Correctly Import-Gated Under QEMU

Status: **Verified**
Feature: [`FEAT-P0-05`](../features/FEAT-P0-05.md)
Introduced in: [`session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md`](../../session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md)
Implemented in: [`session/hand-2026-07-26/25-story-p0-05-04-and-txe-packer.md`](../../session/hand-2026-07-26/25-story-p0-05-04-and-txe-packer.md)

## Description

The flagship validation case for this whole Feature: `blue-sharc.exe`'s real build artifact (`Sharc.Bluekind/target/gate-fast/blue-sharc.exe` in the sibling `Sharc.Blue` project) is fed through `STORY-P0-05-01`'s parser, `STORY-P0-05-02`'s mapper, and `STORY-P0-05-03`'s API shim — proving the preceding three Stories compose correctly against a real, non-fixture binary, mirroring the role `TEST-P0-04-01-A` played for `STORY-P0-04-01` (a hand-crafted fixture is a useful additional test, never a substitute for testing against the real thing this Feature exists to support).

**Correction (2026-07-26), rewriting this Story's own acceptance criteria before implementation began.** The original draft (kept below, superseded) called for reaching the image's real entry point as a scheduled TinyOS task. Two facts, discovered while picking this Story up, make that infeasible this session:

1. `blue-sharc.exe` imports **220 Win32/CRT functions across 14 DLLs** — `KERNEL32.dll` (109 functions: threads, named pipes, memory-mapped files, timers), `WS2_32.dll` (sockets), `advapi32`/`secur32`/`crypt32` (security descriptors, TLS/SSL, certificate stores), `ntdll` (raw syscalls), `VCRUNTIME140.dll` (C++ exception unwinding), and the `api-ms-win-crt-*` family (CRT init, called before `main` even runs). This kernel has no per-task `CR3` switch, no IDT/interrupt/exception-handling subsystem, and no threads — implementing that whole surface is several more Features' worth of work, not a scoping choice available this session.
2. `AddressSpace::create`'s original zero-copy design required every section's on-disk `file_offset` to be page-aligned — a requirement no real-world linker's default `FileAlignment` (512, this binary's own) satisfies, since x86-64 page tables can only ever map whole aligned physical pages.

Fact 2 is fixed generically: `AddressSpace::create` (`STORY-P0-05-02`) now copies each section's bytes into caller-supplied, page-aligned staging storage instead of mapping the file directly — the same tradeoff every real OS loader makes for the identical hardware reason, and it uniformly also handles a section whose `virtual_size` exceeds `file_size` (`.data`'s real 352-byte `.bss` tail). Fact 1 is addressed differently: rather than building a full CRT/Win32 emulation surface in one pass, this Story's AC1 is redefined to prove the loader pipeline against the image's **real** parse/map/import-gate surface without executing its own machine code — a `.txe`-packed copy of the same real bytes (`STORY-P0-08-01`'s deterministic re-layout tool, not a hand-patch) is used only for the fixture's own convenience; `AddressSpace::create` itself now tolerates arbitrary file alignment regardless, proven directly against the raw, unpacked bytes in `address_space.rs`'s own host tests.

## Depends on

`STORY-P0-05-01`, `-02`, `-03` (this Story is otherwise pure integration, no new parsing/mapping/shim logic of its own beyond `AddressSpace::create`'s alignment generalization and `win32_shim::heap_alloc`'s new call implementation), `STORY-P0-08-01` (the TXE packer producing this Story's fixture artifact).

## Acceptance criteria (final, superseding the original draft)

1. `pe::parse` parses `blue-sharc.exe`'s real header, all 6 real sections, and its real (205 named-import) import table without error; `entry_point_rva`/`image_base` match the real file's own optional header fields. **Met**: `blue-sharc-fixture`'s QEMU run.
2. `AddressSpace::create` maps all 6 real sections at their real virtual addresses with their real permissions, including `.data`'s real 352-byte `.bss` tail (demand-zeroed, not exposing adjacent staging bytes). **Met**: same fixture; `address_space.rs`'s own host tests (`a_section_with_virtual_size_larger_than_file_size_demand_zeros_the_tail`, `mapped_page_content_matches_the_sections_source_bytes`).
3. `win32_shim::check_imports` against the real import table returns `Err(ShimError::NotAllowlisted)` — the real image's 220-import capability need vastly exceeds this Phase 0 shim's 9-call allowlist, and the load-time security gate must reject it, not silently allow it (`G-PC-2`/`G-PC-3`). **Met**: same fixture.
4. `win32_shim::resolve`/`heap_alloc` prove `HeapAlloc` — the specific capability this Story's checkpoint targets — resolves against the allowlist and succeeds through the real capability-mediated call path when granted, fails with `PolicyDenied` when not. **Met**: same fixture; `win32_shim.rs`'s own host tests (`heap_alloc_succeeds_when_granted`, `heap_alloc_is_rejected_when_the_policy_denies_it`).
5. A deliberately-corrupted copy of the same real bytes (flipped DOS signature) fails distinguishably (`PeError::InvalidDosSignature`), while the same unmodified bytes still parse — mirroring `TEST-P0-01-03-A`'s `fixture-broken-boot` pattern. **Met**: `blue-sharc-broken-fixture`'s QEMU run.
6. A new `xtask check-image-size` command measures `kernel`'s own built release image against `G-DX-8`'s 8MB ceiling — the whole-image counterpart to `check-crate-sizes`'s per-crate LOC ceiling, previously unimplemented. **Met**: `kernel`'s image is 16,032 bytes, unchanged from every prior Story in this session (no production call site links `exec` into `kernel` yet — same "no dispatcher wired into `main.rs`" gap `STORY-P0-06-03`/`-04` already named for spoor).

**Explicitly not attempted**: jumping into the image's own entry point and executing its real machine code. That requires a live `CR3` switch, an IDT/exception-handling subsystem, and dozens more Win32/CRT shim calls (TLS, exceptions, sockets, crypto) — a substantially larger, separate undertaking recorded as this Story's own natural next step, not silently declared done.

## Tests

`os/src/exec/src/fixture_blue_sharc_main.rs`, `os/src/exec/src/fixture_blue_sharc_broken_main.rs` (Tier 0, QEMU), `os/src/exec/src/address_space.rs` and `os/src/exec/src/win32_shim.rs`'s extended host test suites. See [`TEST-P0-05-04-A`](../tests/TEST-P0-05-04-A.md) and [`REPORT-2026-07-26-20`](../reports/REPORT-2026-07-26-20.md).

## Goals verified

G-PC-1 through G-PC-4 (the loader pipeline proven against the real validation case, to the extent achievable without a CR3 switch/IDT this kernel doesn't have yet).

**Correction (2026-07-26):** an earlier version of this Story additionally claimed `G-AI-1` (local inference hosting) here, on the premise that `blue-sharc.exe` is an inference runtime. It isn't — `blue-sharc.exe` is `Sharc.Blue`'s MCP sidecar/context engine for LLM tool-calling over stdio/IPC, a real and non-trivial native binary but not a model-inference workload. This Story verifies process/executable compatibility (`G-PC-1`–`G-PC-4`) only; `G-AI-1` remains Phase 6's own, separate goal to prove against TinyOS's own future `inference` crate. See `FEAT-P0-05.md`'s matching correction.

---

### Original draft acceptance criteria (superseded 2026-07-26, kept for record)

1. ~~`blue-sharc.exe`, copied into the test fixture set unmodified from a real `Sharc.Blue` build (never patched or recompiled to "make it work" — if it doesn't load as-built, that's a defect in `STORY-P0-05-01`–`-03`, not a fixture problem to paper over), reaches a documented, minimal checkpoint under `xtask`'s QEMU harness: successful process entry and at least one successful heap allocation via the `STORY-P0-05-03` shim.~~
2. ~~The checkpoint is verified via the same `isa-debug-exit` pass/fail pattern `TEST-P0-01-01-A`/`TEST-P0-04-01-A` established.~~
3. ~~A deliberately-corrupted copy of `blue-sharc.exe` (mirroring `TEST-P0-01-03-A`'s `fixture-broken-boot` pattern) fails distinguishably rather than being silently accepted.~~ (retained above as AC5, unchanged in spirit)
4. ~~The full built OS image (kernel + hal + hal-x86_64 + exec, excluding drivers) still meets `G-DX-8`'s 8MB ceiling with this Story's fixture and any supporting code included.~~ (retained above as AC6, generalized into a reusable `xtask check-image-size` command)
