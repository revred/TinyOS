# FEAT-P2-02 — Volume Abstraction, G-SEC-5 Label Carriage, RAM-Backed Volume

Status: **In progress — Story 01 started 2026-07-30** (assurance `baseline-debt`: `D14` is
`stand-in-only` readiness — a RAM volume is the stand-in; block-device storage is Phase 3)
Epic: [`EPIC-P2`](../epics/EPIC-P2.md) §7 priority 2 — **the `LE-48` answer**, proposed by
EPIC-P2 §5 and **accepted by the owner 2026-07-30** ("fill all the gaps"), which is the
decision `LE-48` demanded be written down
Introduced in: [`session/hand-2026-07-29/16G`](../../session/hand-2026-07-29/16G-tinycmd-vertical-slice.md)

## Description

A volume abstraction with **`G-SEC-5` labels carried from the first commit** — origin, signer,
trust, entitlement, quarantine, derivation — and a fixed-capacity RAM-backed volume as its
first implementation. Labels survive `copy`, `move`/`rename` and every other transform
(`BND-13`: transforms cannot upgrade provenance; the copy of a quarantined file is
quarantined). Fixed capacities throughout (bounded files, bounded directories, bounded name
and file sizes — the capacities doctrine); exhaustion is a clean refusal, never a panic
(`SEC-20`). No heap, no `unsafe`. The Phase 3 block-device driver becomes a second backing
behind this same seam, which is exactly why the labels are designed in now rather than
retrofitted through three flavours' file verbs.

## Crate(s) involved

`os/src/shell/` (the `volume` module; splits into its own crate if it approaches the ceiling).

## Depends on

`FEAT-P2-01` (verbs consume the volume through the core, never directly).

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P2-02-01`](../stories/STORY-P2-02-01.md) | Labelled RAM volume: fixed capacities, label propagation under copy/move, fail-closed exhaustion | In progress |

## Containment contract

See `goals/assurance/feature-contracts.tsv` row `FEAT-P2-02`. Hostile inputs: adversarial
file names (traversal, escape sequences, over-length), label-stripping attempts via
copy/rename chains, capacity exhaustion. Storage authority is a per-session object capability
(`SEC-07`); labels are `SEC-06`; a transform that would drop a label is a refused operation,
not a warning.
