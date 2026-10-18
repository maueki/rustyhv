ENTRY(user_entry)
PHDRS { image PT_LOAD FLAGS(5); }
SECTIONS
{
    /* QEMU virt 用の固定配置。カーネル側 linker.ld と el0_image.rs に対応する。 */
    . = 0x40200000;
    .text : { KEEP(*(.text.entry)) *(.text .text.*) } :image
    .rodata : { *(.rodata .rodata.*) *(.data.rel.ro .data.rel.ro.*) } :image
    .data : { *(.data .data.*) } :image
    .bss : { *(.bss .bss.*) *(COMMON) } :image
    ASSERT(SIZEOF(.data) == 0 && SIZEOF(.bss) == 0, "user image must be read-only")
    ASSERT(. <= 0x40400000, "user image exceeds 2 MiB")
    /DISCARD/ : { *(.eh_frame*) *(.comment*) }
}
