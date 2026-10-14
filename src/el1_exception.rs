use crate::hvc;
use core::arch::{asm, global_asm};

global_asm!(include_str!("el1_vectors.S"));

pub(super) fn install() {
    unsafe extern "C" {
        static el1_vectors: u8;
    }
    // SAFETY: EL1 から呼ぶ。表は2048バイト境界で、VBAR_EL1 の変更後に同期する。
    unsafe {
        asm!("msr VBAR_EL1, {table}", "isb",
            table = in(reg) core::ptr::addr_of!(el1_vectors),
            options(nostack, preserves_flags));
    }
}

#[cfg(feature = "el1-exception-test")]
pub(super) fn test() {
    hvc::write_str("Testing BRK at EL1\r\n").expect("console failed");
    // SAFETY: EL1 のベクタとスタックは準備済み。意図した同期例外で停止を確認する。
    unsafe { asm!("brk #0", options(noreturn)) };
}

#[unsafe(no_mangle)]
extern "C" fn el1_exception_handler(vector: u64) -> ! {
    let esr: u64;
    let elr: u64;
    // SAFETY: EL1 の例外ベクタから呼ぶ。出力のために EL2 を呼ぶ前に原因を読む。
    unsafe {
        asm!("mrs {esr}, ESR_EL1", "mrs {elr}, ELR_EL1",
            esr = out(reg) esr, elr = out(reg) elr,
            options(nomem, nostack, preserves_flags));
    }
    hvc::write_str("EXCEPTION at EL1\r\nVector offset: ").expect("console failed");
    hvc::write_bytes(&crate::hex_bytes(vector * 128)).expect("console failed");
    hvc::write_str("\r\nKind: ").expect("console failed");
    hvc::write_str(match vector & 3 {
        0 => "Synchronous\r\n",
        1 => "IRQ\r\n",
        2 => "FIQ\r\n",
        _ => "SError\r\n",
    })
    .expect("console failed");
    if vector & 3 == 0 {
        hvc::write_str("ESR_EL1: ").expect("console failed");
        hvc::write_bytes(&crate::hex_bytes(esr)).expect("console failed");
        hvc::write_str("\r\n").expect("console failed");
    }
    hvc::write_str("ELR_EL1: ").expect("console failed");
    hvc::write_bytes(&crate::hex_bytes(elr)).expect("console failed");
    hvc::write_str("\r\n").expect("console failed");
    crate::halt()
}
