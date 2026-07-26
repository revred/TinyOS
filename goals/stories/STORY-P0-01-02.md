# STORY-P0-01-02 — CI Pipeline Runs Governance Checks on Every PR

Status: **Planned**
Feature: [`FEAT-P0-01`](../features/FEAT-P0-01.md)
Introduced in: [`session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md`](../../session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md)

## Description

CI runs, on every PR from the very first one (per [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md#delivery-strategy-walking-skeleton-first) step 2): `rustfmt` check, `clippy -D warnings`, the crate-size ceiling check (per [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md#crate-size-ceiling-hard-limit-no-exceptions)), and `#![deny(missing_docs)]`. These gates are proven working while the codebase is still trivially small, not discovered broken once they matter.

## Acceptance criteria

1. A PR that fails any of the four checks is blocked from merging, with a clear failure message identifying which check failed.
2. The crate-size ceiling check correctly reports line counts excluding test code (verified against a fixture crate with a known LOC count).
3. All four checks complete in CI within a few minutes at this codebase size, so the governance gates don't themselves become a development-velocity complaint later.

## Tests

- [`TEST-P0-01-02-A`](../tests/TEST-P0-01-02-A.md) — CI governance-gate smoke test.

## Goals verified

G-DX-5 (bounded crate size, enforced from day one), G-DX-6 (SOLID review process — the human-review half of this Story, not automatable yet per `agent/CODING_STANDARDS.md`'s enforcement-summary table).
