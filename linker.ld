ENTRY(_start)

SECTIONS
{
    /* QEMU virt 固有: RAM は0x40000000から。現在の起動設定では128 MiB。
       0x40080000はFerrovisorが選んだ配置先。先頭512 KiBを起動データ用に空ける。
       AArch64の固定入口ではない。QEMUはELFに記録された入口を使用する。 */
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
        /* 64 KiBは今回選んだスタック容量。16バイト境界はAArch64 ABIに合わせる。 */
        . += 64K;
        __stack_top = .;
    }
}
