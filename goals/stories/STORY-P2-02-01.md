# STORY-P2-02-01 — Labelled RAM Volume: Fixed Capacities, Label Propagation, Fail-Closed

Status: **In progress** — assurance state `baseline-debt` (`D14` readiness `stand-in-only`)
Feature: [`FEAT-P2-02`](../features/FEAT-P2-02.md)
Introduced in: [`session/hand-2026-07-29/16G`](../../session/hand-2026-07-29/16G-tinycmd-vertical-slice.md)
Started: 2026-07-30

## Description

The RAM-backed volume behind the volume seam: fixed-capacity files, directories, name and
data sizes (`capacities.rs` is the single reviewable location); every file carries the
`G-SEC-5` label set (origin, signer, trust, entitlement, quarantine, derivation) from
creation. `copy` and `move`/`rename` propagate labels bit-for-bit, and derivation records the
transform; nothing can strip or upgrade a label through any verb path (`BND-13`).

## Acceptance criteria

1. Create/read/list/delete/rename/copy against fixed capacities; each capacity's exhaustion
   is a typed refusal (register message shapes), never a panic, and the volume stays
   consistent afterwards.
2. A quarantined file's copy is quarantined; a rename chain cannot shed the label; the
   adversarial test drives copy→rename→copy and asserts label identity end to end.
3. Path handling refuses traversal (`..` beyond root, absolute escapes) — the adversarial
   path tests `cli-compatibility-mvp.md` §Safety demands.
4. Deterministic enumeration order (insertion order), fixed timestamps at Tier 0 — the
   golden-transcript dependency.

## Not claimed

No persistence (RAM only — the Phase 3 block backing lands behind the same seam). No
performance number (`D14` open debt). 8.3-name aesthetics are formatting, not a storage
constraint.
