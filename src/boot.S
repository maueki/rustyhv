.section .text.boot, "ax"
.global _start
_start:
    // QEMU の ELF 直接起動で単一CPU・MMU無効を前提とする。
    // 実機ではブートローダーとの取り決めで、この初期状態を確認する。
    // DAIF のマスク値と SPSel の値は AArch64 の仕様。
    msr daifset, #0xf
    msr spsel, #1
    ldr x0, =__stack_top
    mov sp, x0

    // Clear BSS before entering Rust. Both bounds are 16-byte aligned.
    ldr x0, =__bss_start
    ldr x1, =__bss_end
0:
    cmp x0, x1
    b.hs 1f
    str xzr, [x0], #8
    b 0b
1:
    bl kernel_main
2:
    wfe
    b 2b
