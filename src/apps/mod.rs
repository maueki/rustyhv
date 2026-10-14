mod hello;

pub(super) enum RunError {
    NotFound,
    ExitStatus(u64),
}

pub(super) fn run(name: &str) -> Result<(), RunError> {
    // 組み込みアプリを EL0 で同期的に実行する。終了 SVC でシェルを再開する。
    match name {
        "hello" => match crate::el0::run(hello_entry) {
            0 => Ok(()),
            status => Err(RunError::ExitStatus(status)),
        },
        _ => Err(RunError::NotFound),
    }
}

extern "C" fn hello_entry() -> ! {
    let status = match hello::run() {
        Ok(()) => 0,
        Err(error) => error.0,
    };
    crate::app_api::exit(status)
}
