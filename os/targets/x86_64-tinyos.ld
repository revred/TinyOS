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

    .boot : ALIGN(4K)
    {
        KEEP(*(.boot))
        KEEP(*(.boot.*))
    } :text

    .text : ALIGN(4K)
    {
        *(.text .text.*)
    } :text

    .rodata : ALIGN(4K)
    {
        *(.rodata .rodata.*)
    } :text

    .data : ALIGN(4K)
    {
        *(.data .data.*)
    } :text

    .bss : ALIGN(4K)
    {
        *(COMMON)
        *(.bss .bss.*)
    } :text

    /DISCARD/ :
    {
        *(.comment)
        *(.eh_frame)
    }
}
