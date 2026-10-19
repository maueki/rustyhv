#![no_std]
#![no_main]

#[path = "../src/app_api.rs"]
mod app_api;
#[path = "../src/apps/fault.rs"]
mod fault;
#[path = "../src/apps/globals.rs"]
mod globals;
#[path = "../src/apps/hello.rs"]
mod hello;
#[path = "../src/apps/input.rs"]
mod input;
#[path = "../src/svc_abi.rs"]
mod svc_abi;

// OS と別にリンクし、ユーザー用の領域だけで実行が完結するイメージを作る。
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
extern "C" fn user_entry(selector: u64) -> ! {
    let status = match selector {
        0 => match hello::run() {
            Ok(()) => 0,
            Err(error) => error.0,
        },
        1 => fault::run(),
        2 => {
            // 保護検証用。Rust の参照を作らず、OS のコード領域を直接ロードする。
            unsafe {
                core::arch::asm!("ldr {value}, [{address}]", value = out(reg) _,
                    address = in(reg) 0x4008_0000u64, options(nostack, preserves_flags));
            }
            0 // ここへ来たら保護に失敗している。
        }
        3 => {
            // 保護検証用。自分の読み取り専用コード領域への書き込みを試みる。
            unsafe {
                core::arch::asm!("str xzr, [{address}]",
                    address = in(reg) 0x4020_0000u64, options(nostack, preserves_flags));
            }
            0
        }
        4 => {
            // 保護検証用。実行禁止のスタック領域への分岐を試みる。
            unsafe {
                core::arch::asm!("br {address}", address = in(reg) 0x4040_0000u64,
                    options(noreturn, nostack));
            }
        }
        5 => {
            // 保護検証用。OS のコードへの書き込みも拒否されることを確認する。
            unsafe {
                core::arch::asm!("str xzr, [{address}]",
                    address = in(reg) 0x4008_0000u64, options(nostack, preserves_flags));
            }
            0
        }
        6 => {
            // 保護検証用。UART は EL1 / EL2 だけが操作する。
            unsafe {
                core::arch::asm!("ldr {value}, [{address}]", value = out(reg) _,
                    address = in(reg) 0x0900_0000u64, options(nostack, preserves_flags));
            }
            0
        }
        7 => {
            // 保護検証用。下へ伸びるスタックの直前のページは未マップ。
            unsafe {
                core::arch::asm!("str xzr, [{address}]",
                    address = in(reg) 0x403f_f000u64, options(nostack, preserves_flags));
            }
            0
        }
        8 => match globals::run() {
            Ok(()) => 0,
            Err(error) => error.0,
        },
        9 => {
            if let Err(error) = globals::run() {
                app_api::exit(error.0);
            }
            app_api::abort()
        }
        10 => {
            // 保護検証用。書き込み可能なデータ領域からの実行は禁止する。
            unsafe {
                core::arch::asm!("br {address}", address = in(reg) 0x4060_0000u64,
                    options(noreturn, nostack));
            }
        }
        11 => match input::run() {
            Ok(()) => 0,
            Err(error) => error.0,
        },
        _ => 1,
    };
    app_api::exit(status)
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    // EL0 の panic は OS の panic 処理を呼ばず、プロセスの異常終了として返す。
    app_api::exit(255)
}
