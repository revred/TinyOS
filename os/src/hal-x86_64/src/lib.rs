//! x86_64-specific HAL backend.
//!
//! `acpi` (`STORY-P0-04-01`) locates and parses ACPI tables into
//! [`hal::topology::Topology`]. `paging` (`STORY-P0-05-02`) builds x86_64
//! page tables. `boot`/`qemu_exit` (moved here from `kernel` by
//! `STORY-P0-05-02`) are the shared PVH boot-entry glue and QEMU exit-code
//! reporting every `no_std`/`no_main` binary in this workspace boots
//! through. `idt`/`interrupts` (`STORY-P0-04-02`) are the IDT/local-APIC
//! bring-up; `pci` (`STORY-P0-04-03`) is the read-only configuration-space
//! bus enumeration recording into [`hal::device::DeviceTable`] —
//! completing `FEAT-P0-04`'s x86_64 HAL backend. `fault`
//! (`STORY-P1-02-01`) captures `#UD`/`#GP`/`#PF`; `tss`/`gdt`
//! (`STORY-P1-02-02`) stand up the Interrupt Stack Table that makes a fault
//! *inside* that path survivable rather than a silent triple fault.
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
//! consistency, though it has no ELF-specific content of its own. `serial`
//! (`STORY-P0-03-01`'s `fixture-pool-bench` numeric-evidence UART driver) is
//! gated identically, for the same reason.
//!
//! `tsc` (`STORY-P1-01-01`'s [`hal::time::CycleSource`] backend and its
//! PIT-calibrated timebase) is deliberately **not** gated: its `asm!` is
//! plain port I/O and `RDTSC` with no ELF-specific content, so it assembles
//! under a COFF host assembler too — which matters because its PIT arithmetic
//! carries host unit tests that must be runnable on a Windows dev machine, not
//! only on the Linux CI runner.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod acpi;
#[cfg(not(target_os = "windows"))]
pub mod boot;
pub mod extended_state;
pub mod fault;
pub mod gdt;
pub mod idt;
#[cfg(not(target_os = "windows"))]
pub mod interrupts;
pub mod paging;
pub mod pci;
#[cfg(not(target_os = "windows"))]
pub mod qemu_exit;
pub mod rflags;
#[cfg(not(target_os = "windows"))]
pub mod serial;
pub mod tsc;
pub mod tss;
