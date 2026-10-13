#![no_std]
#![no_main]

mod hvc;

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, Ordering};

static TIMER_IRQ_RECEIVED: AtomicBool = AtomicBool::new(false);

// vectors.S の272バイトの保存領域と同じ配置。予約欄で16バイト境界を維持する。
#[repr(C)]
struct ExceptionFrame {
    registers: [u64; 31],
    reserved: u64,
    elr: u64,
    spsr: u64,
}

const _: () = {
    assert!(core::mem::size_of::<ExceptionFrame>() == 272);
    assert!(core::mem::offset_of!(ExceptionFrame, elr) == 256);
    assert!(core::mem::offset_of!(ExceptionFrame, spsr) == 264);
};

global_asm!(include_str!("boot.S"));
global_asm!(include_str!("vectors.S"));

unsafe extern "C" {
    static exception_vectors: u8;
}

// QEMU virt 固有: UART0 は PL011 で、MMIO の先頭はこのアドレス。
// 実機へ移植するときは、UART の種類・配置・入力クロックを確認する。
const UART: usize = 0x0900_0000;
// 以下のオフセットとビット位置は QEMU 固有ではなく PL011 の仕様。
const UART_DR: *mut u32 = UART as *mut u32;
const UART_FR: *const u32 = (UART + 0x18) as *const u32;
const UART_TX_FULL: u32 = 1 << 5;

#[unsafe(no_mangle)]
extern "C" fn kernel_main() -> ! {
    // SAFETY: QEMU virt の PL011 レジスタへ、MMU無効・単一CPUでアクセスする。
    // アクセス幅に必要な境界に揃っており、他のコードは UART を操作しない。
    // QEMU virt の入力クロック24 MHzと、今回選んだ115200 baudから除数13と1を求める。
    // レジスタのオフセットと制御ビットは PL011 の仕様に従う。
    unsafe {
        write_volatile((UART + 0x30) as *mut u32, 0); // UART を無効化。
        write_volatile((UART + 0x24) as *mut u32, 13); // 整数部の除数。
        write_volatile((UART + 0x28) as *mut u32, 1); // 小数部の除数。
        write_volatile((UART + 0x2c) as *mut u32, 0x70); // 8ビット・FIFO有効。
        write_volatile((UART + 0x30) as *mut u32, 0x301); // UART・送信・受信を有効化。
    }
    serial_write(b"Ferrovisor\r\n");
    serial_write(b"CurrentEL: EL");
    serial_write(&[b'0' + current_el(), b'\r', b'\n']);
    serial_write(b"Timer frequency (Hz): ");
    let frequency = timer_frequency();
    serial_hex(frequency);
    serial_write(b"\r\n");
    let (gic_version, intid_limit) = gic_info();
    serial_write(b"GIC version: ");
    serial_hex(gic_version);
    serial_write(b"\r\nGIC INTID limit: ");
    serial_hex(intid_limit);
    serial_write(b"\r\n");
    // 今回の起動設定で GICv2 を選んでいる。AArch64 全体の必須条件ではない。
    assert!(gic_version == 2);
    if current_el() == 2 {
        // SAFETY: At EL2, VBAR_EL2 is writable. The assembly table is
        // 2048-byte aligned; ISB makes it effective before the test trap.
        unsafe {
            asm!(
                "adr {table}, {vectors}",
                "msr VBAR_EL2, {table}",
                "isb",
                table = out(reg) _,
                vectors = sym exception_vectors,
                options(nostack, preserves_flags),
            );
        }
        configure_timer_irq();
        serial_write(b"Timer IRQ configured (masked)\r\n");
        #[cfg(feature = "exception-test")]
        // SAFETY: The EL2 vector table and stack are ready. This deliberate
        // synchronous exception enters our handler, which never returns.
        unsafe {
            asm!("brk #0", options(noreturn));
        }
        #[cfg(all(feature = "timer-irq-test", not(feature = "exception-test")))]
        test_timer_irq(frequency);
        #[cfg(all(feature = "el1-test", not(feature = "exception-test")))]
        enter_el1();
    }
    halt()
}

#[cfg(all(feature = "el1-test", not(feature = "exception-test")))]
fn enter_el1() -> ! {
    unsafe extern "C" {
        static __el1_stack_top: u8;
    }
    serial_write(b"Entering EL1\r\n");
    // SAFETY: 単一 CPU の EL2 から、別に確保した16バイト境界のスタックで EL1 へ移る。
    // HCR_EL2 は RW=1（EL1 を AArch64 にする）だけを設定し、Stage 2 や IRQ 転送は無効。
    // SCTLR_EL1 は RES1 ビットを設定し、MMU・キャッシュを無効にする。
    // SPSR_EL2=0x3c5 は DAIF 全マスク・EL1h（SP_EL1 使用）。値は AArch64 の仕様。
    // QEMU の初期化済み UART と物理 RAM 配置を使う。EL1 例外への対応は後続のステップ。
    unsafe {
        asm!(
            "msr daifset, #0xf",
            "msr HCR_EL2, {hcr}",
            "msr SCTLR_EL1, {sctlr}",
            "msr SP_EL1, {stack}",
            "msr ELR_EL2, {entry}",
            "msr SPSR_EL2, {state}",
            "isb",
            "eret",
            hcr = in(reg) 1u64 << 31,
            sctlr = in(reg) 0x30d0_0800u64,
            stack = in(reg) core::ptr::addr_of!(__el1_stack_top),
            entry = in(reg) el1_main as *const (),
            state = in(reg) 0x3c5u64,
            options(noreturn, nostack),
        );
    }
}

#[cfg(all(feature = "el1-test", not(feature = "exception-test")))]
extern "C" fn el1_main() -> ! {
    assert!(current_el() == 1);
    hvc::write_str("CurrentEL: EL1\r\n").expect("console failed");
    // 呼び出し側は安全な Rust 関数を使う。加算は64ビットで折り返す。
    for input in [41u64, u64::MAX] {
        let result = hvc::increment(input).expect("increment failed");
        assert!(result == input.wrapping_add(1));
        assert!(current_el() == 1);
        hvc::write_str("Returned from HVC at EL1\r\n").expect("console failed");
        hvc::write_str("HVC result: ").expect("console failed");
        hvc::write_bytes(&hex_bytes(result)).expect("console failed");
        hvc::write_str("\r\n").expect("console failed");
    }
    // 未対応の操作でも停止せず、エラーとして EL1 へ戻る。
    assert!(hvc::call(u64::MAX, 123) == Err(hvc::Error::UnsupportedOperation));
    hvc::write_str("Unsupported HVC returned an error\r\n").expect("console failed");
    assert!(hvc::increment(0) == Ok(1));
    hvc::write_str("HVC works after an error\r\n").expect("console failed");
    // 不正な整数を送っても UART へ切り詰めて出力せず、エラーで返す。
    assert!(hvc::call(hvc::CONSOLE_BYTE, 256) == Err(hvc::Error::InvalidArgument));
    hvc::write_str("Invalid console byte rejected\r\n").expect("console failed");
    hvc::write_str("Console via HVC: Rustからこんにちは\r\n").expect("console failed");
    assert!(hvc::call(hvc::CONSOLE_READ, 1) == Err(hvc::Error::InvalidArgument));
    halt()
}

fn gic_info() -> (u64, u64) {
    // QEMU virt 固有: GIC Distributor の MMIO 先頭アドレス。
    const GICD_BASE: usize = 0x0800_0000;
    // 0xfe8 / 0x004 は GICv2 の PIDR2 / TYPER のオフセット。
    // SAFETY: QEMU virt の GICv2 Distributor の読み取り専用レジスタ。
    // MMU は無効で、アドレスは32ビットアクセスに必要な境界に揃っている。
    let (pidr2, typer) = unsafe {
        (
            read_volatile((GICD_BASE + 0xfe8) as *const u32),
            read_volatile((GICD_BASE + 0x004) as *const u32),
        )
    };
    // ArchRev は [7:4]。ITLinesNumber [4:0] は32個単位の番号範囲を示す。
    (
        u64::from((pidr2 >> 4) & 0xf),
        u64::from((typer & 0x1f) + 1) * 32,
    )
}

fn configure_timer_irq() {
    // QEMU virt 固有: GICv2 Distributor と CPU interface の MMIO 配置。
    const GICD: usize = 0x0800_0000;
    const GICC: usize = 0x0801_0000;
    // QEMU virt の配線: EL2 物理タイマーは INTID 26。移植先の配線も確認する。
    const TIMER_IRQ: usize = 26;
    const TIMER_BIT: u32 = 1 << TIMER_IRQ;
    // SAFETY: 単一CPU、MMU無効、GICv2 の起動設定として呼ぶ。
    // CPU の DAIF はマスク済み。GIC 設定前にタイマーを停止・マスクする。
    // 以下のレジスタオフセットと制御ビットは GICv2 の仕様。
    // 優先度0x80と優先度マスク0xf0は、Ferrovisor の今回の設定値。
    unsafe {
        asm!("msr CNTHP_CTL_EL2, {masked}", "isb", masked = in(reg) 2u64,
            options(nostack, preserves_flags));
        write_volatile(GICD as *mut u32, 0); // Distributor の配信を停止。
        write_volatile(GICC as *mut u32, 0); // CPU interface の配信を停止。
        write_volatile((GICD + 0x180) as *mut u32, TIMER_BIT); // PPI 26 を無効化。
        write_volatile((GICD + 0x280) as *mut u32, TIMER_BIT); // 保留状態をクリア。
        write_volatile((GICD + 0x400 + TIMER_IRQ) as *mut u8, 0x80); // 優先度。
        write_volatile((GICD + 0x100) as *mut u32, TIMER_BIT); // PPI 26 を有効化。
        write_volatile((GICC + 0x004) as *mut u32, 0xf0); // この優先度より高い要求を許可。
        write_volatile(GICC as *mut u32, 1);
        write_volatile(GICD as *mut u32, 1);
        asm!("dsb sy", "isb", options(nostack, preserves_flags));

        // 設定を読み戻す。ここでは CPU とタイマーの割込みマスクを維持する。
        assert!(read_volatile((GICD + 0x100) as *const u32) & TIMER_BIT != 0);
        assert!(read_volatile((GICD + 0x400 + TIMER_IRQ) as *const u8) == 0x80);
        assert!(read_volatile((GICC + 0x004) as *const u32) == 0xf0);
        assert!(read_volatile(GICC as *const u32) & 1 != 0);
        assert!(read_volatile(GICD as *const u32) & 1 != 0);
    }
}

#[cfg(all(feature = "timer-irq-test", not(feature = "exception-test")))]
fn test_timer_irq(frequency: u64) {
    assert!(frequency > 0);
    serial_write(b"Waiting for timer IRQ (100 ms)\r\n");
    let deadline = physical_counter().wrapping_add(frequency.div_ceil(10));
    // SAFETY: EL2 のベクタ、スタック、GIC の設定後に呼ぶ。
    // HCR_EL2.IMO（ビット4）は AArch64 の仕様で、物理 IRQ を EL2 へ送る。
    // 他の HCR ビットを維持し、タイマーの IMASK と DAIF.I だけを解除する。
    // ハンドラーはタイマーを停止し、保存した CPU 状態へ復帰する。
    unsafe {
        asm!(
            "mrs {hcr}, HCR_EL2",
            "orr {hcr}, {hcr}, #0x10",
            "msr HCR_EL2, {hcr}",
            "isb",
            "msr CNTHP_CVAL_EL2, {deadline}",
            "msr CNTHP_CTL_EL2, {enabled}",
            "isb",
            "msr daifclr, #0x2",
            hcr = out(reg) _,
            deadline = in(reg) deadline,
            enabled = in(reg) 1u64, // ENABLE=1、IMASK=0。
            options(nostack, preserves_flags),
        );
    }
    // ハンドラーの完了を待つ。通知は割込み前後のどちらでも見落とさない。
    while !TIMER_IRQ_RECEIVED.load(Ordering::Acquire) {
        core::hint::spin_loop();
    }
    // SAFETY: 一度の受信試験を終え、CPU の IRQ を再びマスクする。
    unsafe { asm!("msr daifset, #0x2", options(nostack, preserves_flags)) };
    serial_write(b"Returned from timer IRQ\r\n");
}

#[unsafe(no_mangle)]
extern "C" fn exception_handler(vector: u64, frame: *mut ExceptionFrame) {
    let esr: u64;
    let elr: u64;
    // SAFETY: EL2 の例外ベクタから呼ぶ。出力前に例外レジスタを読む。
    // MRS はこれらのレジスタの値を変更しない。
    unsafe {
        asm!(
            "mrs {esr}, ESR_EL2",
            "mrs {elr}, ELR_EL2",
            esr = out(reg) esr,
            elr = out(reg) elr,
            options(nomem, nostack, preserves_flags),
        );
    }
    // 下位 EL の AArch64 同期例外入口。EC=0x16 は AArch64 の HVC。
    // 即値0だけを今回の試験として扱う。HVC の ELR は次の命令を指すので変更しない。
    if vector == 8 && esr >> 26 == 0x16 && esr & 0xffff == 0 {
        assert!(current_el() == 2);
        assert!(!frame.is_null());
        // SAFETY: 状態保存入口が第2引数に、自身のスタック上の保存領域を渡す。
        // 配置は ExceptionFrame と一致し、割込みはマスク済み。参照は復帰前に終了する。
        // 保存された x0・x1 を変更すると、復元時に EL1 の状態コード・結果となる。
        let frame = unsafe { &mut *frame };
        let operation = frame.registers[0];
        let argument = frame.registers[1];
        let (status, result) = match operation {
            hvc::INCREMENT => (hvc::OK, argument.wrapping_add(1)),
            hvc::CONSOLE_BYTE => match u8::try_from(argument) {
                Ok(byte) => {
                    serial_write(&[byte]);
                    (hvc::OK, 0)
                }
                Err(_) => (hvc::INVALID_ARGUMENT, 0),
            },
            hvc::CONSOLE_READ if argument == 0 => serial_read(),
            hvc::CONSOLE_READ => (hvc::INVALID_ARGUMENT, 0),
            _ => (hvc::UNSUPPORTED, 0),
        };
        frame.registers[0] = status;
        frame.registers[1] = result;
        return;
    }
    // HVC は通常の呼び出しとして扱い、1バイトごとの例外ログは出さない。
    serial_write(b"EXCEPTION\r\n");
    serial_write(b"Vector offset: ");
    serial_hex(vector * 128);
    serial_write(b"\r\nKind: ");
    // Each group contains synchronous, IRQ, FIQ, and SError entries.
    serial_write(match vector & 3 {
        0 => b"Synchronous\r\n",
        1 => b"IRQ\r\n",
        2 => b"FIQ\r\n",
        _ => b"SError\r\n",
    });
    if vector & 3 == 0 {
        // ESR describes synchronous exceptions, not IRQ or FIQ.
        serial_write(b"ESR_EL2: ");
        serial_hex(esr);
        serial_write(b"\r\n");
    }
    serial_write(b"ELR_EL2: ");
    serial_hex(elr);
    serial_write(b"\r\n");
    // 現在の EL・SP_ELx の IRQ 入口も CPU 状態を保存している。
    if vector == 5 && finish_irq() {
        TIMER_IRQ_RECEIVED.store(true, Ordering::Release);
        return;
    }
    halt()
}

fn finish_irq() -> bool {
    // QEMU virt 固有: GICv2 CPU interface の配置。
    const GICC: usize = 0x0801_0000;
    // IAR / EOIR のオフセットと INTID の下位10ビットは GICv2 の仕様。
    // SAFETY: EL2 の IRQ ハンドラーから、DAIF をマスクした状態で呼ぶ。
    // GIC は初期化済み。IAR を一度読むと割込みを受領して active 状態にする。
    let acknowledge = unsafe { read_volatile((GICC + 0x00c) as *const u32) };
    let intid = acknowledge & 0x3ff;
    serial_write(b"IRQ INTID: ");
    serial_hex(u64::from(intid));
    serial_write(b"\r\n");
    // 1020以上は特殊な応答で、通常の割込みとして完了通知しない。
    if intid >= 1020 {
        serial_write(b"No IRQ to complete\r\n");
        return false;
    }
    if intid == 26 {
        // QEMU virt の EL2 物理タイマー。完了通知前に割込み要求を止める。
        // SAFETY: EL2 のタイマーを無効化・マスクし、変更を同期する。
        unsafe {
            asm!("msr CNTHP_CTL_EL2, {masked}", "isb", masked = in(reg) 2u64,
                options(nostack, preserves_flags));
        }
        serial_write(b"Timer stopped\r\n");
    }
    // SAFETY: GICC_CTLR=1 により EOImode=0。EOIR で優先度を戻し active 状態を解除する。
    // デバイス側の停止を完了してから、IAR の応答全体をそのまま返す。
    unsafe {
        asm!("dsb sy", options(nostack, preserves_flags));
        write_volatile((GICC + 0x010) as *mut u32, acknowledge);
        asm!("dsb sy", options(nostack, preserves_flags));
    }
    serial_write(b"IRQ completed\r\n");
    // 未対応の割込みでは復帰せず停止する。
    intid == 26
}

fn hex_bytes(value: u64) -> [u8; 18] {
    let mut bytes = *b"0x0000000000000000";
    for (index, byte) in bytes[2..].iter_mut().enumerate() {
        let digit = ((value >> ((15 - index) * 4)) & 0xf) as usize;
        *byte = b"0123456789abcdef"[digit];
    }
    bytes
}

fn serial_hex(value: u64) {
    serial_write(&hex_bytes(value));
}

#[cfg(all(feature = "timer-irq-test", not(feature = "exception-test")))]
fn physical_counter() -> u64 {
    let counter: u64;
    // SAFETY: EL2 の起動処理から呼び、物理カウンタを読み取る。
    // ISB で先行命令の実行を同期してから計測する。設定は変更しない。
    unsafe {
        asm!(
            "isb",
            "mrs {counter}, CNTPCT_EL0",
            counter = out(reg) counter,
            options(nostack, preserves_flags),
        );
    }
    counter
}

fn timer_frequency() -> u64 {
    let frequency: u64;
    // SAFETY: QEMU の起動環境では EL1/EL2 から CNTFRQ_EL0 を読み取れる。
    // MRS は周波数の設定値を読むだけで、タイマーの動作を変更しない。
    unsafe {
        asm!(
            "mrs {frequency}, CNTFRQ_EL0",
            frequency = out(reg) frequency,
            options(nomem, nostack, preserves_flags),
        );
    }
    frequency
}

fn current_el() -> u8 {
    let value: u64;
    // SAFETY: EL1 または EL2 のコードから呼び、読み取り可能な CurrentEL を読む。
    // MRS は現在の EL を変更しない。
    unsafe {
        asm!(
            "mrs {value}, CurrentEL",
            value = out(reg) value,
            options(nomem, nostack, preserves_flags),
        );
    }
    // CurrentEL encodes the exception level in bits [3:2].
    ((value >> 2) & 0b11) as u8
}

fn serial_write(bytes: &[u8]) {
    for &byte in bytes {
        // SAFETY: Same fixed QEMU virt MMIO mapping as above. Volatile
        // accesses poll the transmit FIFO and write one byte at a time.
        unsafe {
            while read_volatile(UART_FR) & UART_TX_FULL != 0 {
                core::hint::spin_loop();
            }
            write_volatile(UART_DR, u32::from(byte));
        }
    }
}

fn serial_read() -> (u64, u64) {
    // SAFETY: EL2 の HVC ハンドラーから、初期化済み PL011 を単一 CPU で操作する。
    // RXFE（ビット4）と DR のエラービット [11:8] は PL011 の仕様。
    unsafe {
        if read_volatile(UART_FR) & (1 << 4) != 0 {
            return (hvc::WOULD_BLOCK, 0);
        }
        let data = read_volatile(UART_DR);
        if data & 0xf00 != 0 {
            // ECR への書き込みで受信エラーを解除。不良バイトは呼び出し元へ渡さない。
            write_volatile((UART + 0x04) as *mut u32, 0);
            return (hvc::INPUT_ERROR, 0);
        }
        (hvc::OK, u64::from(data & 0xff))
    }
}

fn halt() -> ! {
    loop {
        // SAFETY: WFE does not access memory; execution remains here on wake.
        unsafe { asm!("wfe", options(nomem, nostack)) };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_write(b"PANIC\r\n");
    halt()
}
