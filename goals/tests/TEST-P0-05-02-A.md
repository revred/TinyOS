# TEST-P0-05-02-A — Section Mapping Enforces Declared Permissions and Fails Closed on Collision

Status: **Implemented and passing**
Story: [`STORY-P0-05-02`](../stories/STORY-P0-05-02.md)
Tier: Tier 0 (QEMU x86_64), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — real CPU page-table construction is not something a host unit test can fully substitute for.

## Specification

**Given** a set of section descriptors and an image base,
**when** `exec::address_space::AddressSpace::create` builds a new process address space and maps its sections,
**then**:
- each section's mapped pages carry exactly the read/write/execute permission bits it declared, read back via the constructed page-table entries — a read-only section's pages never have the writable bit set, and vice versa,
- no mapped page is ever both writable and executable (W^X held at the actual page-table level, not just checked at PE-parse time by `STORY-P0-05-01`),
- a section set with two sections claiming overlapping virtual address ranges is rejected before any mapping occurs — partial mapping followed by failure is not acceptable; either the whole address space is mapped or none of it is,
- a section set requesting a virtual address range that collides with the kernel's own identity-mapped region (`[0, 0x4000_0000)`) is rejected, never silently remapped over kernel memory,
- dropping an `AddressSpace` reclaims every page-table frame it allocated — verified by mapping-and-tearing-down an address space repeatedly (32 cycles in the QEMU fixture, 10 in the host test) against the same fixed-capacity frame pool and confirming it never exhausts.

**Revision note (this Story's implementing session):** the original specification's "a page in a read-only section faults... if code under test attempts to write to it" is **not** how permission correctness is verified. Actually loading the constructed tree into `CR3` and inducing/catching a live `#PF` would require an IDT/exception-handling subsystem this kernel does not yet have — building one was judged out of this Story's scope (a prerequisite for a *future* Story, not a silent workaround here). Permission correctness is instead verified deterministically by reading the constructed PTEs back (`hal_x86_64::paging::translate`) — equally conclusive for "did `map_4k` set the bits `AddressSpace::create` was asked to set," just not a live-fault demonstration. `STORY-P0-05-02.md`'s acceptance criterion 1 records this explicitly.

## Test type

Tier 0 integration test, per `agent/CODING_STANDARDS.md`'s "every driver/kernel path targets at minimum a Tier 0 test" mandate, plus host unit test coverage of the same logic:
- `hal_x86_64::paging`'s page-table primitives (`cargo test -p hal-x86_64 --lib`, `paging::tests`) — pure `u64`-array manipulation, no target-specific instructions.
- `exec::address_space`'s section-overlap/kernel-collision validation and full `AddressSpace::create`/`Drop` lifecycle (`cargo test -p exec --lib`, `address_space::tests`).
- The Tier 0 QEMU fixture below runs the identical sequence a second time under real target CPU semantics.

## Implementation location

`os/src/hal-x86_64/src/paging.rs` (page-table construction primitives) and `os/src/exec/src/address_space.rs` (validation + `AddressSpace`), exercised under QEMU by `os/src/exec/src/fixture_main.rs` — `exec`'s **own** `exec-fixture` `[[bin]]` target, not another `kernel` Cargo feature like `fixture-broken-boot`/`fixture-context-switch`. `exec` depends on `kernel`'s library for `kernel::mem::Pool`; `kernel`'s own binary depending back on `exec` for this fixture would be a cyclic crate dependency. `xtask qemu-x86_64 --fixture=address-space` builds and boots `exec`'s `exec-fixture` binary instead of `kernel`'s. The shared PVH boot-entry glue (`boot.rs`, `qemu_exit.rs`) moved from `kernel` into `hal-x86_64` as part of this Story so both binaries can reuse it — see `hal_x86_64::boot`'s doc comment.

## Reports

[`REPORT-2026-07-26-09`](../reports/REPORT-2026-07-26-09.md).
