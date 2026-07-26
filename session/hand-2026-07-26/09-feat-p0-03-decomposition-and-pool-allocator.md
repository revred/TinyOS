# Handover 09 — FEAT-P0-02/03/04 Decomposition and the Pool Allocator Core

Follows: [`08-cover-note-mvp-continuation-and-ns-file-access.md`](08-cover-note-mvp-continuation-and-ns-file-access.md), which set this session's mandate: continue `EPIC-P0` toward completion, and carry the ns-file-access direction into `FEAT-P0-03`'s scope.

## What this session did

1. **Decomposed all three remaining `EPIC-P0` Features into Stories**, per Cover Note 08's explicit deferral:
   - `FEAT-P0-02` (scheduler) → `STORY-P0-02-01` through `-04` (task creation/priority, context switch, priority inheritance, WCET enforcement).
   - `FEAT-P0-03` (memory allocator) → `STORY-P0-03-01` through `-03` (pool allocator type, compile-time pool-size configuration, fail-closed exhaustion path).
   - `FEAT-P0-04` (HAL/ACPI) → `STORY-P0-04-01` through `-03` (ACPI table parsing, APIC bring-up, bus enumeration).
   - Updated `EPIC-P0.md`, all three Feature files, `traceability-matrix.md`, and `goals/index.html` to reflect the new Stories.
2. **Implemented and Verified `STORY-P0-03-01`/`-03`** — the `Pool<T, N>` bounded-capacity allocator, in a new `os/src/kernel/src/mem.rs`, test-first per `TEST-P0-03-01-A`/`TEST-P0-03-03-A` (written before the implementation). 6 host unit tests pass (`cargo test -p kernel --lib`), `fmt`/`clippy -D warnings` clean, crate-size check confirms `kernel` at 308 lines (still trivial against the 20,000-line ceiling). Filed `REPORT-2026-07-26-04`.
3. **Restructured the `kernel` crate to carry a lib alongside its existing bin** — `src/lib.rs` (`#![cfg_attr(not(test), no_std)]`) plus a `[lib]` target in `Cargo.toml`, so host-testable logic (the allocator, and future non-boot-dependent kernel code) doesn't need a QEMU round-trip to verify. Confirmed the existing `no_std`/`no_main` binary still builds cleanly against the real `x86_64-tinyos.json` target after this change — `FEAT-P0-01`'s walking skeleton is not regressed.

## What this session deliberately did not do

- **Did not implement `FEAT-P0-02` or `FEAT-P0-04`'s Stories.** Both need Tier 0 (QEMU) verification per their own acceptance criteria — the environment this session ran in has `cargo`/`rustc` but no `qemu-system-x86_64` binary, so any implementation here would be untested against the standard this project holds every driver/kernel path to (`agent/CODING_STANDARDS.md`'s TDD mandate: "every driver/kernel path targets at minimum a Tier 0 test"). Writing scheduler assembly or ACPI parsing without the ability to actually boot and check it would violate the same TDD discipline this session's Story files just wrote down as a requirement. They're decomposed and ready — a session with QEMU available should pick them up next.
- **Did not attempt the parallel-subagent approach Cover Note 08 flagged as worth trying.** With only `FEAT-P0-03` actually implementable in this environment, there was nothing to parallelize against this session — worth trying once a QEMU-capable environment is doing `FEAT-P0-02`/`FEAT-P0-03`(remainder)/`FEAT-P0-04` together, since those three are still genuinely independent of each other per Cover Note 08's reasoning.
- **Did not start Phase 6 mmap design work.** `STORY-P0-03-02` (compile-time pool-size configuration) — the piece closest to a real virtual-memory allocator — is still Planned; the ns-file-access direction from Cover Note 08 is restated in `FEAT-P0-03.md`'s new "Note on scope beyond Phase 0" section so it isn't lost, but no Phase 6 code or design doc changes happened here.

## Key decisions worth restating

- **`Pool::alloc`/`free` take `&mut self`, not `&self`.** No concurrent-access story exists yet (no scheduler is implemented), so the allocator doesn't invent a locking/interior-mutability scheme speculatively — that's explicit in `STORY-P0-03-01`'s acceptance criteria now, not just an implementation choice a reader would have to reverse-engineer. Revisit when `FEAT-P0-02` gives the kernel its first real concurrent caller.
- **`Pool<T, N>` implements `Drop`**, freeing any still-occupied slots' values on scope-exit, even though no Story text explicitly demanded it — treated as implied by "no heap allocation... no unbounded... " correctness generally (an allocator that leaks on drop is a defect, not a missing nice-to-have), tested explicitly (`dropping_a_pool_with_occupied_slots_drops_their_values`) rather than left unverified.
- **`STORY-P0-03-02` (compile-time pool-size config) was decomposed but left unimplemented on purpose** — it's about wiring pool capacities into a kernel-wide config surface that doesn't exist yet (no scheduler/IPC subsystem to size pools for), so implementing it now would mean guessing at API shapes for consumers that don't exist. Pick it up once `FEAT-P0-02` or a real IPC path needs a sized pool.

## Immediate next steps

1. In a QEMU-capable environment: pick up `STORY-P0-02-01` (task creation/priority assignment) or `STORY-P0-04-01` (ACPI parsing) — both are ready to implement test-first, and are independent of each other per Cover Note 08's parallelism note, so this is the point to actually try the parallel-subagent approach it flagged.
2. Once a second `FEAT-P0-0X` Story lands, revisit `STORY-P0-03-02` — a real pool consumer (task pool, IPC message pool) will make its acceptance criteria concrete instead of draft.
3. `EPIC-P0`'s exit criteria (`FEAT-P0-01` through `-04` all Verified) is still the standing target; this handover moves `FEAT-P0-03` from 0/3 to 2/3 Stories Verified and gives `FEAT-P0-02`/`-04` a decomposed starting point, but does not close the Epic.
