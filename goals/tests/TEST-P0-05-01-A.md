# TEST-P0-05-01-A — PE64 Image Parsing Fails Closed on Untrusted Input

Status: **Verified**
Story: [`STORY-P0-05-01`](../stories/STORY-P0-05-01.md)
Tier: Host unit test (no QEMU/hardware dependency), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — mirrors `hal_x86_64::acpi`'s pure-parsing/unsafe-boundary split (`TEST-P0-04-01-A`'s sibling host tests).

## Specification

**Given** a byte slice claimed to be a PE64 executable image (either a hand-crafted fixture, or real bytes from a `Sharc.Blue` `blue-sharc.exe` build),
**when** it is parsed by `exec`'s PE loader into a `LoadDescriptor`,
**then**:

- a well-formed image (real `blue-sharc.exe` bytes, at minimum) parses successfully into a `LoadDescriptor` whose section list, permissions, and import list match what a reference tool (e.g. `objdump`/`llvm-readobj` run against the same file, checked by hand when this Test is implemented) reports — **implemented against a hand-crafted fixture** (`pe::tests::parses_a_well_formed_image`); a real `blue-sharc.exe` comparison is still deferred to `STORY-P0-05-04` per Handover 11's open sourcing decision,
- a truncated file (cut off mid-header, or mid-section-table) is rejected with a typed error, never a panic or an out-of-bounds read (`pe::tests::rejects_truncated_header`, `rejects_truncated_section_table`, `no_truncation_length_panics`),
- a section whose declared file offset + size exceeds the actual file length is rejected, never read past the file's real bounds (`pe::tests::rejects_section_data_past_end_of_file`),
- a section requesting both write and execute permission is rejected at parse time (W^X enforcement), even if every other field in the file is well-formed (`pe::tests::rejects_write_and_execute_section`),
- parsing is deterministic and side-effect-free — parsing the same bytes twice yields the same `LoadDescriptor` (or the same rejection), and rejection never partially constructs a descriptor a caller could accidentally use (`pe::tests::parsing_is_deterministic`).

## Test type

Host unit test — per acceptance criterion 4 of `STORY-P0-05-01`, parsing takes an already-obtained `&[u8]` and does no I/O or `unsafe`, so it's testable with hand-crafted byte fixtures the same way `hal_x86_64::acpi`'s SDT/RSDP parsing is, without a QEMU round-trip. Fuzz testing is expected here too, per `agent/CODING_STANDARDS.md`'s requirement for "any parser that accepts external input" — a malformed/adversarial PE file is exactly that; no `cargo-fuzz` harness exists yet anywhere in this repo, so `no_truncation_length_panics` (exhaustive truncation-length coverage of the one fixture) stands in as an interim, narrower substitute — see `REPORT-2026-07-26-07`'s "Deliberately not done" section.

## Implementation location

`os/src/exec/src/pe.rs`.

## Reports

[`REPORT-2026-07-26-07`](../reports/REPORT-2026-07-26-07.md) — 14/14 tests passing, `cargo fmt`/`clippy` clean, crate size 417 lines.
