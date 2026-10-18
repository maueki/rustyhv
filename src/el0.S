.section .text.el0, "ax"
.global run_el0
run_el0:
    // C 呼び出し規約で保持する x19〜x30 と DAIF を、EL1 のスタックへ保存する。
    sub sp, sp, #112
    stp x19, x20, [sp, #0]
    stp x21, x22, [sp, #16]
    stp x23, x24, [sp, #32]
    stp x25, x26, [sp, #48]
    stp x27, x28, [sp, #64]
    stp x29, x30, [sp, #80]
    mrs x2, DAIF
    str x2, [sp, #96]
    msr daifset, #0xf
    msr SP_EL0, x1
    msr ELR_EL1, x0
    // DAIF 全マスク・EL0t（SP_EL0 使用）。AArch64 の仕様。
    mov x2, #0x3c0
    msr SPSR_EL1, x2
    isb
    eret

.global el1_app_exit
el1_app_exit:
    // EL0 の例外フレームを解放し、x0 に終了コードを取り出す。
    ldr x0, [sp, #0]
    add sp, sp, #272
    ldr x2, [sp, #96]
    msr DAIF, x2
    ldp x19, x20, [sp, #0]
    ldp x21, x22, [sp, #16]
    ldp x23, x24, [sp, #32]
    ldp x25, x26, [sp, #48]
    ldp x27, x28, [sp, #64]
    ldp x29, x30, [sp, #80]
    add sp, sp, #112
    ret
