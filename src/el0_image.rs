// QEMU virt 用の固定領域。user/linker.ld とカーネル linker.ld と一致させる。
pub(super) const ENTRY: usize = 0x4020_0000;
pub(super) const STACK_BASE: usize = 0x4040_0000;
pub(super) const STACK_SIZE: usize = 64 * 1024;
pub(super) const IMAGE_SIZE: usize = include_bytes!(concat!(env!("OUT_DIR"), "/user.bin")).len();
pub(super) const DATA_BASE: usize = 0x4060_0000;
pub(super) const DATA_SIZE: usize = 64 * 1024;
pub(super) const DATA_INITIAL: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/user_data.bin"));
include!(concat!(env!("OUT_DIR"), "/user_layout.rs"));

const _: () = {
    assert!(DATA_INITIAL.len() <= DATA_MEMORY_SIZE);
    assert!(DATA_MEMORY_SIZE <= DATA_SIZE);
};

#[used]
#[unsafe(link_section = ".user_image")]
static USER_IMAGE: [u8; IMAGE_SIZE] = *include_bytes!(concat!(env!("OUT_DIR"), "/user.bin"));
