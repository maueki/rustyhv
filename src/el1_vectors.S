.section .text.el1_vectors, "ax"
// AArch64 の仕様: 2048バイト境界、16入口、各128バイト。
.balign 2048
.global el1_vectors
el1_vectors:
    .set el1_vector_id, 0
    .rept 16
        mov x0, #el1_vector_id
        b el1_exception_entry
        .balign 128
        .set el1_vector_id, el1_vector_id + 1
    .endr

el1_exception_entry:
    msr daifset, #0xf
    // 現段階では全例外が致命的。元の状態を破棄し、EL1 スタックを初期位置に戻す。
    msr spsel, #1
    ldr x1, =__el1_stack_top
    mov sp, x1
    b el1_exception_handler
