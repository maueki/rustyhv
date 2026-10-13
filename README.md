# Ferrovisor

Rust のネイティブアプリと VM が共存する OS／ハイパーバイザーを目指す。
設計とロードマップは [DESIGN.md](DESIGN.md) を参照。
変更履歴は [CHAGELOG.md](CHAGELOG.md) に記録する。

## 最初の一歩: AArch64 でシリアル出力

現在の実装は、QEMU `virt` の単一 CPU で起動し、起動メッセージと現在の例外レベルを出力して待機するところまで。
`CurrentEL` レジスタを読み取り、EL2 なら例外ベクタを設置する。例外時は `EXCEPTION` と出力する。
`timer-irq-test` feature でタイマー IRQ の受信も確認できる。
タイマー IRQ からの復帰と、`el1-test` feature で EL2 → EL1 の移行・HVC の往復に対応。
VM の実行、MMU、ヒープは未実装。
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
TCG を使うため KVM は不要。標準設定の出力例は次の六行。

```text
Ferrovisor
CurrentEL: EL2
Timer frequency (Hz): 0x0000000003b9aca0
GIC version: 0x0000000000000002
GIC INTID limit: 0x0000000000000120
Timer IRQ configured (masked)
```

出力後は待機し、`Ctrl+C` で QEMU を終了する。

`CNTFRQ_EL0` が保持するシステムカウンタの周波数を Hz 単位の16進数で表示する。
上の値は 62,500,000 Hz（62.5 MHz）。CPU の動作周波数とは別で、
カウンタの差分を時間に換算するときに使う。周波数の値は実行環境に依存する。
QEMU は `gic-version=2` で GICv2 に固定する。
Distributor の `GICD_PIDR2` でバージョンを、`GICD_TYPER` で割込み番号の範囲を確認する。
`INTID limit` は上限を含まない値で、`0x120` は番号0〜287の範囲を示す。
範囲内のすべての番号にデバイスが接続されているという意味ではない。
EL2 起動時はタイマーを停止・マスクし、物理タイマーの PPI 26 を GIC で有効化し、
優先度を `0x80`、CPU interface の優先度マスクを `0xf0` に設定する。
Distributor と CPU interface の配信を有効にし、設定を読み戻して確認する。
タイマーは停止済みで IMASK=1、CPU の IRQ もマスク済みなので、割込みはまだ受信しない。

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

### 例外ハンドラの確認

```sh
cargo run --features exception-test
```

EL2 の例外ベクタを設定後、`brk #0` で同期例外を発生させる。
起動メッセージに続いて例外情報が表示され、そのまま待機する。
通常の `cargo run` は意図的な例外を発生させない。

```text
EXCEPTION
Vector offset: 0x0000000000000200
Kind: Synchronous
ESR_EL2: 0x00000000f2000000
ELR_EL2: 0x0000000040081b1c
```

`ESR_EL2` は同期例外の原因を示すレジスタで、この値は AArch64 の `brk #0` に対応する。
`ELR_EL2` は例外からの復帰先を記録するレジスタで、BRK ではその命令自身のアドレスを指す。
上のアドレスは確認時の例で、ビルドによって変わる。
両方を固定16桁の16進数で表示する。同期例外以外では ESR の表示を省略する。

`Vector offset` はベクタ表の先頭から入口までの距離。
`0x200` は現在の EL で SP_ELx を使っているときの同期例外に対応する。
各入口が番号を `x0` で Rust ハンドラへ渡し、`Kind` に Synchronous / IRQ / FIQ / SError を表示する。
QEMU で同期例外とタイマー IRQ を確認済み。FIQ・SError の発生試験は後続段階で行う。

`src/vectors.S` に 2048 バイト境界のベクタ表を置き、16 個の入口を共通ハンドラへ接続する。
現在の EL・SP_ELx のタイマー IRQ と、下位 EL の AArch64 同期例外は CPU 状態を保存する。
タイマー IRQ と試験用 HVC は復帰し、未対応の例外は停止する。
その他の入口はスタックを初期位置へ戻して停止する。
EL1 での起動時は EL2 用の設定と例外テストをスキップする。

### タイマー IRQ の受信確認

```sh
cargo run --features timer-irq-test
```

通常の初期化後、`HCR_EL2.IMO` を設定して物理 IRQ を EL2 へ送る。
EL2 物理タイマーに100 ms後の期限を設定し、タイマーの `IMASK` と CPU の `DAIF.I` を解除する。
これらの操作は AArch64 の機能で、GIC の配置とタイマーの配線は現在の QEMU `virt` 設定に依存する。

```text
Waiting for timer IRQ (100 ms)
EXCEPTION
Vector offset: 0x0000000000000280
Kind: IRQ
ELR_EL2: ...
IRQ INTID: 0x000000000000001a
Timer stopped
IRQ completed
Returned from timer IRQ
```

`0x280` は現在の EL で SP_ELx を使っているときの IRQ 入口。
ハンドラーで割込みをマスクし、GIC の `GICC_IAR` から割込み番号を取得する。
`0x1a` は EL2 物理タイマーの PPI 26。タイマーを停止して要求を取り下げ、
`GICC_EOIR` に IAR の応答を返して完了通知する。
特殊な番号1020〜1023には完了通知しない。
現在の EL・SP_ELx の IRQ 入口では、汎用レジスタ x0〜x30、`ELR_EL2`、`SPSR_EL2` を保存する。
ハンドラーの完了後に復元し、`eret` で元の処理へ戻る。SP は保存領域の確保・解放で元に戻す。
softfloat ターゲットなので FP/SIMD レジスタの保存は含めない。
受信したタイマー IRQ の完了を AtomicBool で通知し、試験側は通知を確認した後、
CPU の IRQ を再びマスクして `Returned from timer IRQ` と出力し待機する。
未対応の割込みや特殊な番号では停止する。周期的なタイマーの再設定はまだ行わない。
`exception-test` も指定した場合は BRK テストを優先し、タイマー IRQ テストをスキップする。
通常の `cargo run` では IRQ のマスクを維持する。

### EL2 から EL1 への移行確認

```sh
cargo run --features el1-test
```

通常の初期化後、EL1 用に確保した別の64 KiBスタックを `SP_EL1` に設定する。
`HCR_EL2.RW=1` で EL1 を AArch64 とし、`SCTLR_EL1` は MMU・キャッシュを無効にする。
`ELR_EL2` に EL1 の入口、`SPSR_EL2` に DAIF 全マスク・EL1h を設定し、`eret` で移行する。

```text
Entering EL1
CurrentEL: EL1
```

EL1 で `CurrentEL` を確認し、文字出力や整数操作を `hvc #0` で EL2 に依頼する。
EL2 のベクタ位置 `0x400`（下位 EL の AArch64 同期例外）に入り、
`ESR_EL2` の EC=0x16 と即値0を確認して操作を処理する。
HVC は通常の呼び出しとして扱い、例外ログを出さない。BRK や IRQ のログは維持する。
保存した CPU 状態を復元し、`eret` で HVC の次の命令に戻る。
EL1 側は移行先が EL1 であることを確認し、`Returned from HVC at EL1` と出力する。
試験用の呼び出し規約は次のとおり。即値0を共通の入口にし、操作番号はレジスタで渡す。

| レジスタ | 呼び出し時 | 復帰時 |
|---|---|---|
| x0 | 操作番号（1: 整数を1増やす、2: 1バイト出力） | 状態（0: 成功、1: 未対応、2: 不正引数） |
| x1 | 整数引数 | 結果（文字出力・エラー時は0） |

x2〜x30 と PSTATE は保持する。加算は64ビットで折り返す。
EL2 側は保存領域を `ExceptionFrame` として参照して x0・x1 を変更し、復帰時に結果を渡す。
構造体のサイズと ELR / SPSR の配置はコンパイル時に検証する。
EL1 側は `hvc::increment(value) -> Result<u64, hvc::Error>` を使い、呼び出し箇所に `unsafe` は不要。
HVC のアセンブリと状態コードの解釈は `src/hvc.rs` 内にまとめる。
EL1 から `41 → 42`、`u64::MAX → 0`、未対応操作のエラー、その後の正常呼び出しを確認する。
EL1 の通常出力は `hvc::write_str(&str)` / `write_bytes(&[u8])` を使い、UART の操作は EL2 が行う。
操作2は引数0〜255だけを受け付ける。文字列は UTF-8 の各バイトを整数で渡し、EL2 は EL1 のポインタを読まない。
途中でエラーになった場合、それ以前のバイトは出力済み。1バイトごとの HVC なので大量出力向けの最適化は未実装。
不正な引数256の拒否と、その後の日本語出力も確認する。


```text
HVC result: 0x000000000000002a
HVC result: 0x0000000000000000
Unsupported HVC returned an error
HVC works after an error
Invalid console byte rejected
Console via HVC: Rustからこんにちは
```

これは今回決めた試験用の取り決めで、AArch64 共通の API や正式な OS API ではない。
同じ物理メモリを使う最小テストで、Stage 2 による隔離やゲスト VM の実行は未実装。
EL1 の UART 直接アクセスを禁止する保護や、EL1 専用の panic 処理もまだ実装しない。
EL1 自身の例外ベクタはまだ設置しない。HVC は EL2 の例外ベクタで受ける。
`timer-irq-test` と同時に指定すると IRQ の受信・復帰後に EL1 へ移行する。
`exception-test` も指定した場合は BRK テストを優先し、EL1 へ移行しない。

### 参照

- [QEMU virt](https://www.qemu.org/docs/master/system/arm/virt.html)
- [QEMU virt のメモリマップ](https://github.com/qemu/qemu/blob/v9.2.0/hw/arm/virt.c)
- [Rust の AArch64 ベアメタルターゲット](https://doc.rust-lang.org/rustc/platform-support/aarch64-unknown-none.html)
