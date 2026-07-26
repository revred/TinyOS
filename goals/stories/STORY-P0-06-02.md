# STORY-P0-06-02 — Append-Only Spoor Journal Writer

Status: **Verified**
Feature: [`FEAT-P0-06`](../features/FEAT-P0-06.md)
Introduced in: [`FEAT-P0-06`](../features/FEAT-P0-06.md), this session (2026-07-26)
Implemented in: [`session/hand-2026-07-26/21-story-p0-06-02-spoor-journal-implementation.md`](../../session/hand-2026-07-26/21-story-p0-06-02-spoor-journal-implementation.md)

## Description

A fixed-capacity, append-only journal a `Spoor` (`STORY-P0-06-01`) is written into the moment it's stamped — no buffering, no batching, per `Sharc.Blue`'s own doctrine ("stamp immediately at execution point"). `Sharc.Blue`'s own journal is a file (`<workspace>/.sharc/spoor/<session>.journal`, an 8-byte magic header followed by N 8-byte records); TinyOS's Phase 0 equivalent has no filesystem yet, so this Story's journal is a fixed-capacity in-memory ring/append buffer (`Pool`-adjacent, not `Pool<T, N>` itself since a journal is append-and-overwrite-oldest, not alloc/free) — with the same record format, so a later Story that adds real storage can serialize this buffer's contents using the identical 8-byte-magic-plus-records layout `Sharc.Blue` already established, rather than inventing a second format.

## Depends on

`STORY-P0-06-01` (the `Spoor` type this journal stores).

## Acceptance criteria

1. `SpoorJournal<N>` (mirroring `Pool<T, N>`'s and `Scheduler<N>`'s own const-generic-capacity pattern) holds up to `N` spoors with no heap allocation; `append` never blocks and never panics — a full journal overwrites its oldest entry (ring-buffer semantics) rather than rejecting a new stamp, since losing the newest audit event is worse than losing the oldest for a fixed-size trace. **Met**: `kernel::spoor_journal::SpoorJournal<N>`; `tests::a_full_journal_overwrites_its_oldest_entry`, `tests::repeated_wraps_always_retain_only_the_most_recent_capacity_entries`, `tests::a_journal_at_capacity_one_still_retains_only_the_newest_entry` (the degenerate `N=1` case).
2. Reading the journal back (an iterator over currently-held spoors, oldest to newest) never observes a torn/partial write. **Met**: `iter()` (oldest-first, correctly accounting for both the not-yet-wrapped and wrapped cases); `append`'s single write to `entries[self.next]` completes before returning, and Rust's borrow checker already serializes any `&mut self` append against a concurrent `&self` read through the safe API this module exposes — there is no separate concurrent-access surface to test beyond that language guarantee, in this kernel's current single-CPU, no-interrupt execution model.
3. The record format (spoors laid out as consecutive 8-byte values, matching `Sharc.Blue`'s own on-disk record shape) is documented precisely enough that a future storage-backed journal could serialize this buffer directly, byte for byte, without a translation step. **Met**: `entries` stores each `Spoor::to_bits()` value directly (no intermediate representation), and `JOURNAL_MAGIC` (`b"SPOORJ01"`, identical to `Sharc.Blue`'s own on-disk magic) is defined for a future serializer to prepend — `tests::journal_magic_matches_sharc_blues_own_on_disk_format`.

## Tests

`os/src/kernel/src/spoor_journal.rs`'s `#[cfg(test)]` module — 6 tests, entirely host-testable (a ring buffer over `Spoor` values has no target dependency). See [`REPORT-2026-07-26-16`](../reports/REPORT-2026-07-26-16.md) for the full pass record.

## Goals verified

G-PA-6, G-AI-3 (as `FEAT-P0-06`).
