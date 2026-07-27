# FEAT-P9-08 — Layout Randomization

Status: **Specified — no Story started**
Epic: [`EPIC-P9`](../epics/EPIC-P9.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §3, §6 (R5), §12

## Description

Last on merit, and worth being clear about why, because randomization is the mechanism the original request suggested.

**Randomization does not defeat a dump.** Layout unpredictability raises cost for A2 only, because A2 must commit to an address *before* observing memory. A3 does the opposite: it observes first and acts never. Against a dump an analyst does not guess, they scan — PE headers, section tables, import strings, page-table structure and known constants all survive relocation, and the second pass finds whatever the first pass moved.

It is still worth doing, and its justification is **threefold** rather than the single one the proposal's first draft recorded ([`session/hand-2026-07-28/08-memory-confidentiality-review.md`](../../session/hand-2026-07-28/08-memory-confidentiality-review.md) §12):

1. It degrades A2, the only adversary it degrades.
2. Under [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4b a randomized image base and process id are **entropy inputs to the key derivation** — today they contribute **zero bits**, since the base is hardcoded in 15 sites and process ids are sequential. This is defence in depth and never a substitute: `root_secret` carries the secrecy and must continue to do so alone.
3. It removes the fixed constants that fingerprint a dump as TinyOS in one pass — `0x1_4000_0000` and `0xdead_0000` — which is the cost [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4d names for publishing the source.

**The reproducibility objection against this Feature is withdrawn.** The first draft deferred R5 partly because randomization "fights reproducibility, which this project spends". Reproducibility under encryption is a solved problem with a shipped precedent, and per [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §4d the debug/production split is *safe* under publication because a debug build measures differently and so never receives a key. R5 stays last on its real merits — not on that one.

## Crate(s) involved

`os/src/exec/` (image base and load layout), `os/src/kernel/` (process ids), `os/src/hal-x86_64/`

## Depends on

`FEAT-P9-02`. Nothing else — its position is a priority judgement, not a dependency.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P9-08-01`](../stories/STORY-P9-08-01.md) | Randomized image base and process id, with the fingerprint constants removed | Specified |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0** · subject **C0/C1/C2** · boundary tests **BND-04, -14, -17**.

Where a process lands changes nothing about what it may do: no capability, no class, no priority follows from an address (`PD-01` private active address spaces, `PD-11` non-increasing provenance, `PD-14` no ambient or class-derived authority, `RCG-10` exact fresh process mapping).

## Exit criteria

Its Story **Verified** at Tier 0: image base and process id vary per boot with measured entropy, the fingerprint constants no longer appear in a dump (measured on `FEAT-P9-01`'s scanner), the deterministic mode is explicit and unavailable in a shipping image, and the `D04`/`D05` baselines still hold.
