# STORY-P9-05-01 — Per-Process Derivation, Per-Surface Subkeys, and the Wipe Discipline

Status: **Specified, not yet started. Gated on `LE-09`.**
Feature: [`FEAT-P9-05`](../features/FEAT-P9-05.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4b

## Description

Implement the derivation and the lifetime rules around it.

## Depends on

`STORY-P9-02-01`, `STORY-P9-03-01`, `STORY-P9-04-01`.

## Acceptance criteria (draft — to be finalized when this Story starts)

1. **Derivation is deterministic per (machine, process, generation) and diverges on any input change.** Two processes differ; two generations of the same process differ; two machines differ. Each of those is a separate assertion, because each corresponds to a different attack.
2. **Per-surface subkeys are genuinely separate**: compromising the spoor key must not read the image-at-rest surface. Tested by construction, not by inspection.
3. **Rotation is keyed to `TeardownGeneration`** and not to a second counter of its own. `STORY-P1-03-02` established that generation as the epoch for `PD-13`; a parallel counter that can drift from it is a defect waiting to happen.
4. **Wipe is verified externally.** After use, the derived key is absent from a guest dump — checked with `FEAT-P9-01`'s scanner, not with an assertion in the function that performed the wipe. The optimizer-elision hazard from `STORY-P9-01-02` criterion 2 applies here with higher stakes.
5. **The public metadata is never credited with secrecy**, and the code says so where it is assembled. This is the misunderstanding most likely to be reintroduced by a later well-meaning change, so it belongs in the source and not only in this document.
6. **Derivation cost is measured on hardware** and holds the p99 budget [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4c's hibernate argument depends on. **Not measurable at Tier 0** — a TCG figure here would be as misleading as the `D04` cross-space number was, and [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §11.5 records this falsification as blocked on `LE-09` rather than deferred silently.

## Explicitly out of scope

- What the keys are used *for* — that is `FEAT-P9-06` and `FEAT-P9-07`.

## Tests

Not yet written — deferred until this Story starts. Host tests can cover derivation divergence and surface separation with an injected root; the wipe verification and the cost measurement are hardware-tier.

## Goals verified

G-SEC-15 — the first real implementation behind that control.
