# STORY-P0-06-01 — Core Packed `Spoor` Type

Status: **Verified**
Feature: [`FEAT-P0-06`](../features/FEAT-P0-06.md)
Introduced in: [`FEAT-P0-06`](../features/FEAT-P0-06.md), this session (2026-07-26)
Implemented in: [`session/hand-2026-07-26/20-strategic-objectives-spoor-ipc-blue-sharc-correction.md`](../../session/hand-2026-07-26/20-strategic-objectives-spoor-ipc-blue-sharc-correction.md)

## Description

The `Spoor` value type itself: a 64-bit packed record encoding category, actor, action, outcome, target, and cost, per `FEAT-P0-06`'s frozen bit layout (mirroring `Sharc.Blue`'s `spoor.rs`/`Spoor.cs` exactly). Constructed via typed builders (`Spoor::stamp` for a fire-and-forget single event, an entry/exit pair for a bracketed action — mirroring `Sharc.Blue`'s "ENTRY + EXIT spoor per atom call" pattern), never via a public bit-twiddling constructor a caller could use to build an invalid or nonsensical record.

## Depends on

None beyond `FEAT-P0-01` (a crate to live in) — this is a pure value type.

## Acceptance criteria

1. `Spoor` is exactly 8 bytes, with typed accessors for each field (`category() -> Category`, `who() -> Actor`, `action() -> Action`, `outcome() -> Outcome`, `target() -> u16`, `cost() -> u32`) that decode the packed bits — never raw bit-shifting exposed to callers. **Met**: `kernel::spoor::Spoor` (a newtype over `u64`), `tests::spoor_is_exactly_eight_bytes`.
2. `Category`, `Actor`, `Action`, `Outcome` are closed Rust enums, not bare integers — an invalid discriminant is a construction-time rejection (a typed `SpoorError`), never silently truncated or wrapped. **Met**: all four are enums with validated `from_bits`; `Spoor::decode` fails closed with `SpoorError::{UnknownCategory,UnknownActor,UnknownAction,UnknownOutcome}` for any nibble outside its field's assigned range — `Category`/`Action` each use 7 of 16 possible nibble values, `Actor` uses 2 of 16, `Outcome` uses all 8 of its assigned range with 8 spare values remaining, all tested explicitly.
3. Construction and every accessor are `const fn` and allocate nothing — a `Spoor` can be built and stamped into a fixed buffer from any real-time path without violating the no-heap-in-hot-path rule. **Met**: `Spoor::stamp`, every accessor, and `Category`/`Actor`/`Action`/`Outcome`'s `to_bits`/`from_bits` are all `const fn`.
4. Round-trip property: for every valid `(category, who, action, outcome, target, cost)` tuple, encoding then decoding returns the exact original tuple — no lossy bit overlap between fields. **Met**: `tests::every_field_combination_round_trips_through_stamp_and_accessors` exhaustively covers every `Category` × `Actor` × `Action` × `Outcome` combination (7×2×7×8 = 784) crossed with 4 representative `target`/`cost` boundary/mid-range samples each (12,544 total combinations), per `agent/CODING_STANDARDS.md`'s adversarial-test expectation for a format any other tooling might parse.

**Scope note, decided when this Story was implemented:** `Category`'s and `Action`'s specific enum variants are TinyOS's own vocabulary (scheduling/lock/wcet/dispatch/exec/memory/boot; create/boost/restore/block/select/overrun/reset-budget) — only the *bit layout* and `Outcome`'s vocabulary are adopted verbatim from `Sharc.Blue`, since `CAT`/`ACT` are inherently per-project taxonomies within a shared wire format (see `kernel::spoor`'s own doc comment for the reasoning). `FEAT-P0-06`'s own text already anticipates this; restated here since it's the acceptance-criteria-relevant detail.

## Tests

`os/src/kernel/src/spoor.rs`'s `#[cfg(test)]` module — 7 tests, entirely host-testable (bit-packing has no target dependency), mirroring `STORY-P0-02-01`'s own host-only precedent. See [`REPORT-2026-07-26-15`](../reports/REPORT-2026-07-26-15.md) for the full pass record.

## Goals verified

G-PA-6, G-AI-3 (the foundational type both goals' eventual audit trails will be built from — see `FEAT-P0-06`'s own framing).
