use core::arch::asm;

// 組み込み EL0 アプリの試験用 SVC 規約。整数だけを渡し、ポインタは渡さない。
pub(super) const WRITE_BYTE: u64 = 1;
pub(super) const EXIT: u64 = 2;

#[derive(Debug)]
pub(crate) struct Error(pub(crate) u64);

pub(crate) fn write_str(text: &str) -> Result<(), Error> {
    for byte in text.bytes() {
        let mut status = WRITE_BYTE;
        // SAFETY: EL1 のベクタが設置済みの EL0 アプリから呼ぶ。
        // EL1 は x0・x1 以外のレジスタと PSTATE を保存・復元する。
        unsafe {
            asm!("svc #0", inout("x0") status, inout("x1") u64::from(byte) => _,
                options(nostack, preserves_flags));
        }
        if status != 0 {
            return Err(Error(status));
        }
    }
    Ok(())
}

pub(super) fn exit(status: u64) -> ! {
    // SAFETY: 実行中の EL0 アプリを終了する。EL1 は保存したシェルの呼び出し元へ戻す。
    unsafe {
        asm!("svc #0", in("x0") EXIT, in("x1") status, options(noreturn, nostack));
    }
}
