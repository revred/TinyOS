# TEST-P0-07-02-A — A Shared-Memory Grant Never Escalates Permissions and Revokes Deterministically

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-07-02`](../stories/STORY-P0-07-02.md)
Tier: Host (`cargo test -p exec --lib`) plus Tier 0 (QEMU x86_64), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — the cross-address-space page-table manipulation itself needs real target CPU paging semantics, mirroring `TEST-P0-05-02-A`'s own Tier 0 requirement for `AddressSpace`.
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D13`
Security controls: `SEC-03`, `SEC-04`, `SEC-18`, `SEC-20`
Containment classes: `C1`, `C2`, `C3`, `C4`
Boundary tests: `BND-04`, `BND-08`, `BND-09`, `BND-14`, `BND-15`, `BND-20`
Protection Domain contracts: `PD-02`, `PD-03`, `PD-05`, `PD-06`, `PD-08`, `PD-09`, `PD-13`, `PD-14`
Code admission gates: `RCG-08`, `RCG-09`, `RCG-12`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

**Given** an owner `exec::address_space::AddressSpace` with an already-mapped page and a sharee `AddressSpace`,
**when**:
- `exec::shared_memory::grant` is called with permissions no broader than the owner's own mapping, at an unoccupied, non-kernel-reserved sharee address — **then** the sharee's page maps to the *same physical frame* as the owner's, with exactly the requested (not broader) permissions,
- `grant` requests permissions exceeding the owner's own page (e.g. write access to an owner's read-only/execute-only page) — **then** `SharedMemoryError::PermissionsExceedOwner`, nothing mapped,
- `grant` targets a region the owner doesn't actually have mapped, a sharee address already occupied, or a sharee address inside the kernel's reserved region — **then** the corresponding typed error, nothing mapped,
- `exec::shared_memory::revoke` is called by the grant's own owner — **then** the sharee's mapping is deterministically unmapped,
- `revoke` is called by any other `TaskId` — **then** `SharedMemoryError::NotOwner`, and critically the mapping *survives* — the rejected attempt must not consume the caller's only grant token, or the real owner would permanently lose the ability to revoke.

**Added 2026-07-26** (transactional/generation-safe grant, `STORY-P0-07-02` acceptance criterion 5):

- `grant` is called with `pages: 0` — **then** `SharedMemoryError::ZeroPages`, nothing mapped,
- a multi-page `grant` maps its first page successfully but a later page's mapping fails (sharee frame pool exhausted) — **then** the already-mapped page(s) are unmapped again before the error returns, so no partial region survives,
- `grant` succeeds at mapping every page but its `GrantRegistry` has no free slot — **then** the mapping is rolled back the same way and `SharedMemoryError::RegistryExhausted` is returned,
- a grant is revoked and a *different* grant later reused the same `sharee_virt` — **then** presenting the first (now-stale) `SharedGrant` token to `revoke` returns `SharedMemoryError::StaleGrant` and does not unmap the second, unrelated grant.

## Test type

Unit tests (`exec::shared_memory`'s own `#[cfg(test)]` module — ownership/permission-headroom/occupancy validation logic, fully host-testable) plus a Tier 0 QEMU fixture (`exec`'s `shared-memory-fixture` binary) exercising the identical grant/revoke sequence against real target-CPU page tables, mirroring the host/QEMU-both pattern `TEST-P0-05-02-A`/`TEST-P0-05-03-A` already established for `AddressSpace`/`win32_shim`. `hal_x86_64::paging::unmap_4k` (the new primitive `AddressSpace::unmap_page` builds on) has its own dedicated host tests in `paging.rs`.

## Implementation location

`os/src/exec/src/shared_memory.rs`, `os/src/exec/src/fixture_shared_memory_main.rs` (`[[bin]] shared-memory-fixture`), `os/src/exec/src/address_space.rs` (`map_page`/`unmap_page`), `os/src/hal-x86_64/src/paging.rs` (`unmap_4k`).

## Reports

[`REPORT-2026-07-26-22`](../reports/REPORT-2026-07-26-22.md) — Pass.
[`REPORT-2026-07-26-24`](../reports/REPORT-2026-07-26-24.md) — Pass (transactional/generation-safe grant addendum).
