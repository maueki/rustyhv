use crate::app_api::{self, Error};
use core::sync::atomic::{AtomicU64, Ordering};

// 非ゼロ初期値は .data、ゼロ初期値は .bss に配置される。
static INITIALIZED: AtomicU64 = AtomicU64::new(41);
static ZERO: AtomicU64 = AtomicU64::new(0);

pub(super) fn run() -> Result<(), Error> {
    assert_eq!(INITIALIZED.load(Ordering::Relaxed), 41);
    assert_eq!(ZERO.load(Ordering::Relaxed), 0);
    app_api::write_str("Globals initialized: data=41 bss=0\r\n")?;
    INITIALIZED.store(42, Ordering::Relaxed);
    ZERO.store(1, Ordering::Relaxed);
    assert_eq!(INITIALIZED.load(Ordering::Relaxed), 42);
    assert_eq!(ZERO.load(Ordering::Relaxed), 1);
    app_api::write_str("Globals changed: data=42 bss=1\r\n")
}
