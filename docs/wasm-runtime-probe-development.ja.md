# Wasm runtimeの独立した実行候補調査

2026-10-29時点の開発実験。RustからWasmを作ってサーバーで実行する製品方針は決定済みだが、具体的エンジン、SDK／Component Model契約、theme ABI、公開描画への組込みは未確定である。この実験を採用済みエンジンやテーマ実行の完成へ数えない。[本来の成果物・実行契約](theme-module-contract.ja.md)。

## 候補と作業範囲

公式の[50.0.0 release notes](https://github.com/bytecodealliance/wasmtime/releases/tag/v50.0.0)ではRust 1.97を必要としており、本体のMSRV 1.94へそのまま加えられない。初期調査候補は正確に`36.0.19`へ固定する。同tagの[workspace Cargo.toml](https://raw.githubusercontent.com/bytecodealliance/wasmtime/v36.0.19/Cargo.toml)はRust 1.86を宣言している。[公式release方針](https://docs.wasmtime.dev/stability-release.html)では12の倍数のmajorを24か月のLTSとし、supported releaseへsecurity fixをbackportする。実験のlockfileと選択featuresを確認し、将来の更新・製品採用時は当時の配信状態を改めて確認する。

独立package `experiments/wasm-runtime-probe/`と独自Cargo.lock／target directoryを用い、本体workspace、既存Cargo.lock、server／DB／WP crateへ依存を追加しない。Wasmtimeのdefault featuresを無効にし、core-moduleの限定実行に必要な`std`、`runtime`、`cranelift`、`wat`だけを調査する。WASI host、ファイル／socketのhost API、PHP、管理資格情報、cacheやHTTP serverへ接続しない。

core moduleでの候補呼出しを調べる作業であり、製品のComponent Model設計をcore module ABIへ変更する決定ではない。入力は実験内で所有する固定の公開canary WATだけで、利用者の任意コードや外部artifactを受け入れる入口は作らない。エンジンのcompileを公開GETから起動しない。

## 確認する境界

このphaseの全Python回帰は290実行／286成功／Windows symlink権限4 skip、exit0（`target/wasm-runtime-probe-python-suite.log`）。既存11証跡checkerも全exit0（`target/wasm-core-current-check-summary.json`）。観測時の実験source／lock／checkerのSHAは`target/wasm-runtime-probe-checkpoint.json`へ記録した。production側sourceと既存PHP／DB証跡は維持し、CMS全体を再build・DB再起動した結果ではない。

固定のmemoryとtyped render exportから、safeなmemory readで有限のbytesを取得する。pointer／lengthの負値、範囲外、出力cap超過、必要export欠落、importを持つmoduleは成功へ変換しない。呼出しごとに新しいStoreを使い、mutableなguest状態を次の呼出しへ持ち越さない。

fuelとStore resource limiterはinstantiateより前に設定し、renderだけでなくstart functionの無限loopも対象とする。memory／tableの初期量・growth・Store内個数の境界を個別に観測する。これらはguest実行や指定したresourceの制限であり、compilerを含む全工程のwall time、process全体のRSS、host APIのCPU／IO、敵対的hostへの隔離を証明しない。固定の有限実験で一般のsandbox安全性・全theme互換性を宣言しない。

## 依存と結果の扱い

同tagの[Wasmtime crate manifest](https://raw.githubusercontent.com/bytecodealliance/wasmtime/v36.0.19/crates/wasmtime/Cargo.toml)は`Apache-2.0 WITH LLVM-exception`を宣言し、[LICENSE](https://raw.githubusercontent.com/bytecodealliance/wasmtime/v36.0.19/LICENSE)に例外本文がある。解決した推移的依存、選択features、実license fileの観測は別途必要であり、宣言1件で製品配布物全体のGPLv2互換性監査を完了したとはしない。

Windows nativeのRust 1.96で5契約が成功し、担当packageのfmtとall-target Clippyもexit0を確認した（`target/wasm-runtime-probe-{tests,fmt,clippy}.log`）。実行時27観測の内訳はoutput／range9、import／fuel4、状態5、resource9で、PHP互換APIの件数へ加算しない。同じinstanceではmutable stateが1→2となる対照を確認し、新しいStore／instanceでは毎回1へ戻る。

依存の限定観測は`target/wasmtime-probe-dependency-observation.json`。lockのregistry 101 packageすべてでarchive SHAが一致し、選択したlicense候補167 fileのarchive／local bytesが一致した。候補file無し16 packageとrust-version未申告13 packageも明示し、宣言MSRVで1.94超は無かった。実compiler 1.96での結果をMSRV 1.94の実コンパイル試験に読み替えない。`.cargo-checksum.json`は101件とも不在であり、選択license以外の全source一致を検証した観測でもない。production側の依存台帳と採用ライセンス選択は更新していない。

既設Linux Rust stdを使うcross-checkは、Wasmtimeのcustom buildが必要とする`x86_64-linux-gnu-gcc`が無くexit101で停止した（`target/wasm-runtime-probe-linux-cross-check.log`）。compilerやtoolchainを追加せず、失敗logを保持した。Linux cross-check成功、native Linux、三配備環境、Wasm SDK、実theme描画、認証後のCMS操作の完成は主張しない。

## 独立artifact実行harness

```powershell
python scripts/check_wasm_runtime_probe.py
python scripts/test_wasm_runtime_probe_gate.py -v
```

checkerは固定manifest `experiments/wasm-runtime-probe/Cargo.toml`、package／bin `recastpress-wasm-runtime-probe`、専用`target/wasm-runtime-probe`だけを指定して`cargo build --offline --locked --message-format=json`を行う。任意のartifact path、PID、guest WAT、file、command引数を受け付けず、tool installやonline fetchを行わない。

今回の完全なCargo JSON streamから、固定manifest・main source・bin名・kind・non-test profileと成功したbuild-finishedを検査し、既存のabsolute executableを一つだけ選ぶ。専用target外、曖昧な複数artifact、欠落・不完全なbuild結果を拒否し、既定pathにある古いbinaryへfallbackしない。

Cargo buildは600秒、選ばれたbinaryの直接実行は20秒をowned helperへ指定する。nativeのstderrが空、stdoutがprefix付き1行、版／profile／27観測／19 output bytes／fuel 10000／unpublished flagの型と値が厳密に一致し、process_pidが実captured child PIDと一致することを確認する。成功出力はcleanup retryを終えた後だけ返す。capture後にCargo JSON 16 MiB、Cargo diagnostics 1 MiB、native result 4 KiBを検査するが、capture中のallocation／RSS上限を保証する処理ではない。

Cargoと環境設定・tool wrapperは信頼するローカルhostの前提。unsupported target等で直接実行できなければ失敗し、別runnerで黙って代替しない。artifact pathとSHA-256は今回のローカル観測であり、署名、reproducible build、敵対的hostに対する真正性認証ではない。27はnative内の固定観測成功の集計で、assert文の個数や、外側checkerが各caseを独立再実装して検証したという意味ではない。5つのnamed Rust契約と実験sourceのreviewを併用する。

専用Python契約6件はexact artifact selection、wrong manifest／source／profile、曖昧・失敗・不完全build、actual PID／strict result型、固定commandとcleanup retryを検査する。unittest discoveryではRust buildを実行しない。短い実Python childのtimeout／回収だけを有限process契約として含む。

2026-10-29のWindows限定確認は、`target/wasm-runtime-probe-gate-tests.log`で6件pass、`target/wasm-runtime-probe-executable-observation.log`で実checker exit 0。Cargo選択artifactのSHA-256は`8c89687aff9db849aae17fc5887df7080cd65e32175cfb01e6b9976676413e32`、native報告PIDはcaptured PID 12996と一致し、27／19／10000の集計を受理した。後続rebuildでbinary hashは変わり得るため、この値を永続の許可hashとして固定しない。Linux実行・全体Python gate・製品engine採用・公開pathへの接続の成功は、この確認に含めない。
