#![no_std]
#![no_main]

#[path = "../src/app_api.rs"]
mod app_api;
#[path = "../src/apps/fault.rs"]
mod fault;
#[path = "../src/apps/hello.rs"]
mod hello;
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
        _ => 1,
    };
    app_api::exit(status)
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    // EL0 の panic は OS の panic 処理を呼ばず、プロセスの異常終了として返す。
    app_api::exit(255)
}
