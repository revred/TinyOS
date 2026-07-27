# STORY-P0-01-02 — CI Pipeline Runs Governance Checks on Every PR

Status: **Verified** (locally; CI run pending)
Feature: [`FEAT-P0-01`](../features/FEAT-P0-01.md)
Introduced in: [`session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md`](../../session/hand-2026-07-26/04-seedmvp-agentmd-goals-vv-model-handover.md)

## Description

CI runs, on every PR from the very first one (per [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md#delivery-strategy-walking-skeleton-first) step 2): `rustfmt` check, `clippy -D warnings`, the crate-size ceiling check (per [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md#crate-size-ceiling-hard-limit-no-exceptions)), `#![deny(missing_docs)]`, the complete 625-cell performance catalogue check, and the mandatory Story/Test/Report performance-and-security assurance-spine check. These gates are proven working while the codebase is still small, not discovered broken once they matter.

## Acceptance criteria

1. A PR that fails any of the four checks is blocked from merging, with a clear failure message identifying which check failed.
2. The crate-size ceiling check correctly reports line counts excluding test code (verified against a fixture crate with a known LOC count).
3. All four checks complete in CI within a few minutes at this codebase size, so the governance gates don't themselves become a development-velocity complaint later.
4. The performance catalogue gate rejects a missing/duplicate/malformed cell and accepts only the complete D01..D25 × G01..G25 cross-product.
5. The assurance-spine gate rejects an unmapped/stale Story or Feature, a Test that resolves to no mapped Story, a Report whose `Test(s) covered:` field names no mapped Story/Test, an unknown assurance ID/state, or an incomplete 20-control security catalogue.
6. The same gate requires exactly five canonical containment classes (`C0`–`C4`), 20 canonical boundary tests (`BND-01`–`BND-20`), exactly one containment contract per Feature, containment classes on every Story/security control, valid Feature boundary-test ownership, and Story classes that stay within the parent Feature's implementation/subject classes.
7. Every `TEST-*` document must carry performance-domain, security-control, containment-class, boundary-test, and assurance-state metadata exactly matching its Story and parent Feature contracts; stale or incomplete Test metadata fails CI.
8. The same gate requires exactly 19 canonical application/platform targets and nine whole-system landing zones; validates every referenced performance domain, security control and containment class; requires every application to be selected by at least one landing zone; and rejects a landing-zone promise unsupported by the applications it names.

## Tests

- [`TEST-P0-01-02-A`](../tests/TEST-P0-01-02-A.md) — CI governance-gate smoke test.

## Goals verified

G-DX-5 (bounded crate size, enforced from day one), G-DX-6 (SOLID review process — the human-review half of this Story, not automatable yet per `agent/CODING_STANDARDS.md`'s enforcement-summary table), G-DX-7 (performance as an implementation spine), G-SEC-10 (one policy/provenance path), G-SEC-12 (bounded governance inputs), and G-SEC-13 (five containment classes enforced through Feature/Story contracts).
