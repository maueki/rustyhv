use core::arch::global_asm;

global_asm!(include_str!("el0.S"));

// 同期例外でアプリを終了した場合の予約済み終了コード。
pub(super) const FAULT_STATUS: u64 = 128;
pub(super) const FAULT_BIT: u64 = 1 << 63;

// AAPCS64 の16バイト結果は x0・x1 で返す。例外と通常の終了コードを混同しない。
#[repr(C)]
pub(super) struct Outcome {
    pub status: u64,
    pub reason: u64,
}

pub(super) fn exit_address() -> u64 {
    unsafe extern "C" {
        fn el1_app_exit();
    }
    el1_app_exit as *const () as u64
}

pub(super) fn run(entry: extern "C" fn() -> !) -> Outcome {
    unsafe extern "C" {
        fn run_el0(entry: extern "C" fn() -> !, stack: *const u8) -> Outcome;
        static __el0_stack_top: u8;
    }
    // SAFETY: シェルの EL1h から同期的に呼ぶ。EL1 のベクタは設置済み。
    // 専用スタックは16バイト境界。終了 SVC または同期例外で、呼び出し元の SP・callee-saved レジスタを戻す。
    unsafe { run_el0(entry, core::ptr::addr_of!(__el0_stack_top)) }
}
