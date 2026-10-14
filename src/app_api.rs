// 組み込み EL1 アプリ用の最小 API。現在はシェルと同じメモリ・スタックを使う。
pub(crate) use crate::hvc::Error;

pub(crate) fn write_str(text: &str) -> Result<(), Error> {
    // UART 操作と unsafe は HVC の実装内に閉じ込める。
    crate::hvc::write_str(text)
}
