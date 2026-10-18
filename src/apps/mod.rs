mod fault;
mod hello;

pub(super) enum RunError {
    NotFound,
    ExitStatus(u64),
}

pub(super) fn run(name: &str) -> Result<(), RunError> {
    // 組み込みアプリを EL0 で実行し、終了 SVC または同期例外でシェルを再開する。
    let entry = match name {
        "hello" => hello_entry,
        "fault" => fault::run,
        _ => return Err(RunError::NotFound),
    };
    match crate::el0::run(entry) {
        0 => Ok(()),
        status => Err(RunError::ExitStatus(status)),
    }
}

extern "C" fn hello_entry() -> ! {
    let status = match hello::run() {
        Ok(()) => 0,
        Err(error) => error.0,
    };
    crate::app_api::exit(status)
}
