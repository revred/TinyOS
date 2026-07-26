# STORY-P0-07-02 — Shared-Memory Region Handle Exchange Between Two Tasks

Status: **Planned, not yet started**
Feature: [`FEAT-P0-07`](../features/FEAT-P0-07.md)
Introduced in: [`FEAT-P0-07`](../features/FEAT-P0-07.md), this session (2026-07-26)

## Description

The "sharing memory between processes" half of `FEAT-P0-07`'s local-IPC scope: a mechanism for two tasks (each with its own `exec::address_space::AddressSpace`, per `STORY-P0-05-02`) to have a page-aligned region mapped into both, with explicitly-declared, independently-chosen permissions per side (e.g. read-write for the owner, read-only for the sharee) — never an implicit "both sides get the same permissions" shortcut.

## Depends on

`STORY-P0-05-02` (`AddressSpace`/section mapping — the region this Story maps into a second address space is exactly the kind of mapping that Story's `paging::map_4k` already performs), `STORY-P0-03-01` (bookkeeping storage for the handle/ownership table).

## Acceptance criteria (draft — to be finalized when this Story starts)

1. A shared region is created by one task (the owner) and explicitly granted to a second task (the sharee) — never ambiently visible to a third task, and never granted with broader permissions on the sharee's side than the owner explicitly specified.
2. The owner revoking a grant unmaps the region from the sharee's address space deterministically — no path leaves a stale, dangling mapping the sharee could still read/write after revocation.
3. Every failure mode (region already granted, sharee's address space has no room, revocation by a non-owner) fails closed with a typed error, per this codebase's established "no stringly-typed errors" rule.

## Tests

Not yet written — deferred until this Story is picked up. Expect Tier 0 (QEMU) verification for the actual cross-address-space page-table manipulation, mirroring `STORY-P0-05-02`'s own Tier 0 requirement, plus host-testable validation logic for the ownership/grant bookkeeping.

## Goals verified

`G-PC-3` (no privileged bypass — a shared region visible to an ungranted third task would be exactly that), `G-RT-2` (deterministic memory behavior — no unbounded growth in the grant-tracking bookkeeping).
