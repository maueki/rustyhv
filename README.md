# Ferrovisor

Rust のネイティブアプリと VM が共存する OS／ハイパーバイザーを目指す。
設計とロードマップは [DESIGN.md](DESIGN.md) を参照。
変更履歴は [CHAGELOG.md](CHAGELOG.md) に記録する。

## 最初の一歩: AArch64 でシリアル出力

現在の実装は、QEMU `virt` の単一 CPU で起動し、起動メッセージと現在の例外レベルを出力して待機するところまで。
`CurrentEL` レジスタを読み取って EL2 での起動を確認する。VM の実行、例外ハンドラ、MMU、割込み、ヒープは未実装。
UART アドレスはこの QEMU ボード専用の固定値を使う。

### 準備

Rust / Cargo と `qemu-system-aarch64` が必要。Ubuntu / Debian の QEMU パッケージ名は `qemu-system-arm`。

```sh
rustup target add aarch64-unknown-none-softfloat
sudo apt-get install --no-install-recommends qemu-system-arm
```

浮動小数点・SIMD の初期化を今回の範囲に含めないため、softfloat ターゲットを使う。

### 起動

リポジトリのルートで実行する。

```sh
cargo run
```

QEMU が ELF を直接ロードするため、UEFI や外部ブートローダーは不要。
TCG を使うため KVM は不要。標準設定での出力は次の二行。

```text
Ferrovisor
CurrentEL: EL2
```

出力後は待機し、`Ctrl+C` で QEMU を終了する。

`CARGO_TARGET_DIR` が書き込み不可の場所を指す環境では、
`CARGO_TARGET_DIR=target cargo run` で出力先を指定する。

確認済み環境: Rust / Cargo 1.94.1、QEMU 11.1.1。

次のコマンドで起動メッセージと `CurrentEL: EL2` の出力を確認する。

```sh
CARGO_TARGET_DIR=target timeout 5s cargo run --offline
```

カーネルは出力後に待機するため、この確認では5秒後に QEMU が終了し、
`timeout` の終了コードは `124` となる。文字出力の成功と合わせて確認する。

### レビューの順序

1. `linker.ld`: メモリ上の配置と 64 KiB のスタック。
2. `src/boot.S`: スタック設定、BSS のゼロ初期化、Rust への移行。
3. `src/main.rs`: PL011 UART の初期化、`CurrentEL` の読み取り、文字出力、待機。
4. `.cargo/config.toml`: ビルド対象と QEMU の起動条件。

### 参照

- [QEMU virt](https://www.qemu.org/docs/master/system/arm/virt.html)
- [QEMU virt のメモリマップ](https://github.com/qemu/qemu/blob/v9.2.0/hw/arm/virt.c)
- [Rust の AArch64 ベアメタルターゲット](https://doc.rust-lang.org/rustc/platform-support/aarch64-unknown-none.html)
