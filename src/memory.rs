use core::{arch::asm, ptr};

// QEMU virt の RAM は0x40000000から128 MiB。現在の起動設定専用の配置。
const RAM: u64 = 0x4000_0000;
const BLOCK: u64 = 2 * 1024 * 1024;
const AF: u64 = 1 << 10;
const NORMAL: u64 = (1 << 2) | (3 << 8); // MAIR[1]、Inner Shareable。
const USER_RW: u64 = 1 << 6;
const USER_RO: u64 = 3 << 6;
const PXN: u64 = 1 << 53;
const UXN: u64 = 1 << 54;

#[repr(C, align(4096))]
struct Table([u64; 512]);

// 初期化後は一切変更しない。単一 CPU・単一アドレス空間で ASID 切替はまだ行わない。
static mut ROOT: Table = Table([0; 512]);
static mut LOW: Table = Table([0; 512]);
static mut RAM_TABLE: Table = Table([0; 512]);
static mut IMAGE: Table = Table([0; 512]);
static mut STACK: Table = Table([0; 512]);

pub(super) fn enable() {
    let control: u64;
    // SAFETY: EL1 の初期化中。MMU とデータ・命令キャッシュが無効であることを確認する。
    unsafe {
        asm!("mrs {value}, SCTLR_EL1", value = out(reg) control,
            options(nomem, nostack, preserves_flags));
    }
    assert_eq!(control & ((1 << 12) | (1 << 2) | 1), 0);
    let root = ptr::addr_of_mut!(ROOT);
    let low = ptr::addr_of_mut!(LOW);
    let ram = ptr::addr_of_mut!(RAM_TABLE);
    let image = ptr::addr_of_mut!(IMAGE);
    let stack = ptr::addr_of_mut!(STACK);
    // SAFETY: 例外はマスク済み、まだページテーブルを使う CPU は存在しない。
    // 各表は4096バイト境界。現在の仮想アドレスと物理アドレスは一致する。
    unsafe {
        (*root).0[0] = low as u64 | 3;
        (*root).0[1] = ram as u64 | 3;
        // GIC と UART の MMIO だけを EL1 に公開。MAIR[0] は Device-nGnRnE。
        for address in [0x0800_0000u64, 0x0900_0000] {
            (*low).0[(address / BLOCK) as usize] = address | AF | PXN | UXN | 1;
        }
        for index in 0..64 {
            let execute = if index == 0 { UXN } else { PXN | UXN };
            (*ram).0[index] = (RAM + index as u64 * BLOCK) | AF | NORMAL | execute | 1;
        }
        // ユーザーイメージは読み取り・実行のみ。EL1 からの実行も禁止する。
        (*ram).0[1] = image as u64 | 3;
        for index in 0..crate::el0_image::IMAGE_SIZE.div_ceil(4096) {
            (*image).0[index] =
                (crate::el0_image::ENTRY + index * 4096) as u64 | AF | NORMAL | USER_RO | PXN | 3;
        }
        (*ram).0[2] = stack as u64 | 3;
        // スタックの64 KiBだけを EL0 に読み書き可能として割り当て、実行は禁止する。
        // 残りの PTE は無効のままにし、スタックの上側には未マップ領域を残す。
        for index in 0..crate::el0_image::STACK_SIZE / 4096 {
            (*stack).0[index] = (crate::el0_image::STACK_BASE + index * 4096) as u64
                | AF
                | NORMAL
                | USER_RW
                | PXN
                | UXN
                | 3;
        }
        // 39ビット VA・4 KiB粒度・32ビット PA。TTBR1 は使わない。
        // キャッシュは無効のままにし、MMU 無効の EL2 との共有メモリを扱う。
        asm!(
            "dsb sy",
            "msr MAIR_EL1, {mair}",
            "msr TCR_EL1, {tcr}",
            "msr TTBR0_EL1, {root}",
            "isb",
            "tlbi vmalle1",
            "dsb sy",
            "isb",
            "msr SCTLR_EL1, {enabled}",
            "isb",
            mair = in(reg) 0xff00u64,
            tcr = in(reg) 25u64 | (1 << 23) | (3 << 12) | (1 << 10) | (1 << 8),
            root = in(reg) root,
            enabled = in(reg) control | 1,
            options(nostack, preserves_flags),
        );
    }
}

pub(super) fn reset_stack() {
    // SAFETY: 前のアプリは終了済みで、次のアプリはまだ実行していない。
    // 単一の専用スタック領域を消去し、次の PID に前の内容を渡さない。
    unsafe {
        ptr::write_bytes(
            crate::el0_image::STACK_BASE as *mut u8,
            0,
            crate::el0_image::STACK_SIZE,
        );
    }
}
