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

// `LE-102`. Both blocks below are gated on `target_os = "none"` — the
// bare-metal condition itself — rather than on the module's own
// `not(target_os = "windows")`, and the difference is not cosmetic: this file
// defines `_start`, and a **Linux** host satisfies not-Windows exactly as the
// `x86_64-tinyos` target does. When `LE-100` put `cargo test --workspace` on
// the Linux runner, this `_start` was compiled into the `hal-x86_64` rlib and
// collided with `Scrt1.o`'s `_start` in every `std` test harness linking it;
// `hal-x86_64`, `kernel`, `exec` and `hal-arm64` all died at the linker and
// not one host test in the workspace ran.
//
// The gate is HERE and not on `pub mod boot` in `lib.rs`, deliberately: the
// module must keep existing for an ELF-native host, because `kernel`'s own
// `[[bin]]` writes `use hal_x86_64::boot as _;` ungated and the Linux
// governance job's `clippy --workspace --all-targets` compiles that bin. Empty
// on a host, present on the target, is the only shape that satisfies both —
// and nothing is lost, because assembling this block on a host never proved
// anything the real `check-guest-images` build does not.
//
// `core::arch::global_asm!` is spelled in full rather than imported: a `use`
// at the top of a module whose only consumers are cfg'd out is an unused
// import, and this workspace builds with `-D warnings`.

// QEMU's built-in `-kernel` direct-boot loader (hw/i386/multiboot.c) only
// understands the Multiboot **1** header (32-bit ELF only) and Linux bzImage
// — neither fits a 64-bit ELF built from a `no_std` Rust target. The Xen PVH
// boot protocol is QEMU's direct-boot path for exactly this case (a raw
// ELF64 kernel, no GRUB/bootloader in between): an ELF note advertising a
// 32-bit physical entry point, per `XEN_ELFNOTE_PHYS32_ENTRY`. QEMU starts
// the CPU in 32-bit protected mode with paging disabled at that address,
// with EBX pointing at a `hvm_start_info` struct this walking skeleton
// doesn't need to read yet.
#[cfg(target_os = "none")]
core::arch::global_asm!(
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

#[cfg(target_os = "none")]
core::arch::global_asm!(
    r#"
    .code32
    .section .boot, "ax"
    .global _start
    .type _start, @function
_start:
        mov $boot_stack_top, %esp

        // Zero the temporary page-table region (PML4, one PDPT, two PDs).
        mov $boot_pml4, %edi
        mov $4096, %ecx
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

        // PDPT[3] -> a second PD, covering the 1GiB at physical
        // 0xC0000000-0xFFFFFFFF (present, writable) — see
        // `boot_pd_gib3`'s own comment for why this range specifically.
        mov $boot_pd_gib3, %eax
        or $0x3, %eax
        mov %eax, boot_pdpt+24

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

        // boot_pd_gib3[0..512] -> 2MiB huge pages covering
        // 0xC0000000-0xFFFFFFFF, identity mapped — see its own `.bss`
        // comment for why.
        mov $0, %ecx
    fill_pd_gib3:
        mov $0x200000, %eax
        mul %ecx
        add $0xC0000000, %eax
        or $0x83, %eax
        mov %eax, boot_pd_gib3(,%ecx,8)
        inc %ecx
        cmp $512, %ecx
        jne fill_pd_gib3

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

        // Enable SSE before calling any Rust code (`STORY-P1-01-01`).
        //
        // Not an optimization — a correctness requirement, and the *only*
        // reason this exists is that `STORY-P1-01-01`'s Tier 0 measurement
        // fixture triple-faulted without it. SSE2 is architecturally
        // guaranteed on every x86_64 CPU, so LLVM freely emits `movups`/
        // `movaps` for ordinary 16-byte struct copies in perfectly
        // float-free code — `kernel::sched::Scheduler::highest_priority_ready`
        // compiles to exactly that. But an SSE instruction executed while
        // `CR4.OSFXSR` is clear raises `#UD`, and with no IDT installed on a
        // fixture boot path that escalates `#UD` -> `#GP` -> `#DF` -> triple
        // fault -> silent QEMU shutdown. That was observed directly (QEMU
        // `-d int,cpu_reset`: `v=06` at the `movups` in
        // `highest_priority_ready`, then `v=0d`, then `v=08`), which means
        // that scheduler function could not execute on the real target
        // binary at all before this change, despite passing its host tests.
        //
        // The alternative fix — adding `-sse,-mmx,+soft-float` to
        // `targets/x86_64-tinyos.json`, as the upstream
        // `x86_64-unknown-none` target does — was rejected: it makes the
        // compiler avoid the vector unit everywhere, which is the wrong
        // trade for a kernel whose own standards put maximum throughput at
        // priority 4 and which expects to host local inference workloads.
        // Enabling the unit the hardware guarantees is the honest fix; see
        // `docs/adr/0003-enable-sse-in-the-boot-path.md`.
        //
        // CR0: clear EM (bit 2) so SSE/x87 instructions are not trapped as
        // emulated, set MP (bit 1) so `FWAIT`/`WAIT` honors TS as the SDM
        // specifies for a machine with a real FPU.
        mov %cr0, %rax
        and $0xFFFFFFFFFFFFFFFB, %rax
        or $0x2, %rax
        mov %rax, %cr0
        // CR4: set OSFXSR (bit 9) — the OS declares it supports `FXSAVE`/
        // `FXRSTOR`-style state management, which is what actually permits
        // SSE instructions — and OSXMMEXCPT (bit 10) so an unmasked SIMD
        // floating-point error raises `#XF` (vector 19) rather than the
        // ambiguous `#UD` this code path exists to eliminate.
        mov %cr4, %rax
        or $0x600, %rax
        mov %rax, %cr4

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
    boot_pd_gib3:
        // Identity-maps 0xC0000000-0xFFFFFFFF (`STORY-P0-04-02`) so
        // `hal_x86_64::interrupts` can reach the local APIC's real,
        // non-relocated MMIO window at its architectural default,
        // 0xFEE00000 (Intel SDM Vol 3A §11.4.1) — QEMU's own APIC device
        // model does not honor `IA32_APIC_BASE`'s relocation field (proven
        // empirically during this Story's bring-up: writing a relocated
        // base and then reading any local-APIC register back, including
        // the read-only APIC ID register, returned all-zero, meaning the
        // access was landing on ordinary backing memory rather than being
        // intercepted by the APIC), so mapping the *default* address is
        // the only path that actually reaches real hardware under this
        // project's Tier 0 target, not just a simplification.
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
