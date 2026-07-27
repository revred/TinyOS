# TEST-P0-06-01-A — Spoor Round-Trips Exactly and Rejects Unknown Field Encodings

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-06-01`](../stories/STORY-P0-06-01.md)
Tier: Host (`cargo test -p kernel --lib`), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — `kernel::spoor` is pure bit-packing logic with no target-specific dependency.
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D11`
Security controls: `SEC-06`, `SEC-14`, `SEC-16`, `SEC-19`
Containment classes: `C0`, `C1`, `C2`, `C3`, `C4`
Boundary tests: `BND-17`
Protection Domain contracts: `PD-02`, `PD-08`, `PD-11`, `PD-13`
Code admission gates: `RCG-02`, `RCG-08`, `RCG-12`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

**Given** `kernel::spoor::Spoor`'s `stamp`/`decode` constructors and its typed field accessors,
**when**:
- a spoor is stamped from a valid `(Category, Actor, Action, Outcome, target, cost)` tuple — **then** every accessor returns exactly the value it was stamped with, for every combination of every enum variant crossed with representative `target`/`cost` boundary and mid-range values (12,544 combinations total) — no lossy bit overlap between adjacent fields,
- raw bits with an out-of-range `CAT`, `WHO`, `ACT`, or `OUT` nibble are decoded — **then** `Spoor::decode` fails closed with the corresponding `SpoorError` variant (`UnknownCategory`/`UnknownActor`/`UnknownAction`/`UnknownOutcome`) rather than silently accepting or wrapping an unrecognized discriminant,
- a validly-stamped spoor's raw bits (`to_bits()`) are decoded back — **then** `decode` returns the identical `Spoor`.

## Test type

Property-style unit test (exhaustive over the full domain of every enumerated field, representative-sample over the two integer fields) plus adversarial rejection tests for each field's invalid-nibble case, per `agent/CODING_STANDARDS.md`'s TDD mandate for a wire format other tooling (a future journal reader, or `Sharc.Blue`-side tooling, given the shared bit layout) might parse.

## Implementation location

`os/src/kernel/src/spoor.rs` (`Spoor`, `Category`, `Actor`, `Action`, `Outcome`, `SpoorError`, its `#[cfg(test)]` module).

## Reports

[`REPORT-2026-07-26-15`](../reports/REPORT-2026-07-26-15.md) — Pass.
