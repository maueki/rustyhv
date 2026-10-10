// 試験用の取り決め。HVC の即値は0、x0 は操作番号、x1 は整数引数。
// 復帰時の x0 は状態コード、x1 は結果。それ以外のレジスタは保持する。
pub(super) const INCREMENT: u64 = 1;
pub(super) const OK: u64 = 0;
pub(super) const UNSUPPORTED: u64 = 1;

#[cfg(all(feature = "el1-test", not(feature = "exception-test")))]
mod client {
    use core::arch::asm;

    #[derive(Debug, PartialEq, Eq)]
    pub(crate) enum Error {
        UnsupportedOperation,
        UnknownStatus(u64),
    }

    pub(crate) fn increment(value: u64) -> Result<u64, Error> {
        call(super::INCREMENT, value)
    }

    pub(crate) fn call(operation: u64, argument: u64) -> Result<u64, Error> {
        assert!(crate::current_el() == 1);
        let mut status = operation;
        let mut result = argument;
        // SAFETY: 初期化済みの EL1 試験から呼ぶ。整数だけを渡し、ポインタは扱わない。
        // EL2 は x0・x1 以外のレジスタと PSTATE を保存・復元する。
        unsafe {
            asm!("hvc #0", inout("x0") status, inout("x1") result,
                options(nostack, preserves_flags));
        }
        match status {
            super::OK => Ok(result),
            super::UNSUPPORTED => Err(Error::UnsupportedOperation),
            other => Err(Error::UnknownStatus(other)),
        }
    }
}

#[cfg(all(feature = "el1-test", not(feature = "exception-test")))]
pub(super) use client::{Error, call, increment};
