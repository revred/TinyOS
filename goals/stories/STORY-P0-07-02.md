# STORY-P0-07-02 — Shared-Memory Region Handle Exchange Between Two Tasks

Status: **Verified**
Feature: [`FEAT-P0-07`](../features/FEAT-P0-07.md)
Introduced in: [`FEAT-P0-07`](../features/FEAT-P0-07.md), this session (2026-07-26)
Implemented in: [`session/hand-2026-07-26/26-feat-p0-07-local-ipc.md`](../../session/hand-2026-07-26/26-feat-p0-07-local-ipc.md), hardened in [`session/hand-2026-07-26/28-story-p0-07-02-transactional-grants.md`](../../session/hand-2026-07-26/28-story-p0-07-02-transactional-grants.md)

## Description

The "sharing memory between processes" half of `FEAT-P0-07`'s local-IPC scope: `exec::shared_memory::grant`/`revoke` let two tasks (each with its own `exec::address_space::AddressSpace`, per `STORY-P0-05-02`) share a page-aligned region, with explicitly-declared, independently-chosen permissions on the sharee's side that can never exceed what the owner's own mapping actually grants (no implicit "both sides get the same permissions" shortcut, no privilege escalation through a grant).

**Crate placement note.** `FEAT-P0-07.md`'s original "Crate(s) involved" named only `os/src/kernel/`, anticipating a single `kernel::ipc` module for both Stories. This Story's actual dependency (`exec::address_space::AddressSpace`) lives in the `exec` crate, which depends on `kernel` — not the reverse — so implementing shared-memory grants inside `kernel::ipc` would have required a cyclic `kernel`↔`exec` dependency. `exec::shared_memory` (a new module in the crate that already owns `AddressSpace`) is the correct placement; `STORY-P0-07-01`'s message channel has no such dependency and remains in `kernel::ipc` as originally scoped.

To make cross-address-space aliasing possible at all, `exec::address_space::AddressSpace` gained two new public primitives this Story needed and `STORY-P0-05-02`/`-04` never did: `map_page`/`unmap_page`, adding/removing a single page mapping outside of `create`'s own section set (the general case a shared-memory grant needs — a page that isn't part of the sharee's own loaded image). `hal_x86_64::paging` correspondingly gained `unmap_4k`, the counterpart to `map_4k` that clears a leaf PTE.

## Depends on

`STORY-P0-05-02` (`AddressSpace`/section mapping and, since this Story, `map_page`/`unmap_page`), `STORY-P0-03-01` (bookkeeping storage pattern; `SharedGrant` itself is a plain value, not `Pool`-backed, since exactly one grant token exists per grant).

## Acceptance criteria (final)

1. A shared region is created by one task (the owner) and explicitly granted to a second task (the sharee) at an address the owner chooses — never ambiently visible to a third task, and never granted with broader permissions on the sharee's side than the owner's own page actually has (`grant` validates every requested page's owner-side permissions before mapping anything). **Met**: `a_well_formed_grant_maps_the_sharee_to_the_owners_backing_frame`, `a_grant_requesting_write_beyond_the_owners_read_only_page_is_rejected`.
2. The owner revoking a grant unmaps the region from the sharee's address space deterministically — no path leaves a stale, dangling mapping the sharee could still read/write after revocation. **Met**: `revoking_a_grant_unmaps_it_from_the_sharee`.
3. Every failure mode (region not owned/mapped, sharee's target range already occupied, revocation by a non-owner, a `sharee_virt` colliding with the kernel's own reserved region) fails closed with a typed `SharedMemoryError`. **Met**: `granting_an_unmapped_owner_region_is_rejected`, `granting_into_an_already_mapped_sharee_region_is_rejected`, `granting_into_the_kernel_reserved_region_is_rejected`, `revocation_by_a_non_owner_is_rejected`.
4. A rejected revocation attempt (wrong caller) must not consume the grant token — the real owner must still be able to revoke afterward. `revoke` takes the grant by reference, not by value, specifically to avoid this: an earlier draft took it by value, which would have silently destroyed the caller's only token on a failed non-owner attempt, leaving no way to ever revoke a live grant. **Met**: `revocation_by_a_non_owner_is_rejected` asserts the mapping survives the rejected attempt.
5. **(Added 2026-07-26, closing part of the security-spine review's `STORY-P0-07-02` follow-up list.)** `grant` is fully transactional and generation-safe: zero pages is rejected outright; a mid-loop mapping failure (frame exhaustion) or a full [`GrantRegistry`](../../os/src/exec/src/shared_memory.rs) unmaps every page that same call already mapped, never leaving a partial region live; and every successful grant is stamped with a registry-assigned generation, so a stale token from an already-revoked grant can never be confused with — and used to tear down — an unrelated later grant that happens to reuse the same `sharee_virt`. **Met**: `granting_zero_pages_is_rejected`, `grant_rolls_back_partial_mapping_on_frame_exhaustion`, `grant_rolls_back_the_mapping_when_the_registry_is_full`, `revoke_rejects_a_stale_token_whose_address_was_regranted`. **Not met by this addition**: task-exit-triggered revocation (`STORY-P0-07-02`'s security review also asked for this) — deferred, since no task-exit/teardown mechanism exists anywhere in the scheduler yet; registering live grants for it now would have nothing to hook into.

## Tests

`os/src/exec/src/shared_memory.rs`'s `#[cfg(test)]` module (host-testable ownership/grant/permission-headroom validation logic) and `os/src/exec/src/fixture_shared_memory_main.rs` (Tier 0 QEMU fixture proving the same grant/revoke sequence against real target-CPU page-table walk). `os/src/hal-x86_64/src/paging.rs` gained `unmap_4k`'s own host tests. See [`TEST-P0-07-02-A`](../tests/TEST-P0-07-02-A.md), [`REPORT-2026-07-26-22`](../reports/REPORT-2026-07-26-22.md), and [`REPORT-2026-07-26-24`](../reports/REPORT-2026-07-26-24.md).

## Goals verified

`G-PC-3` (no privileged bypass — a shared region visible to an ungranted third task, or one with escalated permissions, would be exactly that), `G-RT-2` (deterministic memory behavior — no unbounded growth in the grant-tracking bookkeeping; a `SharedGrant` is a fixed-size value, not a growable table).
