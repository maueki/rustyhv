mod hello;

pub(super) enum RunError {
    NotFound,
    Console(crate::app_api::Error),
}

pub(super) fn run(name: &str) -> Result<(), RunError> {
    // カーネルと一緒にリンクした Rust 関数を EL1 で同期的に呼ぶ。
    // アプリが戻るとシェルを再開する。動的ロードやプロセスの分離はまだ行わない。
    match name {
        "hello" => hello::run().map_err(RunError::Console),
        _ => Err(RunError::NotFound),
    }
}
