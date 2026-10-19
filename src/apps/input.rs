use crate::app_api::{self, Error};

pub(super) fn run() -> Result<(), Error> {
    let mut buffer = [0u8; 32];
    let mut length = 0;
    let mut overflow = false;
    app_api::write_str("Input (max 32 ASCII bytes): ")?;
    loop {
        match app_api::read_byte()? {
            b'\r' | b'\n' => {
                app_api::write_str("\r\n")?;
                if overflow {
                    app_api::write_str("Input too long; discarded\r\n")?;
                    return Err(Error(2));
                }
                app_api::write_str("Input: ")?;
                // バッファには ASCII だけを保存している。
                app_api::write_str(core::str::from_utf8(&buffer[..length]).expect("ASCII input"))?;
                return app_api::write_str("\r\n");
            }
            3 => return app_api::write_str("^C\r\nInput cancelled\r\n"),
            8 | 127 if !overflow && length > 0 => {
                length -= 1;
                app_api::write_str("\x08 \x08")?;
            }
            byte @ 0x20..=0x7e if !overflow && length < buffer.len() => {
                buffer[length] = byte;
                length += 1;
                app_api::write_byte(byte)?;
            }
            0x20..=0x7e if !overflow => {
                overflow = true;
                app_api::write_str("\x07")?;
            }
            _ => {}
        }
    }
}
