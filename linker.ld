ENTRY(_start)

SECTIONS
{
    /* QEMU virt 固有: RAM は0x40000000から。現在の起動設定では128 MiB。
       0x40080000はrustyhvが選んだ配置先。先頭512 KiBを起動データ用に空ける。
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
    .el1_stack (NOLOAD) : ALIGN(16)
    {
        /* EL1 の試験用に別の64 KiBスタックを確保する。容量は今回の選択。 */
        . += 64K;
        __el1_stack_top = .;
    }
    ASSERT(. <= 0x40200000, "kernel exceeds private 2 MiB region")
    . = 0x40200000;
    .user_image : ALIGN(4096) { KEEP(*(.user_image)) }
    ASSERT(. <= 0x403ff000, "user image overlaps stack guard")
    . = 0x40400000;
    .el0_stack (NOLOAD) : ALIGN(4096)
    {
        /* 組み込みアプリ用の64 KiB。EL1 のシェルとは別のスタックを使う。 */
        . += 64K;
        __el0_stack_top = .;
    }
}
