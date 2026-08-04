/* Linker script for the TinyOS AArch64 bring-up image (STORY-P1-07-01).
 *
 * Target: Raspberry Pi 5 / BCM2712, debug UART only. This is not a HAL port —
 * FEAT-P1-07 §6 is the scope boundary, and this script exists to place one
 * boot stub and one UART driver, nothing more.
 *
 * LOAD ADDRESS. 0x80000 is where the Raspberry Pi firmware places and enters a
 * bare-metal AArch64 image — *provided config.txt contains `os_check=0`*.
 * Without it the Pi 5 firmware concludes the image is a Linux kernel and loads
 * it at 0x200000 instead. That divergence from every Pi 4 tutorial presents as
 * total silence, not as an error, which is why it is written here rather than
 * assumed. `hal_arm64::board::KERNEL_LOAD_ADDRESS` carries the same constant
 * and a test pins the two together.
 *
 * ENTRY FIRST. The firmware jumps to the first byte of the loaded image, so
 * .text.boot must be the first thing in the file — an ENTRY() directive alone
 * would not save us, because the image is flattened with objcopy and the ELF
 * entry point is discarded on the way.
 *
 * ALIGNMENT IS NOT COSMETIC HERE. Until STORY-P1-07-03 enables the MMU, every
 * access on this board is Device-nGnRnE, and unaligned accesses to Device
 * memory take an alignment fault — on a board that, until STORY-P1-07-02, has
 * no vector table to report it. That is why the target spec keeps
 * `+strict-align` and why __bss_start/__bss_end are 16-byte aligned: the
 * zeroing loop writes 16 bytes at a time and must never straddle.
 *
 * NO .eh_frame, NO unwinding. panic-strategy is abort; anything the compiler
 * emits for unwinding is dead weight in an 8MB image ceiling (G-DX-8).
 */
ENTRY(_start)

/* 64 KiB. One core, no tasks, no allocator, no recursion in this slice — the
 * deepest call chain is _start -> boot_main -> Pl011::write_str. Sized for
 * headroom rather than measured, and deliberately over-provisioned. The 4 KiB
 * guard page below it (__stack_guard) is reserved here and left unmapped by
 * STORY-P1-07-03's identity map, so once the MMU is on a stack overflow
 * reports through STORY-P1-07-02's handler instead of silently eating .bss.
 * Between _start and the MMU enable the guard does not bite — that window is
 * short and allocation-free by construction. */
__stack_size = 0x10000;

SECTIONS
{
    . = 0x80000;

    __image_start = .;

    /* The firmware jumps here. Must be first. */
    .text.boot : ALIGN(8)
    {
        KEEP(*(.text.boot))
        KEEP(*(.text.boot.*))
    }

    .text : ALIGN(8)
    {
        *(.text .text.*)
    }

    .rodata : ALIGN(8)
    {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(8)
    {
        *(.data .data.*)
    }

    /* Zeroed by the boot stub before any Rust code runs. The 16-byte alignment
     * of both bounds is what makes the stp-based zeroing loop total: it can
     * never write past __bss_end and never take an unaligned Device-memory
     * fault. */
    .bss : ALIGN(16)
    {
        __bss_start = .;
        *(COMMON)
        *(.bss .bss.*)
        . = ALIGN(16);
        __bss_end = .;
    }

    /* The guard hole: one page, 4 KiB-aligned, between .bss and the stack.
     * Reserved in the address map but never mapped — see mmu.rs. */
    .stack_guard (NOLOAD) : ALIGN(0x1000)
    {
        __stack_guard = .;
        . += 0x1000;
    }

    .stack (NOLOAD) : ALIGN(16)
    {
        . += __stack_size;
        __stack_top = .;
    }

    __image_end = .;

    /DISCARD/ :
    {
        *(.comment)
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.note.*)
    }
}
