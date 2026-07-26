# STORY-P0-05-02 — Process Address-Space Creation and Section Mapping

Status: **Verified**
Feature: [`FEAT-P0-05`](../features/FEAT-P0-05.md)
Introduced in: [`session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md`](../../session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md)
Implemented in: [`session/hand-2026-07-26/14-story-p0-05-02-address-space-implementation.md`](../../session/hand-2026-07-26/14-story-p0-05-02-address-space-implementation.md)

## Description

Given a `LoadDescriptor` (`STORY-P0-05-01`)'s sections, build a real x86_64 page-table tree mapping each section at its declared virtual address with its declared permissions — the step that turns a validated-but-inert description of a PE image into memory a CPU could actually execute out of. Section/page-table-frame storage goes through `kernel::mem::Pool<T, N>` (`FEAT-P0-03`'s `STORY-P0-03-01`, already Verified), not a heap allocation, consistent with `agent/CODING_STANDARDS.md`'s RT-path allocation discipline.

## Depends on

`STORY-P0-05-01` (the `LoadDescriptor`/`SectionDescriptor` types this Story maps — though see "implementation note" below on exactly how loosely coupled the two ended up), `STORY-P0-03-01` (the `Pool<T, N>` allocator for page-table-frame bookkeeping), and `FEAT-P0-02` through at least `STORY-P0-02-02` (context switch) — mapping a process's memory is only useful once the scheduler can actually run something in it.

## Acceptance criteria

Finalized this session (the Story was introduced with these left as "draft" pending implementation-time scoping — see the implementing session's handover for the full reasoning behind each deferral below):

1. Every mapped page's page-table entry carries exactly the read/write/execute permissions its section declared — never more permissive than requested (a read-only section is never left writable). **Met**, verified by reading the constructed page-table entries back (`hal_x86_64::paging::translate`, wrapped by `AddressSpace::translate`) rather than by inducing and catching a live CPU `#PF`: this Story's tree is never loaded into `CR3` (see criterion 4), and this kernel has no IDT/exception-handling subsystem yet to catch a fault even if it were — flagged as a gap for whichever future Story adds one, not silently worked around.
2. A section set with two sections claiming overlapping virtual address ranges, or a virtual address range colliding with the kernel's own identity-mapped region (`boot.rs`'s first-1GiB huge-page map, `[0, 0x4000_0000)`), fails closed with a typed `AddressSpaceError`, never a panic or a silent overwrite of kernel memory — and never a partial mapping: validation runs over the *whole* section set before any page table is touched. **Met**.
3. Address-space creation and teardown is symmetric: dropping an `AddressSpace` reclaims every page-table frame it allocated, with no leaked frame surviving past the drop — mirrors `kernel::mem::Pool`'s own `Drop` discipline (`STORY-P0-03-01`'s implementation note), applied one level up. **Met**, verified by 10 (host test) and 32 (QEMU fixture) repeated create/drop cycles against the same fixed-capacity frame pool never exhausting it.
4. This Story does not implement per-process page-table isolation: the constructed tree is never loaded into `CR3`, so there is no live "current address space" concept and no distinct address space per task yet — that's scoped precisely when a future Story adds real task dispatch, against whatever `FEAT-P0-02`'s scheduler has established about task memory ownership by then. **Met by design**, not a partial implementation of isolation.

## Tests

Two layers, mirroring `STORY-P0-05-01`'s own host/Tier-0 split:

- **Host-testable** (`cargo test -p hal-x86_64 --lib`, `cargo test -p exec --lib`): `hal_x86_64::paging`'s page-table construction/read-back (`map_4k`/`translate`, permission bits, remap rejection, frame exhaustion) and `exec::address_space`'s section-overlap/kernel-collision validation and full `AddressSpace::create`/`Drop` lifecycle, all pure `u64`-array manipulation with no target-specific instructions.
- **Tier 0** (`cargo run -p xtask -- qemu-x86_64 --fixture=address-space`): a **separate** `no_std`/`no_main` binary, `exec`'s own `exec-fixture` `[[bin]]` — not a `kernel` Cargo feature like `broken-boot`/`context-switch` are, because `exec` depends on `kernel`'s library for `mem::Pool`, and `kernel`'s own binary depending back on `exec` would be a cyclic crate dependency. Exercises the real `AddressSpace::create` end to end under QEMU/target CPU semantics.

See [`TEST-P0-05-02-A`](../tests/TEST-P0-05-02-A.md) for the full specification and [`REPORT-2026-07-26-09`](../reports/REPORT-2026-07-26-09.md) for the verification run.

## Goals verified

G-PC-1 (native executable loading — the mapping half), G-RT-2 (deterministic memory behavior — no heap allocation on this path).
