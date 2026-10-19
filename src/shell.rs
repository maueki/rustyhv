use crate::hvc::{self, Error};
use crate::process::{ExitReason, Manager, State};

const PROMPT: &str = "rustyhv> ";
const LINE_CAPACITY: usize = 128;

pub(super) fn run() -> Result<(), Error> {
    hvc::write_str("rustyhv shell (EL1). Type help.\r\n")?;
    hvc::write_str(PROMPT)?;
    let mut processes = Manager::new();
    let mut line = [0u8; LINE_CAPACITY];
    let mut length = 0;
    let mut overflow = false;
    let mut after_cr = false;
    loop {
        let byte = match hvc::read_byte() {
            // 受信成功。入力された1バイトを、下のキー処理へ渡す。
            Ok(byte) => byte,
            // UART の受信エラー。編集中の行を破棄して、新しい入力を待つ。
            Err(Error::ReceiveFault) => {
                length = 0;
                overflow = false;
                after_cr = false;
                hvc::write_str("\r\nInput error; line discarded\r\n")?;
                hvc::write_str(PROMPT)?;
                continue;
            }
            // その他の HVC エラー。シェルを継続せず、呼び出し元へエラーを返す。
            Err(error) => return Err(error),
        };
        // CRLF を1回の Enter として扱う。CR 単独・LF 単独にも対応する。
        if after_cr && byte == b'\n' {
            after_cr = false;
            continue;
        }
        after_cr = byte == b'\r';
        match byte {
            // Enter（CR または LF）。行を実行し、入力状態を初期化してプロンプトを出す。
            b'\r' | b'\n' => {
                hvc::write_str("\r\n")?;
                if overflow {
                    hvc::write_str("Line too long (max 128 bytes); discarded\r\n")?;
                } else {
                    // 入力は ASCII のみ受け付けるので UTF-8 として常に有効。
                    execute(
                        core::str::from_utf8(&line[..length]).expect("ASCII line"),
                        &mut processes,
                    )?;
                }
                length = 0;
                overflow = false;
                hvc::write_str(PROMPT)?;
            }
            // Ctrl+C（ETX=3）。行を取り消す。シェル自体は終了しない。
            3 => {
                length = 0;
                overflow = false;
                hvc::write_str("^C\r\n")?;
                hvc::write_str(PROMPT)?;
            }
            // Backspace（8）または DEL（127）。有効な行の末尾を1文字消し、画面も消す。
            8 | 127 if !overflow && length > 0 => {
                length -= 1;
                hvc::write_str("\x08 \x08")?;
            }
            // 空行、または上限超過で破棄予定の行では、削除キーを無視する。
            8 | 127 => {}
            // ASCII の印字文字（空白〜チルダ）。空きがあれば保存し、入力をエコーする。
            0x20..=0x7e if !overflow && length < line.len() => {
                line[length] = byte;
                length += 1;
                hvc::write_bytes(&[byte])?;
            }
            // 初めて入力上限を超えた印字文字。ベルを鳴らし、行全体を破棄対象にする。
            0x20..=0x7e if !overflow => {
                overflow = true;
                hvc::write_str("\x07")?;
            }
            // 上限超過後の印字文字、非 ASCII、上で扱わない制御文字は無視する。
            _ => {}
        }
    }
}

fn execute(line: &str, processes: &mut Manager) -> Result<(), Error> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }
    let (command, arguments) = line.split_once(' ').unwrap_or((line, ""));
    let arguments = arguments.trim_start();
    match command {
        "help" if arguments.is_empty() => hvc::write_str(
            "help       Show commands\r\ninfo       Show system information\r\nps         Show the last process\r\necho TEXT  Print TEXT\r\nrun NAME   Run hello, fault, globals, globals-fault\r\nProtection tests: read-kernel, write-kernel, read-device, write-code, exec-stack, stack-guard, exec-data\r\n",
        ),
        "info" if arguments.is_empty() => hvc::write_str(
            "rustyhv\r\nShell: EL1, console: EL2 via HVC\r\nMemory: EL0 protected, one process at a time\r\nPlatform: QEMU virt, AArch64, 1 CPU\r\n",
        ),
        "echo" => {
            hvc::write_str(arguments)?;
            hvc::write_str("\r\n")
        }
        "run" => {
            let mut names = arguments.split_ascii_whitespace();
            let Some(name) = names.next() else {
                return hvc::write_str("Usage: run NAME (see help)\r\n");
            };
            if names.next().is_some() {
                return hvc::write_str("Usage: run NAME (see help)\r\n");
            }
            let Some(app) = crate::apps::find(name) else {
                return hvc::write_str("Unknown app; type help\r\n");
            };
            match processes.run(app) {
                Ok(ExitReason::Code(0)) => Ok(()),
                Ok(reason) => {
                    let status = match reason {
                        ExitReason::Code(status) => status,
                        ExitReason::Fault { .. } => crate::el0::FAULT_STATUS,
                    };
                    hvc::write_str("App exited with status: ")?;
                    hvc::write_bytes(&crate::hex_bytes(status))?;
                    hvc::write_str("\r\n")
                }
                Err(message) => hvc::write_str(message),
            }
        }
        "ps" if arguments.is_empty() => show_process(processes),
        "help" | "info" | "ps" => hvc::write_str("This command takes no arguments\r\n"),
        _ => hvc::write_str("Unknown command; type help\r\n"),
    }
}

fn show_process(processes: &Manager) -> Result<(), Error> {
    let Some(process) = processes.last() else {
        return hvc::write_str("No processes\r\n");
    };
    hvc::write_str("PID NAME STATE REASON\r\n")?;
    hvc::write_bytes(&crate::hex_bytes(process.pid))?;
    hvc::write_str(" ")?;
    hvc::write_str(process.name)?;
    match process.state {
        State::Ready => hvc::write_str(" Ready -\r\n"),
        State::Running => hvc::write_str(" Running -\r\n"),
        State::Exited(reason) => {
            let (label, value) = match reason {
                ExitReason::Code(code) => (" Exited code=", code),
                ExitReason::Fault { esr } => (" Exited fault=", esr),
            };
            hvc::write_str(label)?;
            hvc::write_bytes(&crate::hex_bytes(value))?;
            hvc::write_str("\r\n")
        }
    }
}
