use crate::{UART, hvc};
use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

// QEMU virt 固有: UART0 の割込みは SPI 1（INTID 33）、GICv2 の配置は以下。
const GICD: usize = 0x0800_0000;
const GICC: usize = 0x0801_0000;
const UART_IRQ: usize = 33;
const UART_BIT: u32 = 1 << (UART_IRQ - 32);

pub(super) fn configure() {
    // SAFETY: 単一 CPU の EL2 で、DAIF をマスクした初期化中に呼ぶ。
    // オフセットは GICv2 / PL011 の仕様。CPU 0 にレベル割込みとして配信する。
    unsafe {
        write_volatile((UART + 0x38) as *mut u32, 0); // IMSC: 待機時だけ受信を許可。
        write_volatile((GICD + 0x184) as *mut u32, UART_BIT);
        write_volatile((GICD + 0x284) as *mut u32, UART_BIT);
        write_volatile((GICD + 0x400 + UART_IRQ) as *mut u8, 0x80);
        write_volatile((GICD + 0x800 + UART_IRQ) as *mut u8, 1); // CPU 0。
        let config = (GICD + 0xc08) as *mut u32;
        write_volatile(config, read_volatile(config) & !(3 << 2)); // INTID 33: level。
        write_volatile((GICD + 0x104) as *mut u32, UART_BIT);
        asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

pub(super) fn read_blocking() -> (u64, u64) {
    // SAFETY: EL2 の HVC ハンドラー内で DAIF.I=1、HCR_EL2.IMO=1 の状態で呼ぶ。
    // UART の FIFO は無効。RXIM（ビット4）を許可すると1バイトでも IRQ が立つ。
    // IRQ 例外には入らず、保留された IRQ で WFI を起こし、この関数内で完了する。
    unsafe {
        write_volatile((UART + 0x38) as *mut u32, 1 << 4);
        asm!("dsb sy", options(nostack, preserves_flags));
    }
    loop {
        let result = crate::serial_read();
        if result.0 != hvc::WOULD_BLOCK {
            // SAFETY: 受信要求を取り下げてから受領・完了通知する。
            // 先に文字が届いていた場合など、IAR が特殊番号なら EOIR は書かない。
            unsafe {
                write_volatile((UART + 0x38) as *mut u32, 0);
                asm!("dsb sy", options(nostack, preserves_flags));
                let acknowledge = read_volatile((GICC + 0x00c) as *const u32);
                let intid = acknowledge & 0x3ff;
                if intid < 1020 {
                    assert!(intid == UART_IRQ as u32);
                    write_volatile((GICC + 0x010) as *mut u32, acknowledge);
                }
                write_volatile((GICD + 0x284) as *mut u32, UART_BIT);
                asm!("dsb sy", options(nostack, preserves_flags));
            }
            return result;
        }
        // SAFETY: CPU の IRQ マスクは維持するが、WFI の起床はマスクされない。
        // 確認直後に入力されても IRQ は保留され、WFI が待ち続けることはない。
        // 起床が受信以外の理由でも、ループでデータの有無を再確認する。
        unsafe { asm!("dsb sy", "wfi", options(nostack, preserves_flags)) };
    }
}
