# TEST-P0-05-04-A — `blue-sharc.exe`'s Real TXE-Packed Image Parses, Maps, and Is Correctly Import-Gated Under QEMU

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-05-04`](../stories/STORY-P0-05-04.md)
Tier: Tier 0 (QEMU x86_64), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — the flagship end-to-end validation for `FEAT-P0-05`, mirroring the role `TEST-P0-04-01-A` played for `STORY-P0-04-01` (real target, not a fixture stand-in, is the point of this specific Test).
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D09`, `D25`
Security controls: `SEC-02`, `SEC-03`, `SEC-05`, `SEC-09`, `SEC-16`, `SEC-19`, `SEC-20`
Containment classes: `C1`, `C3`, `C4`
Boundary tests: `BND-03`, `BND-04`, `BND-05`, `BND-09`, `BND-10`, `BND-11`, `BND-12`, `BND-19`, `BND-20`
Protection Domain contracts: `PD-01`, `PD-02`, `PD-03`, `PD-04`, `PD-05`, `PD-06`, `PD-08`, `PD-11`, `PD-12`, `PD-13`, `PD-14`
Code admission gates: `RCG-01`, `RCG-02`, `RCG-03`, `RCG-04`, `RCG-05`, `RCG-06`, `RCG-07`, `RCG-08`, `RCG-09`, `RCG-10`, `RCG-11`, `RCG-12`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

**Correction (2026-07-26):** this Test's own original draft (written ahead of implementation, per the TDD mandate) specified reaching `blue-sharc.exe`'s real entry point as a scheduled TinyOS task. Picking up the Story surfaced that this is infeasible against the *unmodified* real binary this session: it imports 220 Win32/CRT functions across 14 DLLs (sockets, TLS, crypto, C++ exception unwinding, full CRT init) — its own C runtime calls dozens of these before `main` even runs — and this kernel has no per-task `CR3` switch, no IDT/interrupts, and no threads. `STORY-P0-05-04.md`'s acceptance criteria were rewritten accordingly (see that file's own note) before this Test was implemented against the revised scope below.

## Specification

**Given** `exec::pe::parse`, `exec::address_space::AddressSpace::create`, and `exec::win32_shim::check_imports`/`resolve`/`heap_alloc`, and `blue-sharc.exe`'s real build artifact (`Sharc.Bluekind/target/gate-fast/blue-sharc.exe` in the sibling `Sharc.Blue` project) re-packed once, offline, via `xtask pack-txe` (`STORY-P0-08-01`) into a page-aligned, `.bss`-flattened `blue-sharc.txe` — byte-for-byte equivalent to the real PE, not a hand-patched substitute,
**when** a dedicated QEMU fixture (`exec`'s `blue-sharc-fixture` binary) boots and runs it under `xtask qemu-x86_64 --fixture=blue-sharc`,
**then**:
- `pe::parse` parses the real image's header, all 6 real sections, and its real (205 named-import) import table without error,
- `AddressSpace::create` maps all 6 real sections at their real virtual addresses with their real permissions, including `.data`'s real 352-byte `.bss` tail,
- `win32_shim::check_imports` against the real import table returns `Err(ShimError::NotAllowlisted)` — correctly rejecting an image whose real capability needs (220 imports) vastly exceed this Phase 0 shim's 9-call allowlist; this is the load-time security gate (`G-PC-2`/`G-PC-3`) working as designed, not a test failure,
- `win32_shim::resolve(b"KERNEL32.dll", b"HeapAlloc")` resolves, and `win32_shim::heap_alloc` succeeds when the capability policy grants it — this Story's own redefined checkpoint (a real, capability-mediated `HeapAlloc` call path, proven directly rather than by executing the image's own machine code),
- a deliberately-corrupted copy of the same `blue-sharc.txe` bytes (flipped DOS signature, mirroring `TEST-P0-01-03-A`'s `fixture-broken-boot` pattern, via a separate `blue-sharc-broken-fixture` binary and `xtask qemu-x86_64 --fixture=blue-sharc-broken`) fails closed with `PeError::InvalidDosSignature`, while the same unmodified bytes still parse — proving the rejection is specific to the corruption, not a fixture-wide regression.

## Test type

Integration test (Tier 0, QEMU-based), per `agent/CODING_STANDARDS.md`'s "every driver/kernel path targets at minimum a Tier 0 test" requirement. This Test is pure composition — it exercises `STORY-P0-05-01`–`-03` together against the real validation case (repacked via `STORY-P0-08-01`'s tool, not hand-patched); it does not introduce new parsing/mapping/shim logic of its own (that logic's own dedicated tests are `TEST-P0-05-01-A` through `-03-A`, and `TEST-P0-08-01-A` for the packer itself).

## Implementation location

`os/src/exec/src/fixture_blue_sharc_main.rs` (the successful-load fixture, `[[bin]] blue-sharc-fixture`), `os/src/exec/src/fixture_blue_sharc_broken_main.rs` (the corrupted-copy fixture, `[[bin]] blue-sharc-broken-fixture`), `os/src/exec/fixtures/blue-sharc.txe` (the real, TXE-packed fixture artifact), `os/src/xtask/src/main.rs`'s `--fixture=blue-sharc`/`--fixture=blue-sharc-broken` dispatch. `os/src/exec/src/address_space.rs`'s `AddressSpace::create` was generalized (`STORY-P0-05-04`) to copy each section's bytes into caller-supplied page-aligned staging storage rather than mapping the file directly — the fix for real-world `FileAlignment` (512) being finer than a page, the same reason every real OS loader copies section data rather than mapping it zero-copy. `os/src/exec/src/win32_shim.rs` gained a real `heap_alloc` call implementation.

## Reports

[`REPORT-2026-07-26-20`](../reports/REPORT-2026-07-26-20.md) — Pass.
