#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};

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
        write_volatile((UART + 0x30) as *mut u32, 0x101); // UART と送信を有効化。
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
        let elapsed = wait_100ms(frequency);
        serial_write(b"100 ms wait ticks: ");
        serial_hex(elapsed);
        serial_write(b"\r\nTimer deadline reached\r\n");
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
    }
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
    // CPU の DAIF とタイマーの IMASK はマスク済み。タイマーは停止済み。
    // 以下のレジスタオフセットと制御ビットは GICv2 の仕様。
    // 優先度0x80と優先度マスク0xf0は、Ferrovisor の今回の設定値。
    unsafe {
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
fn test_timer_irq(frequency: u64) -> ! {
    assert!(frequency > 0);
    serial_write(b"Waiting for timer IRQ (100 ms)\r\n");
    let deadline = physical_counter().wrapping_add(frequency.div_ceil(10));
    // SAFETY: EL2 のベクタ、スタック、GIC の設定後に呼ぶ。
    // HCR_EL2.IMO（ビット4）は AArch64 の仕様で、物理 IRQ を EL2 へ送る。
    // 他の HCR ビットを維持し、タイマーの IMASK と DAIF.I だけを解除する。
    // ハンドラーは割込みをマスクして停止するため、元の処理へは復帰しない。
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
    // IRQ はポーリングせず、既存の例外ハンドラーで受ける。
    halt()
}

fn wait_100ms(frequency: u64) -> u64 {
    // EL2 から呼ぶ。100 ms後の物理カウンタ値をタイマーの期限にする。
    // 100 msは確認用に選んだ時間。周波数は固定せず CNTFRQ_EL0 から取得する。
    assert!(frequency > 0);
    let start = physical_counter();
    let deadline = start.wrapping_add(frequency.div_ceil(10));
    // SAFETY: EL2 で CNTHP レジスタへアクセスする。割込みはマスクしたまま、
    // 一度無効化して期限を設定し、有効化する。ISB 後に状態を確認する。
    unsafe {
        asm!(
            "msr CNTHP_CTL_EL2, {masked}",
            "msr CNTHP_CVAL_EL2, {deadline}",
            "msr CNTHP_CTL_EL2, {enabled}",
            "isb",
            masked = in(reg) 0b010u64, // IMASK=1、ENABLE=0。
            deadline = in(reg) deadline,
            enabled = in(reg) 0b011u64, // IMASK=1、ENABLE=1。
            options(nostack, preserves_flags),
        );
    }
    loop {
        let control: u64;
        // SAFETY: EL2 のタイマー制御レジスタを読み、ISTATUS を確認する。
        unsafe {
            asm!("mrs {control}, CNTHP_CTL_EL2", control = out(reg) control,
                options(nomem, nostack, preserves_flags));
        }
        if control & (1 << 2) != 0 {
            break;
        }
        core::hint::spin_loop();
    }
    // SAFETY: 期限到達後、割込みをマスクしたままタイマーを無効化する。
    unsafe {
        asm!("msr CNTHP_CTL_EL2, {masked}", "isb", masked = in(reg) 0b010u64,
            options(nostack, preserves_flags));
    }
    physical_counter().wrapping_sub(start)
}

#[unsafe(no_mangle)]
extern "C" fn exception_handler(vector: u64) -> ! {
    let esr: u64;
    let elr: u64;
    // SAFETY: This handler is entered through the EL2 vector table.
    // Read the exception registers before printing; neither MRS changes them.
    unsafe {
        asm!(
            "mrs {esr}, ESR_EL2",
            "mrs {elr}, ELR_EL2",
            esr = out(reg) esr,
            elr = out(reg) elr,
            options(nomem, nostack, preserves_flags),
        );
    }
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
    halt()
}

fn serial_hex(value: u64) {
    let mut bytes = *b"0x0000000000000000";
    for (index, byte) in bytes[2..].iter_mut().enumerate() {
        let digit = ((value >> ((15 - index) * 4)) & 0xf) as usize;
        *byte = b"0123456789abcdef"[digit];
    }
    serial_write(&bytes);
}

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
    // SAFETY: QEMU boots this kernel at EL1 or EL2, where CurrentEL is
    // readable. MRS only reads the register; it does not change the EL.
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
