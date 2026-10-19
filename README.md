# rustyhv

Rust のネイティブアプリと VM が共存する OS／ハイパーバイザーを目指す。
設計とロードマップは [DESIGN.md](DESIGN.md) を参照。
変更履歴は [CHAGELOG.md](CHAGELOG.md) に記録する。

## 最初の一歩: AArch64 でシリアル出力

現在の実装は、QEMU `virt` の単一 CPU で起動し、起動メッセージと現在の例外レベルを出力して待機するところまで。
`CurrentEL` レジスタを読み取り、EL2 なら例外ベクタを設置する。例外時は `EXCEPTION` と出力する。
`timer-irq-test` feature でタイマー IRQ の受信も確認できる。
タイマー IRQ からの復帰と、`el1-test` feature で EL2 → EL1 の移行・HVC の往復に対応。
`shell` feature で EL1 の簡易シェルを起動できる。
シェルの `run hello` で組み込み Rust アプリを EL0 で実行できる。
シェル構成では EL1 の MMU で EL0 を保護する。VM の実行とヒープは未実装。
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
rustyhv
CurrentEL: EL2
Timer frequency (Hz): 0x0000000003b9aca0
GIC version: 0x0000000000000002
GIC INTID limit: 0x0000000000000120
Timer IRQ configured (masked)
```

出力後は待機する。QEMU の終了は `Ctrl+A` を押した後に `X`。
`mon:stdio` を使うため、`Ctrl+C` はゲストのシェルへ渡る。

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
`HCR_EL2.RW=1` で EL1 を AArch64 とし、移行時は MMU・キャッシュを無効にする。
シェル構成では EL1 の初期化で Stage 1 MMU を有効にし、キャッシュは無効のまま維持する。
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
| x0 | 操作番号（1: 整数を1増やす、2: 出力、3: 待機なし入力、4: 待機付き入力） | 状態（0: 成功、1: 未対応、2: 不正引数、3: 未入力、4: 受信エラー） |
| x1 | 整数引数 | 結果（文字出力・エラー時は0） |

x2〜x30 と PSTATE は保持する。加算は64ビットで折り返す。
EL2 側は保存領域を `ExceptionFrame` として参照して x0・x1 を変更し、復帰時に結果を渡す。
構造体のサイズと ELR / SPSR の配置はコンパイル時に検証する。
EL1 側は `hvc::increment(value) -> Result<u64, hvc::Error>` を使い、呼び出し箇所に `unsafe` は不要。
HVC のアセンブリと状態コードの解釈は `src/hvc.rs` 内にまとめる。
EL1 から `41 → 42`、`u64::MAX → 0`、未対応操作のエラー、その後の正常呼び出しを確認する。
EL1 の通常出力は `hvc::write_str(&str)` / `write_bytes(&[u8])` を使い、UART の操作は EL2 が行う。
操作2は引数0〜255だけを受け付ける。文字列は UTF-8 の各バイトを整数で渡し、EL2 は EL1 のポインタを読まない。
操作3の引数は0。入力がなければ即座に未入力を返す。
シェルの `read_byte() -> Result<u8, Error>` は操作4を使い、入力が来るまで EL2 内で待機する。
操作4の引数も0で、シェル構成でのみ利用可能。
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
EL1 の入口で `VBAR_EL1` を設定する。EL1 の致命的な例外は原因を表示して停止する。
HVC は引き続き EL2 の例外ベクタで受ける。
`timer-irq-test` と同時に指定すると IRQ の受信・復帰後に EL1 へ移行する。
`exception-test` も指定した場合は BRK テストを優先し、EL1 へ移行しない。

### 簡易シェル

```sh
cargo run --features shell
```

EL2 で初期化し、EL1 に移行して次のプロンプトを表示する。
`shell` は `el1-test` を有効にするが、整数操作の試験に代えてシェルを実行する。

```text
rustyhv shell (EL1). Type help.
rustyhv> help
help       Show commands
info       Show system information
echo TEXT  Print TEXT
ps         Show the last process
run NAME   Run hello, fault, globals, globals-fault
Protection tests: read-kernel, write-kernel, read-device, write-code, exec-stack, stack-guard, exec-data
rustyhv> echo hello
hello
rustyhv>
```

- `help`: コマンド一覧。`info`: 現在の固定構成（EL1 シェル、EL2 コンソール、QEMU virt）を表示。
- `echo TEXT`: 引数を表示。先頭と末尾の空白は除き、途中の空白は保持する。
- `run hello`: 組み込み Rust アプリを実行し、終了後にシェルへ戻る。
- `run fault`: EL0 で意図的に BRK 例外を起こし、診断後にシェルへ戻る。
- `ps`: 直近のプロセスの PID・状態・終了理由を表示する。PID は16進数。
- Enter（CR・LF・CRLF）、Backspace / Delete、Ctrl+C に対応。Ctrl+C は入力行を取り消す。
- 入力は ASCII の印字文字、最大128バイト。超過した行は Enter 時に全体を破棄する。
- 空行は何も実行せず、未知のコマンドや不正な引数はメッセージを表示して入力へ戻る。
- UART の受信エラー時は行を破棄して入力へ戻る。通常の入出力に EL1 側の `unsafe` は不要。

履歴、矢印キー、日本語入力、外部プログラム、ファイル操作、VM 管理コマンドは未実装。
入力がない間は、EL2 の HVC 内で UART の受信通知を許可して `wfi` で待機する。
GICv2 の UART0 割込みは QEMU virt の INTID 33。シェルでは FIFO を無効にして1文字で起床させる。
CPU の IRQ はマスクしたまま、保留割込みによる WFI の起床を使う。
受信を再確認し、UART の通知を無効化してから GIC の受領・完了通知を行い、EL1 へ戻る。
通常の IRQ ベクタに入る処理や、複数タスクのスケジューリングは今回追加しない。
確認と WFI の間に文字が届いても IRQ が保留されるため、入力を見落として眠り続けない。
QEMU の終了は `Ctrl+A` → `X`。`timer-irq-test` と併用すると IRQ 試験後にシェルを起動する。
`exception-test` との併用では BRK 試験を優先し、シェルを起動しない。

対話の自動検証（Linux、Python 3.11 以降）:

```sh
python3 tools/test-shell.py
python3 tools/test-shell.py --release
```

実際の Cargo runner 設定で QEMU を起動し、コマンド・行編集・入力上限・復帰・終了キーを検証する。
1文字入力と20回の起床に加え、Linux の `/proc` で1秒間の待機中に QEMU が使う CPU 時間を検証する。

### 組み込み Rust アプリ

`src/process.rs` が単一プロセスを管理する。生成時に PID を割り当て、
Ready → Running → Exited と遷移する。実行は同期的で、アプリ実行中はシェルを操作できない。
終了済みの記録は直近1件だけ保持し、次の実行で置き換える。
`ps` は正常終了のコードと例外の ESR を区別する。通常終了コード128も例外と区別できる。
未知のアプリや引数エラーではプロセスを作成しない。
レジスタはアセンブリの保存領域で保持し、専用スタックは単一の領域を順番に再利用する。
スケジューラー・バックグラウンド実行・プリエンプションは導入しない。

```text
rustyhv> run hello
Hello from app!
rustyhv>
```

`src/apps/hello.rs` がアプリ、`src/app_api.rs` が最小のアプリ用 API。
`build.rs` が `user/main.rs` を OS と別にビルドし、読み取り専用のユーザーイメージを組み込む。
OS の debug / release にかかわらず、ユーザーイメージは最適化レベル2でビルドする。
入口は0x40200000、スタックは0x40400000から64 KiB。固定配置は QEMU virt 向けの今回の選択。
書き込み可能な `.data` と `.bss` は0x40600000からの64 KiBに配置する。
起動前に領域全体をゼロにし、OS が保持する `.data` の初期値だけをコピーする。
`run globals` は初期値を検証して書き換え、`run globals-fault` は書き換え後に例外で終了する。
どちらの終了後も、次の起動で `.data`・`.bss` が初期状態に戻る。
アプリは `app_api::write_str()` で出力し、`Result<(), Error>` を返す。
起動用の入口が結果を終了 SVC に渡す。成功は終了コード0、出力エラーはその状態コードで終了する。
アプリ側に `unsafe` や UART のアドレス・レジスタ操作は不要。
アプリの登録は `src/apps/mod.rs`、生成と終了の管理は `src/process.rs` が担当する。
未登録の名前や余分な引数はシェルが報告する。

現在はカーネルと一緒にビルドしたアプリを、専用64 KiBスタックで EL0t へ移して実行する。
EL1 のシェルの SP と callee-saved レジスタ・DAIF を保存し、終了 SVC で復元してシェルへ戻る。
出力は EL0 → SVC → EL1 → HVC → EL2 の順で UART に届く。
SVC ハンドラーは保存された SPSR が EL0t を示すことを検証する。

試験用の `svc #0` 規約:

| 操作（x0） | 引数（x1） | 動作 |
|---|---|---|
| 1 | 出力するバイト（0〜255） | x0 に状態、x1 に0を返して EL0 に復帰 |
| 2 | 終了コード | アプリを終了し、EL1 のシェルへ復帰 |

出力の状態は0: 成功、1: 未対応操作、2: 不正引数、3: コンソールエラー。
SVC の即値・番号・状態は今回決めた取り決め。FP/SIMD の保存はまだ含めない。
EL0 の同期例外は原因を表示してアプリを終了し、シェルに復帰する。
`run fault` は安全な `app_api::abort()` で BRK を起こす確認用アプリ。アプリ側に直接の `unsafe` は不要。
例外終了コード128（0x80）を予約し、EL1 自身の例外は従来どおり停止する。

```text
rustyhv> run fault
APP FAULT at EL0
Vector offset: 0x0000000000000400
Kind: Synchronous
ESR_EL1: 0x00000000f2000000
ELR_EL1: ...
App exited with status: 0x0000000000000080
rustyhv> run hello
Hello from app!
rustyhv>
```

OS とユーザーイメージを別々にリンクし、シェル構成では MMU でアクセス権を分ける。
ユーザーイメージの panic は終了コード255でプロセスを終了する。
外部ファイルからのロード、ヒープ、アプリ実行中の強制中断、マルチタスクは未実装。
対話テストでは実行・再実行・シェルの再開・不正な名前と引数を debug / release で確認する。

### 単一プロセスのメモリ保護

`src/memory.rs` が EL1 の Stage 1 MMU を設定する。QEMU virt・RAM 128 MiB・1 CPU 向けの固定配置。
仮想アドレスと物理アドレスは一致させ、ページのアクセス権を利用する。

| 領域 | EL0 | EL1 |
|---|---|---|
| OS のコード・データ・スタック・ページテーブル | アクセス不可 | 利用可能 |
| ユーザーイメージの有効ページ（0x40200000〜） | 読み取り・実行 | 読み取りのみ、実行禁止 |
| ユーザースタック（0x40400000〜64 KiB） | 読み書き、実行禁止 | 読み書き、実行禁止 |
| ユーザーデータ・BSS（0x40600000〜64 KiB） | 読み書き、実行禁止 | 読み書き、実行禁止 |
| GIC・UART | アクセス不可 | 操作可能 |
| ユーザー領域の未使用ページ | 未マップ | 未マップ |

ユーザーイメージの末尾側に1ページを残し、スタック直前の0x403ff000をガードページにする。
スタックの上側も未マップ。プロセス作成時にスタックをゼロ消去して再利用する。
専用アドレス空間を並列に切り替える仕組みはまだなく、1つのユーザースロットを順番に利用する。
ユーザーイメージの読み取り専用コードは組み込みアプリ間で共有する。

```text
rustyhv> run hello
Hello from app!
rustyhv> ps
PID NAME STATE REASON
0x0000000000000001 hello Exited code=0x0000000000000000
rustyhv> run read-kernel
APP FAULT at EL0
...
FAR_EL1: 0x0000000040080000
App exited with status: 0x0000000000000080
```

保護確認アプリは `read-kernel`、`write-kernel`、`read-device`、`write-code`、`exec-stack`、`stack-guard`、`exec-data`。
不正アクセスはアボートとして終了し、`ps` に ESR を記録する。各試験後に `hello` が実行できることも確認する。
これらは意図した命令を発行する検証用アプリで、通常のアプリ API は引き続き安全な Rust 関数を使う。
マルチタスクは保留。アプリが無限ループした場合、メモリ保護があってもシェルには戻らない。

### EL1 の例外確認

```sh
cargo run --features el1-exception-test
```

EL1 のベクタ設置後、起動時に `brk #0` を実行する。
シェルと HVC の通常試験より先に実行し、EL1 の例外ハンドラーで停止する。

```text
Testing BRK at EL1
EXCEPTION at EL1
Vector offset: 0x0000000000000200
Kind: Synchronous
ESR_EL1: 0x00000000f2000000
ELR_EL1: ...
```

`src/el1_vectors.S` に AArch64 仕様のベクタ表を置き、`src/el1_exception.rs` で設置と診断を行う。
下位 EL の AArch64 同期例外入口では CPU 状態を保存し、EL0 の SVC を処理する。
出力 SVC はレジスタと ELR_EL1・SPSR_EL1 を復元して EL0 へ戻り、終了 SVC はシェルへ戻す。
EL0 の同期例外は診断後にアプリを終了する。EL1 自身やその他の未対応例外は原因を表示して停止する。
`exception-test` も指定した場合は従来の EL2 BRK を優先し、EL1 へ移行しない。

### 参照

- [QEMU virt](https://www.qemu.org/docs/master/system/arm/virt.html)
- [QEMU virt のメモリマップ](https://github.com/qemu/qemu/blob/v9.2.0/hw/arm/virt.c)
- [QEMU の PL011 実装](https://github.com/qemu/qemu/blob/v11.0.0/hw/char/pl011.c)
- [Arm: 割込みのマスクと WFI の起床](https://documentation-service.arm.com/static/5ed6547eca06a95ce53f9321)
- [Rust の AArch64 ベアメタルターゲット](https://doc.rust-lang.org/rustc/platform-support/aarch64-unknown-none.html)
