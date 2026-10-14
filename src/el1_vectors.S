.section .text.el1_vectors, "ax"
// AArch64 の仕様: 2048バイト境界、16入口、各128バイト。
.balign 2048
.global el1_vectors
el1_vectors:
    .set el1_vector_id, 0
    .rept 16
        .if el1_vector_id == 8
            // EL0 の同期例外は状態を保存し、SVC からの復帰に備える。
            sub sp, sp, #272
            stp x0, x1, [sp, #0]
            mov x0, #el1_vector_id
            b el1_saved_entry
        .else
            mov x0, #el1_vector_id
            b el1_exception_entry
        .endif
        .balign 128
        .set el1_vector_id, el1_vector_id + 1
    .endr

el1_exception_entry:
    msr daifset, #0xf
    // 未対応の例外は致命的。元の状態を破棄して EL1 スタックを初期位置に戻す。
    msr spsel, #1
    ldr x1, =__el1_stack_top
    mov sp, x1
    mov x1, xzr
    b el1_exception_handler

el1_saved_entry:
    msr daifset, #0xf
    stp x2, x3, [sp, #16]
    stp x4, x5, [sp, #32]
    stp x6, x7, [sp, #48]
    stp x8, x9, [sp, #64]
    stp x10, x11, [sp, #80]
    stp x12, x13, [sp, #96]
    stp x14, x15, [sp, #112]
    stp x16, x17, [sp, #128]
    stp x18, x19, [sp, #144]
    stp x20, x21, [sp, #160]
    stp x22, x23, [sp, #176]
    stp x24, x25, [sp, #192]
    stp x26, x27, [sp, #208]
    stp x28, x29, [sp, #224]
    stp x30, xzr, [sp, #240]
    mrs x1, ELR_EL1
    mrs x2, SPSR_EL1
    stp x1, x2, [sp, #256]
    mov x1, sp
    bl el1_exception_handler
    // 非0ならアプリ終了用の復帰処理へ分岐。0なら通常の SVC 復帰。
    cbz x0, 1f
    br x0
1:
    ldp x1, x2, [sp, #256]
    msr ELR_EL1, x1
    msr SPSR_EL1, x2
    ldp x2, x3, [sp, #16]
    ldp x4, x5, [sp, #32]
    ldp x6, x7, [sp, #48]
    ldp x8, x9, [sp, #64]
    ldp x10, x11, [sp, #80]
    ldp x12, x13, [sp, #96]
    ldp x14, x15, [sp, #112]
    ldp x16, x17, [sp, #128]
    ldp x18, x19, [sp, #144]
    ldp x20, x21, [sp, #160]
    ldp x22, x23, [sp, #176]
    ldp x24, x25, [sp, #192]
    ldp x26, x27, [sp, #208]
    ldp x28, x29, [sp, #224]
    ldr x30, [sp, #240]
    ldp x0, x1, [sp, #0]
    add sp, sp, #272
    eret
