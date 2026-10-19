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
extern "C" fn el1_exception_handler(vector: u64, frame: *mut crate::ExceptionFrame) -> u64 {
    let esr: u64;
    let elr: u64;
    // SAFETY: EL1 の例外ベクタから呼ぶ。出力のために EL2 を呼ぶ前に原因を読む。
    unsafe {
        asm!("mrs {esr}, ESR_EL1", "mrs {elr}, ELR_EL1",
            esr = out(reg) esr, elr = out(reg) elr,
            options(nomem, nostack, preserves_flags));
    }
    #[cfg(feature = "shell")]
    if vector == 8 {
        assert!(!frame.is_null());
        // SAFETY: 下位 EL の同期例外入口が272バイトの保存領域を渡す。
        // ネストする HVC は EL2 の別スタックを使い、この領域の参照は例外復帰・アプリ終了前に終了する。
        let frame = unsafe { &mut *frame };
        // EL0t からの同期例外だけをアプリの障害として回収する。
        // EL1 自身の例外にはこの復帰経路を使わない。
        if frame.spsr & 0xf == 0 {
            if esr >> 26 != 0x15 || esr & 0xffff != 0 {
                report("APP FAULT at EL0\r\n", vector, esr, elr);
                frame.registers[0] = crate::el0::FAULT_STATUS;
                frame.registers[1] = crate::el0::FAULT_BIT | esr;
                return crate::el0::exit_address();
            }
            match frame.registers[0] {
                crate::svc_abi::WRITE_BYTE => {
                    // SVC の1バイト出力。
                    let status = match u8::try_from(frame.registers[1]) {
                        Ok(byte) => match hvc::write_bytes(&[byte]) {
                            Ok(()) => 0,
                            Err(_) => 3, // コンソール出力エラー。
                        },
                        Err(_) => 2, // 不正引数。
                    };
                    frame.registers[0] = status;
                    frame.registers[1] = 0;
                }
                crate::svc_abi::EXIT => {
                    // SVC の終了要求。
                    // 終了コードを保存し、eret ではなく EL1 の呼び出し元へ戻す。
                    frame.registers[0] = frame.registers[1];
                    frame.registers[1] = 0; // 通常終了。終了コード128でも例外とは区別する。
                    return crate::el0::exit_address();
                }
                crate::svc_abi::READ_BYTE => {
                    // 引数は0だけを許可。待機しても EL1 の例外フレームは保持される。
                    let (status, byte) = if frame.registers[1] != 0 {
                        (2, 0)
                    } else {
                        match hvc::read_byte() {
                            Ok(byte) => (0, u64::from(byte)),
                            Err(hvc::Error::ReceiveFault) => (4, 0),
                            Err(_) => (3, 0),
                        }
                    };
                    frame.registers[0] = status;
                    frame.registers[1] = byte;
                }
                _ => {
                    frame.registers[0] = 1; // 未対応の操作。
                    frame.registers[1] = 0;
                }
            }
            return 0; // SVC の次の命令へ eret で戻る。
        }
    }
    #[cfg(not(feature = "shell"))]
    let _ = frame;
    report("EXCEPTION at EL1\r\n", vector, esr, elr);
    crate::halt()
}

fn report(heading: &str, vector: u64, esr: u64, elr: u64) {
    hvc::write_str(heading).expect("console failed");
    hvc::write_str("Vector offset: ").expect("console failed");
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
    if matches!(esr >> 26, 0x20 | 0x21 | 0x24 | 0x25) {
        let far: u64;
        // SAFETY: 命令・データアボートの診断として、EL1 の障害アドレスを読む。
        unsafe {
            asm!("mrs {far}, FAR_EL1", far = out(reg) far,
                options(nomem, nostack, preserves_flags));
        }
        hvc::write_str("FAR_EL1: ").expect("console failed");
        hvc::write_bytes(&crate::hex_bytes(far)).expect("console failed");
        hvc::write_str("\r\n").expect("console failed");
    }
}
