use core::arch::asm;

// 組み込み EL0 アプリの試験用 SVC 規約。整数だけを渡し、ポインタは渡さない。
use crate::svc_abi::{EXIT, READ_BYTE, WRITE_BYTE};

#[derive(Debug)]
pub(crate) struct Error(pub(crate) u64);

pub(crate) fn write_str(text: &str) -> Result<(), Error> {
    for byte in text.bytes() {
        write_byte(byte)?;
    }
    Ok(())
}

pub(crate) fn write_byte(byte: u8) -> Result<(), Error> {
    call(WRITE_BYTE, u64::from(byte)).map(|_| ())
}

pub(crate) fn read_byte() -> Result<u8, Error> {
    // 端末入力。未入力なら EL2 の WFI で待機する。CRLF の LF は共通入力処理が省く。
    call(READ_BYTE, 0).and_then(|value| u8::try_from(value).map_err(|_| Error(2)))
}

fn call(operation: u64, argument: u64) -> Result<u64, Error> {
    let mut status = operation;
    let mut result = argument;
    // SAFETY: 初期化済みの EL0 から整数だけを渡す。OS にユーザーポインタは渡さない。
    // EL1 は x0・x1 以外のレジスタと PSTATE を保存・復元する。
    unsafe {
        asm!("svc #0", inout("x0") status, inout("x1") result,
            options(nostack, preserves_flags));
    }
    if status == 0 {
        Ok(result)
    } else {
        Err(Error(status))
    }
}

pub(super) fn exit(status: u64) -> ! {
    // SAFETY: 実行中の EL0 アプリを終了する。EL1 は保存したシェルの呼び出し元へ戻す。
    unsafe {
        asm!("svc #0", in("x0") EXIT, in("x1") status, options(noreturn, nostack));
    }
}

pub(crate) fn abort() -> ! {
    // SAFETY: 初期化済みの EL0 アプリで意図的に同期例外を発生させる。
    // EL1 が保存したシェルの状態へ戻すため、アプリの実行は再開しない。
    unsafe { asm!("brk #0", options(noreturn)) };
}
