// 試験用の取り決め。HVC の即値は0、x0 は操作番号、x1 は整数引数。
// 復帰時の x0 は状態コード、x1 は結果。それ以外のレジスタは保持する。
pub(super) const INCREMENT: u64 = 1;
pub(super) const CONSOLE_BYTE: u64 = 2;
pub(super) const CONSOLE_READ: u64 = 3;
pub(super) const CONSOLE_WAIT: u64 = 4;
pub(super) const OK: u64 = 0;
pub(super) const UNSUPPORTED: u64 = 1;
pub(super) const INVALID_ARGUMENT: u64 = 2;
pub(super) const WOULD_BLOCK: u64 = 3;
pub(super) const INPUT_ERROR: u64 = 4;

#[cfg(all(feature = "el1-test", not(feature = "exception-test")))]
mod client {
    use core::arch::asm;

    #[derive(Debug, PartialEq, Eq)]
    pub(crate) enum Error {
        UnsupportedOperation,
        InvalidArgument,
        WouldBlock,
        ReceiveFault,
        UnknownStatus(u64),
    }

    #[cfg(not(feature = "shell"))]
    pub(crate) fn increment(value: u64) -> Result<u64, Error> {
        call(super::INCREMENT, value)
    }

    pub(crate) fn write_str(text: &str) -> Result<(), Error> {
        write_bytes(text.as_bytes())
    }

    pub(crate) fn write_bytes(bytes: &[u8]) -> Result<(), Error> {
        // 各バイトを整数で渡す。EL2 は EL1 のポインタを参照しない。
        // 途中で失敗した場合、それ以前のバイトは出力済み。
        for &byte in bytes {
            call(super::CONSOLE_BYTE, u64::from(byte))?;
        }
        Ok(())
    }

    #[cfg(feature = "shell")]
    pub(crate) fn read_byte() -> Result<u8, Error> {
        // 未入力時は EL2 内で WFI 待機する。呼び出し側に unsafe は不要。
        call(super::CONSOLE_WAIT, 0).map(|value| u8::try_from(value).expect("invalid console byte"))
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
            super::INVALID_ARGUMENT => Err(Error::InvalidArgument),
            super::WOULD_BLOCK => Err(Error::WouldBlock),
            super::INPUT_ERROR => Err(Error::ReceiveFault),
            other => Err(Error::UnknownStatus(other)),
        }
    }
}

#[cfg(all(feature = "el1-test", not(feature = "exception-test")))]
pub(super) use client::{Error, write_bytes, write_str};

#[cfg(all(
    feature = "el1-test",
    not(feature = "exception-test"),
    not(feature = "shell")
))]
pub(super) use client::{call, increment};

#[cfg(all(feature = "shell", not(feature = "exception-test")))]
pub(super) use client::read_byte;
