.section .text.vectors, "ax"
// 2048バイト境界、16個の入口、各128バイトは AArch64 の仕様。
// QEMU virt 固有の値ではない。
.balign 2048
.global exception_vectors
exception_vectors:
    // Four groups of four entries; each entry occupies 128 bytes.
    // All exceptions are fatal in this first implementation.
    .set vector_id, 0
    .rept 16
        // x0 is the first argument in the C calling convention.
        mov x0, #vector_id
        b exception_entry
        .balign 128
        .set vector_id, vector_id + 1
    .endr

exception_entry:
    msr daifset, #0xf
    // No return: discard the interrupted context and use a fresh stack.
    msr spsel, #1
    ldr x1, =__stack_top
    mov sp, x1
    b exception_handler
