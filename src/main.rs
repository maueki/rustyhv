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

const UART: usize = 0x0900_0000;
const UART_DR: *mut u32 = UART as *mut u32;
const UART_FR: *const u32 = (UART + 0x18) as *const u32;
const UART_TX_FULL: u32 = 1 << 5;

#[unsafe(no_mangle)]
extern "C" fn kernel_main() -> ! {
    // SAFETY: These are aligned PL011 MMIO registers on QEMU virt, with
    // the MMU off and a single CPU. No other code accesses the UART.
    unsafe {
        write_volatile((UART + 0x30) as *mut u32, 0); // Disable UART.
        write_volatile((UART + 0x24) as *mut u32, 13); // 24 MHz / 115200 baud.
        write_volatile((UART + 0x28) as *mut u32, 1);
        write_volatile((UART + 0x2c) as *mut u32, 0x70); // 8-bit, FIFO enabled.
        write_volatile((UART + 0x30) as *mut u32, 0x101); // UART + transmit.
    }
    serial_write(b"Ferrovisor\r\n");
    serial_write(b"CurrentEL: EL");
    serial_write(&[b'0' + current_el(), b'\r', b'\n']);
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
        #[cfg(feature = "exception-test")]
        // SAFETY: The EL2 vector table and stack are ready. This deliberate
        // synchronous exception enters our handler, which never returns.
        unsafe {
            asm!("brk #0", options(noreturn));
        }
    }
    halt()
}

#[unsafe(no_mangle)]
extern "C" fn exception_handler() -> ! {
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
    serial_write(b"ESR_EL2: ");
    serial_hex(esr);
    serial_write(b"\r\nELR_EL2: ");
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
