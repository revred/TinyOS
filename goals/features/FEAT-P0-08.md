# FEAT-P0-08 — TXE: TinyOS-Native Page-Aligned Executable Packaging

Status: **Verified — 1/1 Stories Verified**
Epic: [`EPIC-P0`](../epics/EPIC-P0.md)
Introduced in: (this Feature — 2026-07-26, surfaced while picking up `STORY-P0-05-04`, per a user direction that TinyOS should natively support a page-aligned executable container rather than treating real-world PE file alignment as a per-loader special case)

## Description

A **TXE** is not a new binary format — it is a real PE32+ image, byte-for-byte parseable by `exec::pe::parse` exactly as before, re-laid-out once, deterministically, offline (`xtask pack-txe`) so that every section's on-disk `PointerToRawData` is page-aligned and every section's `SizeOfRawData` equals its `VirtualSize` (any `.bss`-style demand-zero tail physically zero-written into the file, not left implicit). Real-world linkers default to a 512-byte `FileAlignment`, which x86-64 page tables cannot map directly — there is no page-table entry that means "starting from byte 1024 of this buffer." `exec::address_space::AddressSpace::create` (`STORY-P0-05-02`, generalized in `STORY-P0-05-04`) already tolerates arbitrary file alignment via a copy-based mapper, so a TXE is not required for correctness — it exists because doing the unavoidable copy once, at build/deploy time, rather than on every boot, is the more elegant fix, and because a reusable, named native packaging format is valuable in its own right for however TinyOS ends up distributing programs in production, not just for satisfying one Tier 0 fixture's convenience.

This Feature is deliberately narrow: it packages an *executable* only. A native shared-library container (the `.ton` counterpart to `.exe`→`.txe`, named but not built — see `goals/epics/backlog.md`) and any real security scanning/code-signing/sandboxing model for what TXE containers are trusted to do (per the same 2026-07-26 direction) are explicitly out of scope here — the latter needs the real `aci` capability/policy engine (Phase 5) and almost certainly a real IDT/exception-handling subsystem, neither of which exist yet.

## Crate(s) involved

`os/src/xtask/` (new `txe` module) — host-side, `std`, per `docs/mvp-delivery-strategy.md#crate-map`'s `xtask` classification. Nothing runs on the target kernel; a TXE, once produced, is parsed by the same unmodified `exec::pe::parse`/`AddressSpace::create` any other PE goes through.

## Depends on

`FEAT-P0-05`'s `STORY-P0-05-01` (the parser a TXE must still satisfy byte-for-byte).

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P0-08-01`](../stories/STORY-P0-08-01.md) | `xtask pack-txe`: deterministic PE→TXE re-layout | Verified |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C3** · subject **C4** · boundary tests **BND-10, -11, -12, -13**.

That row also selects this Feature’s [`PD-*`](../security/protection-domain-contracts.tsv) and [`RCG-*`](../security/code-admission-gates.tsv) Security Charter obligations. Every Test repeats the exact selections and CI rejects drift.

Packaging changes layout only. A TXE emitted by this C3 build tool remains C4 candidate data until independent content hash, signature, origin, entitlement, anti-rollback, import, mapping, and policy checks create a fresh C3 runtime process. Required evidence includes deterministic reproduction, metadata preservation, substitution detection, provenance monotonicity, quarantine/no-execute behavior, and proof that packaging cannot sign, promote, install, or grant authority.

## Exit criteria

- `STORY-P0-08-01` reaches **Verified**. **Met.**
- The packed output still parses via `exec::pe::parse` with identical section permissions, identical imports, identical entry point — a TXE changes on-disk layout only, never the image's own logical contents. **Met**, proven both by `xtask`'s own host tests and by `STORY-P0-05-04`'s QEMU fixture successfully parsing/mapping the real `blue-sharc.txe` this tool produced.
