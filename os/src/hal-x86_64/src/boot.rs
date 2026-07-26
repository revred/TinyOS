//! The Xen PVH boot note and the 32-bit-to-long-mode boot entry.
//!
//! QEMU's built-in PVH direct-boot loader (invoked via `-kernel`) starts the
//! CPU in 32-bit protected mode with paging disabled. This module's job ends
//! the moment it hands off to 64-bit Rust code in an `extern "C" fn
//! kernel_main` — no kernel logic lives here, only the mode transition every
//! x86_64 kernel needs regardless of what it does next.
//!
//! Lives in `hal-x86_64` rather than `kernel` (where `STORY-P0-01-01`
//! originally placed it) as of `STORY-P0-05-02`, so more than one
//! `no_std`/`no_main` binary can reuse the same PVH entry glue: `kernel`'s
//! own `[[bin]]` and `exec`'s new `exec-fixture` `[[bin]]` (needed because
//! `exec` depends on `kernel`'s library for `kernel::mem::Pool`, which rules
//! out `kernel`'s own binary depending back on `exec` for a Tier 0 fixture
//! without a cyclic crate dependency). A dependent crate only needs to link
//! against `hal-x86_64` (which both already do) and define its own
//! `#[no_mangle] extern "C" fn kernel_main` and `#[panic_handler]` — the
//! linker script's `ENTRY(_start)` and
//! `KEEP(*(.note.pvh))`/`KEEP(*(.boot))` directives (see
//! `targets/x86_64-tinyos.ld`) pull this module's object code out of
//! whichever rlib it was compiled into, exactly as they already pull it out
//! of `kernel`'s own compilation unit.
//!
//! Written in AT&T syntax (GNU `as`/LLVM-MC default for x86), matching the
//! directive style (`.section`, `.quad`, ...) used throughout.
//!
//! Per the PVH boot protocol, `EBX` holds the physical address of the
//! `hvm_start_info` struct at `_start` and is never touched by this
//! module's page-table/GDT setup (which only uses `EAX`/`ECX`/`EDX`), so it
//! survives untouched to `long_mode_entry`, where it's moved into `EDI`
//! (the SysV x86-64 calling convention's first integer argument) before
//! `kernel_main` is called — `hal_x86_64::acpi::discover_topology`
//! (`STORY-P0-04-01`) is what reads it, when the caller's `kernel_main`
//! chooses to.

use core::arch::global_asm;

// QEMU's built-in `-kernel` direct-boot loader (hw/i386/multiboot.c) only
// understands the Multiboot **1** header (32-bit ELF only) and Linux bzImage
// — neither fits a 64-bit ELF built from a `no_std` Rust target. The Xen PVH
// boot protocol is QEMU's direct-boot path for exactly this case (a raw
// ELF64 kernel, no GRUB/bootloader in between): an ELF note advertising a
// 32-bit physical entry point, per `XEN_ELFNOTE_PHYS32_ENTRY`. QEMU starts
// the CPU in 32-bit protected mode with paging disabled at that address,
// with EBX pointing at a `hvm_start_info` struct this walking skeleton
// doesn't need to read yet.
global_asm!(
    r#"
    .section .note.pvh, "a"
    .align 4
    pvh_note_start:
        .long 4                        // namesz
        .long 4                        // descsz
        .long 0x12                     // type: XEN_ELFNOTE_PHYS32_ENTRY
        .ascii "Xen\0"                 // name (4 bytes, already aligned)
        .long _start                   // desc: 32-bit physical entry address
    pvh_note_end:
    "#
);

global_asm!(
    r#"
    .code32
    .section .boot, "ax"
    .global _start
    .type _start, @function
_start:
        mov $boot_stack_top, %esp

        // Zero the temporary page-table region (PML4, one PDPT, one PD).
        mov $boot_pml4, %edi
        mov $3072, %ecx
        xor %eax, %eax
        rep stosl

        // PML4[0] -> PDPT (present, writable)
        mov $boot_pdpt, %eax
        or $0x3, %eax
        mov %eax, boot_pml4

        // PDPT[0] -> PD (present, writable)
        mov $boot_pd, %eax
        or $0x3, %eax
        mov %eax, boot_pdpt

        // PD[0..512] -> 2MiB huge pages covering the first 1GiB, identity mapped.
        mov $0, %ecx
    fill_pd:
        mov $0x200000, %eax
        mul %ecx
        or $0x83, %eax
        mov %eax, boot_pd(,%ecx,8)
        inc %ecx
        cmp $512, %ecx
        jne fill_pd

        // Load CR3 with the PML4 physical address.
        mov $boot_pml4, %eax
        mov %eax, %cr3

        // Enable PAE (CR4 bit 5).
        mov %cr4, %eax
        or $0x20, %eax
        mov %eax, %cr4

        // Set LME (long-mode enable) in EFER (MSR 0xC0000080, bit 8).
        mov $0xC0000080, %ecx
        rdmsr
        or $0x100, %eax
        wrmsr

        // Enable paging (CR0 bit 31). We are now in IA-32e compatibility mode.
        mov %cr0, %eax
        or $0x80000000, %eax
        mov %eax, %cr0

        lgdt boot_gdt_pointer
        ljmp $0x08, $long_mode_entry

    .align 16
    boot_gdt_start:
        .quad 0
        .quad 0x00209A0000000000
        .quad 0x0000920000000000
    boot_gdt_end:
    boot_gdt_pointer:
        .word boot_gdt_end - boot_gdt_start - 1
        .long boot_gdt_start

    .code64
    long_mode_entry:
        mov $0x10, %ax
        mov %ax, %ds
        mov %ax, %es
        mov %ax, %ss
        mov %ax, %fs
        mov %ax, %gs
        mov $boot_stack_top, %rsp
        mov %ebx, %edi
        call kernel_main
    .hang:
        hlt
        jmp .hang

    .section .bss, "aw", @nobits
    .align 4096
    boot_pml4:
        .skip 4096
    boot_pdpt:
        .skip 4096
    boot_pd:
        .skip 4096
    .align 16
    boot_stack_bottom:
        // 65536 (64KiB) sufficed through STORY-P0-02-02, but
        // STORY-P0-05-02's page-table construction routinely passes
        // 4096-byte `PageTable` values by value through several call
        // layers (`AddressSpace::map_section` -> `paging::map_4k` ->
        // `walk_create` -> `FrameAllocator::allocate_frame` ->
        // `Pool::alloc`), and this workspace's unoptimized dev-profile
        // build doesn't reuse those temporaries' stack slots across call
        // layers the way an optimized build would — 64KiB triple-faulted
        // (`TEST-P0-05-02-A`'s fixture: RSP wrapped to 0, `#PF` on push)
        // well before `kernel_main` returned. 1MiB has wide headroom
        // above the deepest observed chain without materially changing
        // this walking skeleton's memory footprint.
        .skip 1048576
    boot_stack_top:
    "#,
    options(att_syntax)
);
