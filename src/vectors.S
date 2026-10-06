.section .text.vectors, "ax"
// 2048バイト境界、16個の入口、各128バイトは AArch64 の仕様。
// QEMU virt 固有の値ではない。
.balign 2048
.global exception_vectors
exception_vectors:
    // 現在の EL・SP_ELx の IRQ と、下位 EL の AArch64 同期例外は状態を保存する。
    .set vector_id, 0
    .rept 16
        .if (vector_id == 5) || (vector_id == 8)
            // x0・x1 を変更する前に保存。272バイトで16バイト境界を維持する。
            sub sp, sp, #272
            stp x0, x1, [sp, #0]
            mov x0, #vector_id
            b saved_exception_entry
        .else
            // 致命的な例外では元の処理へ戻らず、番号を第1引数に渡す。
            mov x0, #vector_id
            b exception_entry
        .endif
        .balign 128
        .set vector_id, vector_id + 1
    .endr

exception_entry:
    msr daifset, #0xf
    // 致命的な例外は復帰しない。元の状態を破棄してスタックを初期位置へ戻す。
    msr spsel, #1
    ldr x1, =__stack_top
    mov sp, x1
    b exception_handler

// AArch64 の例外復帰処理。QEMU 固有の操作はない。
// softfloat ターゲットを使うため、FP/SIMD レジスタは今回の保存対象に含めない。
saved_exception_entry:
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
    mrs x1, ELR_EL2
    mrs x2, SPSR_EL2
    stp x1, x2, [sp, #256]
    bl exception_handler

    // 復帰先と元の PSTATE を戻す。DAIF と NZCV は eret が復元する。
    ldp x1, x2, [sp, #256]
    msr ELR_EL2, x1
    msr SPSR_EL2, x2
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
