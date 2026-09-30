.section .text.boot, "ax"
.global _start
_start:
    // Single CPU, MMU off; keep exceptions masked until vectors exist.
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
