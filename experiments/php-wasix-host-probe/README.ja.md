# v0.5 P0：PHP 8.5.12 WASIXのRustホスト比較

独立workspaceの候補実験。Wasmer 7.2.0とwasmer-wasix 0.702.0を固定し、[公式成果物](../php-runtime-candidate-probe/artifact.json)のサイズ・SHA-256を照合してから、固定プログラムだけを実行する。既定のversion-onlyと、追加の3ケース言語比較を区別し、WP・DB・製品互換性の成功へ広げない。

## Windowsでの実測

2026-11-01、Rust 1.96.0／Windows x64でCranelift候補のビルドは成功した。実行はmodule compile段階で`cranelift-codegen-0.133.0`のlowering内にindex out of boundsのpanicが起き、exit 101となった。PHPの版を確認するところには到達していない。[失敗証跡](evidence/windows-cranelift.json)を保持し、対応済みとは扱わない。

この実測とは別に、[Wasmer 7.2のMSVC EH実装](https://github.com/wasmerio/wasmer/blob/v7.2.0/lib/vm/src/libcalls/eh/mod.rs)にもpanicのstubが残る。コンパイラのpanicを直せば完全互換になる、という根拠にはしない。

```text
cargo fetch --locked --manifest-path experiments/php-wasix-host-probe/Cargo.toml
python experiments/php-wasix-host-probe/check.py --record
```

checkerはビルド済みartifactをCargoのJSONから選び、所有する子プロセスに外側300秒の期限を設ける。失敗・異常終了・結果なし・期限切れは失敗として記録し、checker自身も非zeroで終わる。hostのcache絶対パスは診断抜粋から除く。

## Linux LLVM比較

2026-11-01、[CI run 41293870063](https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/41293870063)が成功した。Rust 1.94.0／Linux x86_64／LLVM 22.1.8で、PHP 8.5.12の固定echoを実行し、PHP_INT_SIZE=8を観測した。通常PHPの基準も8で一致した。例外・Fiber・WP・HTTP・DBはこの実行には含まれない。

[取得したJSON証跡](evidence/linux-llvm-41293870063/linux-llvm.json)と[CI各段階](evidence/linux-llvm-41293870063/ci-stages.json)は、source commit `5fd037742b27f0b3b7b9096f92ea58ef091a4f2e`に対する履歴である。以降の実装変更へ成功を付け替えない。[importer](import_ci_evidence.py)で4064Bのartifact ZIPのSHAと元commitの各入力hashを照合した。実行バイナリそのものは取得していない。

[独立workflow](../../.github/workflows/php-wasix-candidate.yml)は、公式と同じWasmer 7.2.0のLLVM backendを使う。host LLVMは22.1系で、PHPを作るwasixcc側のLLVM 21.1.206とは別。[llvm-toolchain.json](llvm-toolchain.json)で取得物のサイズ・SHA-256・版・配置を固定する。

```text
python experiments/php-wasix-host-probe/check.py --llvm --record
```

CIはupstream PHP artifactへの読取アクセスとCargo.lockの取得を先に確認し、その後に大きなLLVMを取得する。Rustは1.94.0、ビルドは2 job、全体は45分まで。必要なtokenはPHP取得stepだけに渡し、外部へ継承・出力しない。uploadするのは証跡だけで、PHP・LLVMバイナリは再配布しない。CI段階ごとの失敗も`ci-stages.json`に残す。

## 固定言語契約

```text
python experiments/php-wasix-host-probe/language_check.py --llvm --record
```

言語比較のselectorは`--language-contract`で、パス・PHPコード・任意引数は受け付けない。同じ[native fixture](../php-runtime-candidate-probe/fixtures/runtime_contract.php)をcompile時のsnapshotとして取り込み、SHA照合後に`/fixture/runtime_contract.php`だけをreadonlyのメモリFSから渡す。読取りとseekを許可し、write・set_len・unlink・rename・link・作成を拒否する。

比較は基本値・例外・Fiberの3ケースに限り、型・警告・ケース内部の全フィールドをnative基準と照合する。WPとscratchのディレクトリは渡さず、元fixture内の残り3ケースのmissing-directory結果は未比較として保存する。readonlyの権限照会やWPが成功したとは扱わない。Runnerの他の仮想root書込みの禁止も未達である。

`compile_elapsed_ms`はmodule compile、`php_execution_elapsed_ms`は`run_wasm`内のWASIX/PHP起動・実行・終了の時間で、全体RSSや純粋なPHP処理時間とは異なる。旧version-only証跡には測っていない値を追加しない。

実装追加時にWindowsでFSの2単体テスト、locked/offline release build、fmtを確認した。比較器はnativeの入力hashを照合し、正常対照と、値・例外順・Fiber戻値・数値型の変更、重複JSON key・非有限数の拒否を確認した。これらはWasm上の3ケース成功を意味せず、Linuxの実行結果を別に取得する。

[CI run 41295645988](https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/41295645988)ではLinuxのFSテスト2件・version-onlyは成功したが、言語契約は失敗した。source `64c004f`の[原観測](evidence/linux-llvm-41295645988/linux-llvm-language-contract.json)を変えずに取り込み、[事後比較](evidence/linux-llvm-41295645988/case-comparison.json)で基本値・例外の完全一致とFiberの不一致を確認した。CI中はhostのFiber失敗で全ケース比較へ進まなかったため、事後の個別比較と元の失敗判定を区別する。

Fiberは`Cannot resume a fiber that is not suspended`を返した。期待は通常PHP通り維持し、3ケース全体・製品候補は合格にしない。`compile_elapsed_ms=155366`、`php_execution_elapsed_ms=61`は当該CIの単発観測である。cold compileと実行を分け、次は同じmoduleの再利用を実証する。

この観測後に比較器を変更し、hostが個別ケースの失敗を返しても、取得できた3ケースの一致・不一致をそれぞれ記録するようにした。元CIのraw JSONは変更せず、新実装でも全3ケース一致とhost成功の両方を要求する。

## コンパイル済みmoduleの再利用

```text
python experiments/php-wasix-host-probe/reuse_check.py --llvm --record
```

固定`--module-reuse`モードは、ループ外でPHPの`Module::new`を1回だけ呼び、`Module.clone`を3つの要求へ渡す。要求ごとにTokio runtime・WASIX runtime・WasiRunnerを新設し、GLOBALSの初期状態、3つの異なる環境キーのうち該当する1つだけが見えること、stdout/stderrの上限・EOF・終了を観測する。host directory・home・環境継承・networkは渡さない。

compile回数はコード内の明示した呼出し点の計数で、backend内部の全操作の計測ではない。runtimeの`shutdown_timeout`から戻ることを、全thread停止や全RSS返却の証明にしない。プロセス全体は外側checkerが監督する。Windowsのrelease buildと引数拒否に続き、Linuxで以下の正実行を取得した。

[CI run 41298369677](https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/41298369677)の[再利用実測](evidence/ci-41298369677/linux-llvm-module-reuse.json)では、compile呼出し1回・clone3回・新規要求3回が成功した。compileは83,476ms、各`run_wasm`は55/53/51msで、GLOBALS・個別環境値・EOFを照合した。同じrunの言語契約は基本値・例外一致／Fiber不一致のままであり、[全体判定](evidence/ci-41298369677/provenance.json)はfailureとして保存する。再利用の成立をWP bootstrapや製品性能へ広げない。

## imported shared memory上限の単体実証

test専用[memory_limits.rs](src/memory_limits.rs)は、公開`VirtualTaskManager::task_wasm`で`CreateMemoryOfType`を変更してからTokio managerへ渡す。`build_memory`のoverrideだけには依存しない。対象は`env::memory`を1つだけimportする既知のshared memoryで、既存memory attach・内部定義・linker groupは受け付けない。

[Windowsの実測](evidence/memory-policy-windows.json)で、小さい固定WATを本物のWasiRunnerから実行した。初期403 pagesから1024 pagesへのgrowは成功し、capありの追加1 pageは−1、対照のcapなしでは1025 pagesになった。recycle callbackで実memoryのmaximumも1024／65536と照合した。初期値超過と既存／別経路の拒否を含む3件、既存FS2件、計5 unitが成功した。Cargoのtest artifactとソース・compile時PHP fixture・実行バイナリをSHAで結合した。

1024 pages=64MiBは試験値である。PHP経路にはまだ適用せず、`php_memory_limit_verified=false`を維持する。静的仮想予約・compiler・RSS・複数memory合計の上限ではなく、全guest／fork／直接instantiateの制御とも異なる。Watはdev依存だけに追加する。

## 実験の制御範囲

host環境・home・host directoryをguestへ渡さず、networkはUnsupportedVirtualNetworking、HTTP clientは無し。stdinは閉じ、stdout/stderrは各64KiBを超える書込みをpipeへ入れる前に拒否する。EOF回収にも期限を設ける。

メモリ・CPU・実行内の期限の制御は未達。外側の所有プロセス期限と、virtual threadingの上限を、悪意あるPHPや全process spawnの制御の証明にしない。`unsafe_code=forbid`を保ち、任意のguest入力・製品listener・実DBへ接続しない。Node.js・native PHPをホスト実行の代替に使わない。

PHP_INT_SIZEは実行値を記録し、保存済みnative基準の8Bと別項目で比較する。名前の`-64`やWasm32という情報だけで合否を決めない。版の基本実行が通った後にも、例外・Fiber・権限照会・WP・要求I/O・RPC・異常回収・全配備先・依存と配布の監査が必要である。
