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
//! **`LE-102`: not-Windows is not the same condition as bare metal, and the
//! module gate above cannot be the only one.** A Linux host satisfies
//! `not(target_os = "windows")` exactly as `x86_64-tinyos` (`"os": "none"`)
//! does, so on the Linux runner these four modules are compiled into the
//! `hal-x86_64` rlib — and `boot` defines `_start`. That was harmless while
//! CI only ever ran `clippy`, which does not link; the moment `LE-100` added
//! `cargo test --workspace`, `_start` collided with `Scrt1.o`'s in every
//! `std` test harness that links this crate and `hal-x86_64`, `kernel`,
//! `exec` and `hal-arm64` all failed at the linker, so not one host test in
//! the workspace ran. The module gate is deliberately left as it is — the
//! Linux governance job's `clippy --workspace --all-targets` compiles
//! `kernel`'s `[[bin]]`, which writes `use hal_x86_64::boot as _;` ungated,
//! so the module must still exist for an ELF-native host. What moved is the
//! `global_asm!` inside `boot`, now gated on `target_os = "none"` and
//! guarded by [`gate_tests`] below, because from this project's Windows bench
//! the two conditions are one condition and no local gate can tell them
//! apart.
//!
//! `tsc` (`STORY-P1-01-01`'s [`hal::time::CycleSource`] backend and its
//! PIT-calibrated timebase) is deliberately **not** gated: its `asm!` is
//! plain port I/O and `RDTSC` with no ELF-specific content, so it assembles
//! under a COFF host assembler too — which matters because its PIT arithmetic
//! carries host unit tests that must be runnable on a Windows dev machine, not
//! only on the Linux CI runner. `actuation` (`STORY-P1-06-01`'s
//! [`hal::actuation::OutputLine`] backend — the Tier 0 actuator stand-in) is
//! ungated for the same reason and on the same grounds: a bare `out`, no
//! ELF-specific content, and kernel-side host tests that depend on the type.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod acpi;
pub mod actuation;
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

/// `LE-102`'s guard: nothing in `boot` may emit a symbol outside the
/// bare-metal target.
///
/// Lives in `lib.rs` rather than in `boot.rs` and that placement is the whole
/// point — `boot` is itself `#[cfg(not(target_os = "windows"))]`, so a test
/// written inside it does not exist on this project's only development bench
/// and would gate nothing where the mistake is actually made.
#[cfg(test)]
mod gate_tests {
    /// `boot.rs` defines `_start` in a `global_asm!` block. A Linux host
    /// satisfies the module's `not(target_os = "windows")` gate, so without a
    /// `target_os = "none"` gate on the block itself that symbol lands in the
    /// `hal-x86_64` rlib and every `std` test harness linking it gets two
    /// `_start`s — which is exactly how the first CI run of `LE-100`'s
    /// `host-tests` job died, at the linker, in four crates, before a single
    /// test ran.
    ///
    /// COMMENT LINES ARE EXCLUDED, and not as tidiness. `boot.rs`'s own
    /// explanation of this gate contains the string `global_asm!`, so a scan
    /// that did not skip comments would match the prose describing the fix and
    /// then demand a `#[cfg]` above a sentence. That is the same self-match
    /// `metric_labels.rs` hit twice — once on its doc comment and once on its
    /// own error string.
    #[test]
    fn every_global_asm_in_boot_is_gated_to_the_bare_metal_target() {
        let offenders = ungated_global_asm_sites(include_str!("boot.rs"));
        assert!(
            offenders.is_empty(),
            "LE-102: these `global_asm!` blocks in boot.rs are not gated on \
             `target_os = \"none\"`, so they assemble on any ELF host and their \
             `_start` collides with the C runtime's in every std test harness \
             that links this crate: {offenders:?}"
        );
    }

    /// The falsification half, run against text shaped exactly like the defect.
    /// Without it the scan above is satisfied by a file containing no
    /// `global_asm!` at all — which is what a scan that silently matched
    /// nothing would look like, and is `LE-80`'s family.
    #[test]
    fn the_scan_sees_an_ungated_block_and_accepts_a_gated_one() {
        let bad = "// mentions global_asm! harmlessly\nglobal_asm!(\n    \"nop\"\n);\n";
        assert_eq!(ungated_global_asm_sites(bad), vec![2usize]);

        let good = "#[cfg(target_os = \"none\")]\ncore::arch::global_asm!(\n    \"nop\"\n);\n";
        assert!(ungated_global_asm_sites(good).is_empty());

        // The wrong gate must NOT be accepted -- it is the one this row exists
        // for, and a scan that took any `#[cfg(...)]` would have passed the
        // committed defect unchanged.
        let wrong = "#[cfg(not(target_os = \"windows\"))]\nglobal_asm!(\n    \"nop\"\n);\n";
        assert_eq!(ungated_global_asm_sites(wrong), vec![2usize]);
    }

    /// One-based line numbers of `global_asm!` invocations not immediately
    /// preceded by `#[cfg(target_os = "none")]`.
    fn ungated_global_asm_sites(source: &str) -> Vec<usize> {
        const GATE: &str = "#[cfg(target_os = \"none\")]";
        let lines: Vec<&str> = source.lines().map(str::trim).collect();
        let mut sites = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            if line.starts_with("//") || !line.contains("global_asm!(") {
                continue;
            }
            let gated = index.checked_sub(1).is_some_and(|i| lines[i] == GATE);
            if !gated {
                sites.push(index + 1);
            }
        }
        sites
    }
}
