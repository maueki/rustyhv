// QEMU virt 用の固定領域。user/linker.ld とカーネル linker.ld と一致させる。
pub(super) const ENTRY: usize = 0x4020_0000;
pub(super) const STACK_BASE: usize = 0x4040_0000;
pub(super) const STACK_SIZE: usize = 64 * 1024;

#[used]
#[unsafe(link_section = ".user_image")]
static USER_IMAGE: [u8; include_bytes!(concat!(env!("OUT_DIR"), "/user.bin")).len()] =
    *include_bytes!(concat!(env!("OUT_DIR"), "/user.bin"));
