# STORY-P0-08-01 — `xtask pack-txe`: Deterministic PE → TXE Re-Layout

Status: **Verified**
Feature: [`FEAT-P0-08`](../features/FEAT-P0-08.md)
Introduced in: [`FEAT-P0-08`](../features/FEAT-P0-08.md)
Implemented in: [`session/hand-2026-07-26/25-story-p0-05-04-and-txe-packer.md`](../../session/hand-2026-07-26/25-story-p0-05-04-and-txe-packer.md)

## Description

`xtask pack-txe --input=<pe> --output=<txe>` reads a real PE32+ image and re-serializes it so every section's `PointerToRawData` lands on a page (4096-byte) boundary and every section's `SizeOfRawData` equals its `VirtualSize` — a `.bss`-style demand-zero tail is physically zero-written into the output file rather than left implicit. The DOS/PE/COFF/optional headers and the section table are copied verbatim except for each section header's own `PointerToRawData`/`SizeOfRawData` fields, patched in place; every RVA (including the import directory's) is untouched, since RVAs are always re-derived from the section table by any correct reader — never a hardcoded file offset — so relocating section *file* positions never invalidates them.

## Depends on

`exec::pe`'s own byte layout (`STORY-P0-05-01`) — `pack` mirrors just enough of that layout (DOS header, COFF header, section table) to patch it; it deliberately reimplements this rather than depending on the `exec` crate directly, since `xtask` is host-only `std` tooling and `exec`'s PE parsing exists for the kernel-side no_std path.

## Acceptance criteria

1. Every section's new `PointerToRawData` is page-aligned, even when the source image's wasn't. **Met**: `xtask::txe::tests::every_repacked_section_starts_on_a_page_boundary`.
2. Every section's new `SizeOfRawData` equals its `VirtualSize` exactly — no implicit `.bss` gap left for a loader to special-case. **Met**: `every_repacked_sections_raw_size_equals_its_virtual_size`.
3. Real file-backed bytes are preserved exactly; the `.bss` tail (and any page-rounding pad) is physically zeroed, not left as uninitialized/adjacent data. **Met**: `section_content_is_preserved_and_the_tail_is_zeroed`.
4. Two sections with non-page-aligned sizes never overlap once repacked. **Met**: `two_sections_with_non_page_aligned_sizes_never_overlap`.
5. Malformed input (missing DOS signature, truncated) fails closed with a typed `TxePackError`, never a panic. **Met**: `rejects_input_missing_the_dos_signature`, `rejects_truncated_input`.
6. Run against the real `blue-sharc.exe` build artifact, the packed output parses via `exec::pe::parse` with identical section count/permissions/imports/entry point to the original, and `AddressSpace::create` maps every section correctly. **Met**: `STORY-P0-05-04`'s `blue-sharc-fixture` QEMU run, which consumes this tool's real output (`os/src/exec/fixtures/blue-sharc.txe`, produced by `cargo run -p xtask -- pack-txe --input=<real blue-sharc.exe path> --output=os/src/exec/fixtures/blue-sharc.txe`).

## Tests

`os/src/xtask/src/txe.rs`'s `#[cfg(test)]` module — pure host unit tests against hand-built synthetic PE images (deliberately non-page-aligned, mirroring a real linker's default 512-byte `FileAlignment`), run via `cargo test -p xtask` (`xtask` has no `[lib]` target; its `#[cfg(test)]` code compiles as part of the `xtask` binary's own test harness). See [`TEST-P0-08-01-A`](../tests/TEST-P0-08-01-A.md) and [`REPORT-2026-07-26-19`](../reports/REPORT-2026-07-26-19.md).

## Goals verified

G-DX-3 (`xtask` as the single home for build/deploy tooling, per that crate's own doc comment) — this Story is host-side tooling, not a runtime kernel behavior, so it doesn't itself verify a Phase 0 RT/security goal; it's a prerequisite `STORY-P0-05-04` (`G-PC-1`–`G-PC-4`) builds on.
