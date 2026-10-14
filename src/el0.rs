use core::arch::global_asm;

global_asm!(include_str!("el0.S"));

pub(super) fn exit_address() -> u64 {
    unsafe extern "C" {
        fn el1_app_exit();
    }
    el1_app_exit as *const () as u64
}

pub(super) fn run(entry: extern "C" fn() -> !) -> u64 {
    unsafe extern "C" {
        fn run_el0(entry: extern "C" fn() -> !, stack: *const u8) -> u64;
        static __el0_stack_top: u8;
    }
    // SAFETY: シェルの EL1h から同期的に呼ぶ。EL1 のベクタは設置済み。
    // 専用スタックは16バイト境界。終了 SVC で呼び出し元の SP・callee-saved レジスタを戻す。
    unsafe { run_el0(entry, core::ptr::addr_of!(__el0_stack_top)) }
}
