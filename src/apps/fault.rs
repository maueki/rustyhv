use crate::app_api;

pub(super) extern "C" fn run() -> ! {
    // 意図した BRK 例外で、OS 全体を停止せずシェルへ戻れることを確認する。
    app_api::abort()
}
