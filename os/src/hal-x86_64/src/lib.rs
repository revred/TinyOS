//! x86_64-specific HAL backend.
//!
//! `acpi` (`STORY-P0-04-01`) locates and parses ACPI tables into
//! [`hal::topology::Topology`]. `paging` (`STORY-P0-05-02`) builds x86_64
//! page tables. `boot`/`qemu_exit` (moved here from `kernel` by
//! `STORY-P0-05-02`) are the shared PVH boot-entry glue and QEMU exit-code
//! reporting every `no_std`/`no_main` binary in this workspace boots
//! through. Bus enumeration and APIC bring-up land in the rest of
//! `FEAT-P0-04`.
//!
//! `#![no_std]` is suppressed under `cfg(test)` so `cargo test` links the
//! host's `std` test harness, matching `kernel`'s `lib.rs` split. `boot` and
//! `qemu_exit` are additionally gated to `not(target_os = "windows")`:
//! `boot` contains `global_asm!` using ELF section directives
//! (`.note.pvh`, `.boot`), which assemble fine both under the real
//! `x86_64-tinyos` target (`"os": "none"`) and under an ELF-native host
//! toolchain (e.g. `cargo clippy --workspace --all-targets` on this
//! project's Linux CI runner) but not under a COFF-flavored host assembler
//! (Windows) — the same reason `kernel`'s own `[[bin]]`, which housed this
//! code before `STORY-P0-05-02` moved it here, was never buildable via a
//! bare host `cargo build`/`clippy` on a Windows dev machine either.
//! `qemu_exit`'s `out`-instruction `asm!` is gated the same way for
//! consistency, though it has no ELF-specific content of its own.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod acpi;
#[cfg(not(target_os = "windows"))]
pub mod boot;
pub mod paging;
#[cfg(not(target_os = "windows"))]
pub mod qemu_exit;
