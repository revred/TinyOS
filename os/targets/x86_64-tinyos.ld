/* Linker script for the TinyOS x86_64 kernel walking skeleton.
 * Identity-mapped (no higher-half split yet — that lands with FEAT-P0-03's
 * memory pool / virtual memory work). Boots via the Xen PVH direct-boot
 * protocol (see boot.rs) rather than Multiboot/GRUB — the `.note.pvh`
 * section must land inside a PT_NOTE program header for QEMU to find it.
 */
ENTRY(_start)

PHDRS
{
    text PT_LOAD FLAGS(5);   /* R+X */
    note PT_NOTE FLAGS(4);   /* R   */
}

SECTIONS
{
    . = 1M;

    .note.pvh : ALIGN(4)
    {
        KEEP(*(.note.pvh))
    } :note :text

    /* Section-boundary symbols (STORY-P1-03-02): the W^X kernel page
     * tables are built from these at run time (exec::kernel_map), so the
     * permission map can never drift from the layout the linker actually
     * produced. .boot is deliberately inside the executable range — its
     * mode-transition code is fetched exactly like .text is. */
    .boot : ALIGN(4K)
    {
        __kernel_exec_start = .;
        KEEP(*(.boot))
        KEEP(*(.boot.*))
    } :text

    .text : ALIGN(4K)
    {
        *(.text .text.*)
        __kernel_exec_end = .;
    } :text

    .rodata : ALIGN(4K)
    {
        __kernel_rodata_start = .;
        *(.rodata .rodata.*)
        __kernel_rodata_end = .;
    } :text

    .data : ALIGN(4K)
    {
        *(.data .data.*)
    } :text

    .bss : ALIGN(4K)
    {
        *(COMMON)
        *(.bss .bss.*)
        __kernel_image_end = .;
    } :text

    /DISCARD/ :
    {
        *(.comment)
        *(.eh_frame)
    }
}
