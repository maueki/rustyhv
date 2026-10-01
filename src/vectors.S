.section .text.vectors, "ax"
.balign 2048
.global exception_vectors
exception_vectors:
    // Four groups of four entries; each entry occupies 128 bytes.
    // All exceptions are fatal in this first implementation.
    .rept 16
        b exception_entry
        .balign 128
    .endr

exception_entry:
    msr daifset, #0xf
    // No return: discard the interrupted context and use a fresh stack.
    msr spsel, #1
    ldr x0, =__stack_top
    mov sp, x0
    b exception_handler
