use crate::hvc::{self, Error};

const PROMPT: &str = "ferrovisor> ";
const LINE_CAPACITY: usize = 128;

pub(super) fn run() -> Result<(), Error> {
    hvc::write_str("Ferrovisor shell (EL1). Type help.\r\n")?;
    hvc::write_str(PROMPT)?;
    let mut line = [0u8; LINE_CAPACITY];
    let mut length = 0;
    let mut overflow = false;
    let mut after_cr = false;
    loop {
        let byte = match hvc::read_byte() {
            Ok(byte) => byte,
            Err(Error::ReceiveFault) => {
                length = 0;
                overflow = false;
                after_cr = false;
                hvc::write_str("\r\nInput error; line discarded\r\n")?;
                hvc::write_str(PROMPT)?;
                continue;
            }
            Err(error) => return Err(error),
        };
        // CRLF を1回の Enter として扱う。CR 単独・LF 単独にも対応する。
        if after_cr && byte == b'\n' {
            after_cr = false;
            continue;
        }
        after_cr = byte == b'\r';
        match byte {
            b'\r' | b'\n' => {
                hvc::write_str("\r\n")?;
                if overflow {
                    hvc::write_str("Line too long (max 128 bytes); discarded\r\n")?;
                } else {
                    // 入力は ASCII のみ受け付けるので UTF-8 として常に有効。
                    execute(core::str::from_utf8(&line[..length]).expect("ASCII line"))?;
                }
                length = 0;
                overflow = false;
                hvc::write_str(PROMPT)?;
            }
            3 => {
                // Ctrl+C は行の取り消し。シェル自体は終了しない。
                length = 0;
                overflow = false;
                hvc::write_str("^C\r\n")?;
                hvc::write_str(PROMPT)?;
            }
            8 | 127 if !overflow && length > 0 => {
                length -= 1;
                hvc::write_str("\x08 \x08")?;
            }
            8 | 127 => {}
            0x20..=0x7e if !overflow && length < line.len() => {
                line[length] = byte;
                length += 1;
                hvc::write_bytes(&[byte])?;
            }
            0x20..=0x7e if !overflow => {
                // 超過後は行全体を破棄する。途中までのコマンドを実行しない。
                overflow = true;
                hvc::write_str("\x07")?;
            }
            _ => {} // 非 ASCII・その他の制御文字は受け付けない。
        }
    }
}

fn execute(line: &str) -> Result<(), Error> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }
    let (command, arguments) = line.split_once(' ').unwrap_or((line, ""));
    let arguments = arguments.trim_start();
    match command {
        "help" if arguments.is_empty() => hvc::write_str(
            "help       Show commands\r\ninfo       Show system information\r\necho TEXT  Print TEXT\r\nrun hello  Run the built-in Rust app\r\n",
        ),
        "info" if arguments.is_empty() => hvc::write_str(
            "Ferrovisor\r\nShell: EL1, console: EL2 via HVC\r\nPlatform: QEMU virt, AArch64, 1 CPU\r\n",
        ),
        "echo" => {
            hvc::write_str(arguments)?;
            hvc::write_str("\r\n")
        }
        "run" => {
            let mut names = arguments.split_ascii_whitespace();
            let Some(name) = names.next() else {
                return hvc::write_str("Usage: run hello\r\n");
            };
            if names.next().is_some() {
                return hvc::write_str("Usage: run hello\r\n");
            }
            match crate::apps::run(name) {
                Ok(()) => Ok(()),
                Err(crate::apps::RunError::NotFound) => {
                    hvc::write_str("Unknown app; available: hello\r\n")
                }
                Err(crate::apps::RunError::Console(error)) => Err(error),
            }
        }
        "help" | "info" => hvc::write_str("This command takes no arguments\r\n"),
        _ => hvc::write_str("Unknown command; type help\r\n"),
    }
}
