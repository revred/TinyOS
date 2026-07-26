# STORY-P0-05-01 — PE64 Image Parsing into a Validated, Typed Load Descriptor

Status: **Verified**
Feature: [`FEAT-P0-05`](../features/FEAT-P0-05.md)
Introduced in: [`session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md`](../../session/hand-2026-07-26/10-feat-p0-02-01-and-p0-04-01-parallel-verification.md)
Implemented in: [`session/hand-2026-07-26/12-story-p0-05-01-pe64-parser-implementation.md`](../../session/hand-2026-07-26/12-story-p0-05-01-pe64-parser-implementation.md)

## Description

Parse a PE64 (`PE32+`) executable image — DOS stub/header, COFF header, optional header, section table, and import directory — into a validated, typed `LoadDescriptor` the rest of `FEAT-P0-05` consumes, without trusting any offset or length the file itself claims until it's been checked against the file's actual size. This is the same untrusted-firmware-input discipline `hal_x86_64::acpi` already established for ACPI tables (`STORY-P0-04-01`): a PE file is external, potentially adversarial input from the kernel's own trust-boundary perspective (`G-PC-4`), not merely a format to deserialize optimistically.

## Depends on

`FEAT-P0-01` (a booting/host-testable kernel workspace to build this crate inside — this Story itself is expected to be entirely host-testable, no QEMU dependency, mirroring `STORY-P0-02-01`'s split).

## Acceptance criteria

1. Parsing rejects a malformed/truncated PE image with a typed error (no stringly-typed errors, per `agent/CODING_STANDARDS.md`) rather than reading past a section/table's declared bounds — every offset and length the file header claims is checked against the actual file size before it's trusted, mirroring `hal_x86_64::acpi::AcpiError`'s fail-closed pattern. **Met**: `pe::PeError` is a hand-rolled, `no_std`-friendly enum (no `Display`/string messages); every offset/length read goes through `checked_add`/`.get()`-bounds-checked helpers before use.
2. The `LoadDescriptor` records, per section: virtual address, size, file offset, and required permissions (read/write/execute) — and construction fails closed if any section requests both write and execute permission (`G-PC-1`'s W^X requirement is enforced at parse time, not deferred to the mapper). **Met**: `pe::SectionDescriptor` carries all four fields; `parse` rejects a write+execute section with `PeError::WriteExecuteSection` before it's ever stored.
3. The import directory is parsed into a list of (DLL name, imported symbol name) pairs the loader can check against `STORY-P0-05-03`'s allowlist, without resolving or loading anything yet — resolution is `STORY-P0-05-03`'s job, not this Story's. **Met**: `LoadDescriptor::imports()` yields `pe::ImportEntry { dll_name, symbol_name }` pairs; no allowlist check or resolution happens in this module. Ordinal-only imports (no name) are not recorded — see `REPORT-2026-07-26-07`'s "Deliberately not done" section.
4. Parsing is pure (no `unsafe`, no I/O) — it operates on an already-obtained `&[u8]` (the whole file, or a validated prefix), so it's fully host-testable against hand-crafted and real (`blue-sharc.exe`) fixture bytes without needing QEMU, mirroring the pure/unsafe-boundary split in `hal_x86_64::acpi`. **Met**: `pe::parse` contains zero `unsafe` blocks (verified: no `unsafe` keyword anywhere in `os/src/exec/src/pe.rs`); tested only with hand-crafted fixture bytes — a real `blue-sharc.exe` fixture is still not sourced (deferred to `STORY-P0-05-04`, per Handover 11's open decision).

## Tests

Implemented as `#[cfg(test)]` functions in `os/src/exec/src/pe.rs`, run via `cargo test -p exec --lib` — the same location and Tier `TEST-P0-05-01-A` specifies. See [`TEST-P0-05-01-A`](../tests/TEST-P0-05-01-A.md) for the full mapping from test function to specification bullet, and [`REPORT-2026-07-26-07`](../reports/REPORT-2026-07-26-07.md) for the verification run.

## Goals verified

G-PC-1 (native executable loading — the parsing half), G-PC-4 (small, auditable loader TCB — this Story is where untrusted-input discipline is established for the whole Feature).
