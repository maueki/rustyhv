ENTRY(user_entry)
PHDRS { image PT_LOAD FLAGS(5); data PT_LOAD FLAGS(6); }
SECTIONS
{
    /* QEMU virt 用の固定配置。カーネル側 linker.ld と el0_image.rs に対応する。 */
    . = 0x40200000;
    .text : { KEEP(*(.text.entry)) *(.text .text.*) } :image
    .rodata : { *(.rodata .rodata.*) *(.data.rel.ro .data.rel.ro.*) } :image
    ASSERT(. <= 0x403ff000, "user image overlaps stack guard")
    /* プロセスごとに初期化する書き込み可能な領域。実行は禁止する。 */
    . = 0x40600000;
    .data : { *(.data .data.*) . = ALIGN(16); } :data
    .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) . = ALIGN(16); } :data
    ASSERT(. <= 0x40610000, "user data exceeds 64 KiB")
    /DISCARD/ : { *(.eh_frame*) *(.comment*) }
}
