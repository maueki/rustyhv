ENTRY(_start)

SECTIONS
{
    /* QEMU virt RAM starts at 0x40000000. Leave space for boot data. */
    . = 0x40080000;
    .text : ALIGN(16) { KEEP(*(.text.boot)) *(.text .text.*) }
    .rodata : ALIGN(16) { *(.rodata .rodata.*) }
    .data : ALIGN(16) { *(.data .data.*) }
    .bss (NOLOAD) : ALIGN(16)
    {
        __bss_start = .;
        *(.bss .bss.*) *(COMMON)
        . = ALIGN(16);
        __bss_end = .;
    }
    .stack (NOLOAD) : ALIGN(16)
    {
        . += 64K;
        __stack_top = .;
    }
}
