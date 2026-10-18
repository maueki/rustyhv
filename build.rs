fn main() {
    println!("cargo:rerun-if-changed=linker.ld");
    println!("cargo:rustc-link-arg=-Tlinker.ld");
    for file in [
        "user/main.rs",
        "user/linker.ld",
        "src/app_api.rs",
        "src/apps/hello.rs",
        "src/apps/fault.rs",
        "src/svc_abi.rs",
    ] {
        println!("cargo:rerun-if-changed={file}");
    }
    if std::env::var_os("CARGO_FEATURE_SHELL").is_some()
        && std::env::var_os("CARGO_FEATURE_EXCEPTION_TEST").is_none()
    {
        build_user_image();
    }
}

fn build_user_image() {
    use std::{env, fs, path::PathBuf, process::Command};
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let elf_path = out.join("user.elf");
    let status = Command::new(env::var_os("RUSTC").expect("RUSTC"))
        .args([
            "user/main.rs",
            "--edition=2024",
            "--crate-name",
            "rustyhv_user",
            "--target",
        ])
        .arg(env::var("TARGET").expect("TARGET"))
        .args([
            "-C",
            "panic=abort",
            "-C",
            "opt-level=2",
            "-C",
            "relocation-model=static",
            "-C",
            "link-arg=-Tuser/linker.ld",
            "-o",
        ])
        .arg(&elf_path)
        .status()
        .expect("run rustc for user image");
    assert!(status.success(), "user image build failed");
    let elf = fs::read(elf_path).expect("read user ELF");
    assert_eq!(&elf[..6], b"\x7fELF\x02\x01"); // ELF64、リトルエンディアン。
    let u16_at = |i| u16::from_le_bytes(elf[i..i + 2].try_into().unwrap()) as usize;
    let u64_at = |i| u64::from_le_bytes(elf[i..i + 8].try_into().unwrap()) as usize;
    let base = 0x4020_0000usize;
    assert_eq!(u64_at(24), base, "unexpected user entry");
    let mut image = Vec::new();
    // 自分でリンクした ELF の PT_LOAD だけを取り出し、固定配置のバイナリにする。
    for index in 0..u16_at(56) {
        let header = u64_at(32) + index * u16_at(54);
        if u32::from_le_bytes(elf[header..header + 4].try_into().unwrap()) != 1 {
            continue;
        }
        let offset = u64_at(header + 8);
        let address = u64_at(header + 16);
        let size = u64_at(header + 32);
        assert_eq!(
            size,
            u64_at(header + 40),
            "user image has writable zero-fill data"
        );
        assert!(address >= base && address + size <= base + 0x20_0000);
        let start = address - base;
        image.resize(image.len().max(start + size), 0);
        image[start..start + size].copy_from_slice(&elf[offset..offset + size]);
    }
    assert!(!image.is_empty());
    fs::write(out.join("user.bin"), image).expect("write user image");
}
