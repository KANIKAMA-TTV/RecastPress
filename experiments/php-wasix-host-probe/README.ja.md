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

source `98215a2`のtest専用実装は、公開`VirtualTaskManager::task_wasm`で`CreateMemoryOfType`を変更してからTokio managerへ渡す。後続実装で[共通policy](src/memory_policy.rs)と[WAT tests](src/memory_limits.rs)へ分けた。`build_memory`のoverrideだけには依存しない。対象は`env::memory`を1つだけimportする既知のshared memoryで、既存memory attach・内部定義・linker groupは受け付けない。

[Windowsの実測](evidence/memory-policy-windows.json)で、小さい固定WATを本物のWasiRunnerから実行した。初期403 pagesから1024 pagesへのgrowは成功し、capありの追加1 pageは−1、対照のcapなしでは1025 pagesになった。recycle callbackで実memoryのmaximumも1024／65536と照合した。初期値超過と既存／別経路の拒否を含む3件、既存FS2件、計5 unitが成功した。Cargoのtest artifactとソース・compile時PHP fixture・実行バイナリをSHAで結合した。

1024 pages=64MiBは試験値である。この観測時点はPHP経路未統合で、receiptの`php_memory_limit_verified=false`を維持する。後続の固定PHP試験は次節へ分ける。静的仮想予約・compiler・RSS・複数memory合計の上限ではなく、全guest／fork／直接instantiateの制御とも異なる。Watはdev依存だけに追加する。

## 固定PHPの上限・OOM・回復の追加試験

```text
python experiments/php-wasix-host-probe/memory_check.py --llvm --record
```

追加`--php-memory-cap`モードではWATと共通のsafe policyを使い、通常要求→256MiB capでの128MiB確保失敗→通常要求の回復→同確保の無cap対照を、同じコンパイル済みmodule・4つの新規環境で観測する。PHP自身のmemory_limit=-1の実出力、期待OOMの失敗、callbackの実max/size、無capの成功をすべて要求する。上限は製品設定ではない。

期待OOMの要求は`ok=false`と実行errorを保持し、4ケース全体の制御実証と区別する。PHPのmemoryはmetadataだけを読み、WAT用の先頭20B読取りは流用しない。既存の通常モード・製品メモリ上限のverdictは変えない。実PHPの正実行はLinux CIで別途確認する。

初回の[64MiB試験](evidence/ci-41302992753/provenance.json)は失敗した。source `7c57fad`の通常・過剰確保・回復の3ケースとも、スクリプト前のOPcache用128MiB shared segment確保で停止し、READYにも到達していない。実max=1024 pagesは観測したが、PHPコードによる期待OOM・回復は未実証である。無capの128MiB確保だけ成功した。元receiptを保持し、通常起動の余地を設ける256MiB試験へ分ける。PHPのOPcache設定と128MiB確保量は維持する。

続く[run 41305668144](evidence/ci-41305668144/provenance.json)、source `e9a2e6c`の256MiB試験は4ケースの期待をすべて満たした。cap付き通常/回復は2605 pages、最大4096 pagesを観測し、過剰確保はPHPのREADYと実`memory_limit=-1`を出してからOOMで失敗した。無cap対照は同じ128MiB確保に成功し6734 pagesに達した。失敗caseの`ok=false`を維持し、固定4ケースの線形メモリ制御だけを成功とする。RSS・compiler・合計上限、製品設定は未確定である。

## PHPからRustへの専用RPC

```text
python experiments/php-wasix-host-probe/rpc_check.py --llvm --record
```

固定`--rpc-contract`は同じPHP moduleを1回compileし、正常2要求→PHPが不完全frameを無視して終了する要求→正常回復の4環境を新設する。要求専用の`/rp-p0-rpc/channel`だけでRustへ渡し、cache miss→set7→get7→DB固定行→close/reopen後get7を照合する。各要求のnonce・GLOBALS・cache初期値と5 callbackを確認する。stdoutにはRPCに似た文字列とNUL/0xffを出し、呼出し数が増えず元byteのまま回収できることを要求する。

固定FSは1 channel、frame/write各128B、要求総量1024B、最大8操作、1応答1024B、応答読取り総量8192Bを上限とし、未知操作・未読応答への書込み・未完了frame等を要求全体の失敗として保持する。同じ応答の再読でも転送総量を消費し、seek/reopenで残量を戻さない。正常close/reopenはcacheを同要求内に維持し、新要求では空にする。不完全frameはPHPがexit0を返しても`finish()`で拒否し、当該caseの`ok=false`を維持する。

WASIXがI/O直前に行う現在offsetへのseekとゼロ長readは許可し、FSが受け取る任意位置変更を拒否する。ただしWASIXの`fd_seek(SET/CUR)`はdescriptorだけを動かす場合があり、guestの途中seekをすべて即時拒否する保証はない。空read・seek・metadata照会を含む全I/O回数の上限も別で、外側のprocess期限を残す。bad complete frameの拒否はRust unitの範囲で、実PHP4ケースには含まれない。

実DB・標準wpdb・WP bootstrap・製品ABIは未実装。追加時点のローカルでは全23 unit、release build、fmtと引数拒否を確認し、実PHP4要求は後続Linux CIで観測した。CIはRPCとmemoryの失敗後も他の独立比較を実行するが、失敗したstepを成功へ置き換えない。

同じ[run 41305668144のRPC実測](evidence/ci-41305668144/linux-llvm-rpc-contract.json)は受入`true/true/false/true`で通った。正常要求ごとにRust callback5回、入力90B、応答生成/実読取り76Bを照合し、stdoutのNUL/0xffを含むRPC風文字列はdispatchされなかった。PHPがexit0を返す未完了要求はRustが拒否し、次のfresh要求は正常に戻った。言語契約のFiberは引き続き不一致であり、このrun全体のCI verdictはfailureのまま保存する。

## 生成CGIの3要求

```text
python experiments/php-wasix-host-probe/cgi_check.py --llvm --record
```

`--cgi-contract`はCLI固定artifactの検証より前に別分岐し、同じCI jobで作った固定CGIパスだけを読む。build/tool/manifest/source/patch/config/実物の由来とbyte数・SHAを前後照合し、Rustもreceiptと同一の読取済みバイトをcompileする。任意path・PHP・query・env mappingは渡せない。これは同jobの来歴照合であり、署名付きattestationやbit単位の再現ビルドではない。

compile1回から3つのfresh CGI環境へcloneし、GET/Cookie、POSTとphp://input、404/2 Cookie/7B binary本文を比較する。正常終了・両pipeのEOF・出力上限を確認した後だけ、既存の[CGI parser](../php-wasi-host-probe/src/cgi_response.rs)へ渡す。FastCGI socketやHTTP listenerを通した実証ではない。

[native CGI比較基準](evidence/native-cgi-contract.json)は通常PHP8.5.12の配布78ファイルを検証し、実`php-cgi.exe`で3要求を実行したもの。CIのcheckerはその基準を検証して比較するがnative PHPを起動しない。SCRIPT_FILENAMEだけを明示正規化し、JSON型・値・status・Cookie・binaryは維持する。公開証跡ではローカル絶対pathを除き、元JSON byte数/SHAと正規化済み比較値を区別する。

新たな共通FS化後にrelease build・14 unit（FS2、memory3、CGI parser9）・fmtを確認した。native3要求と合成改変の拒否はWasmの実行成功へ数えず、CGI実物の3要求は専用CIで別に観測する。

[CI run 41302992818](https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/41302992818)はsource `7c57fad`でビルドからRust実行・native比較まで成功した。[原記録と由来の照合](../php-wasix-cgi-build/evidence/ci-41302992818/provenance.json)を保存し、GET/Cookie・POST/raw body・404/2 Cookie/binaryの3要求が一致した。compile1回は157,978ms、各`run_wasm`は69/70/65ms。HTTP listener、実DB、通常WP bootstrap、Fiber、製品性能の実証へは広げない。

同じ固定source・patchで作った前回と今回はWasmのサイズは同じでもSHAが異なる。今回実行したSHAを同jobのbuild receiptへ結び付けており、再現ビルドの成立は未確認である。builder自身の`guest_executed=false`と、その後の別Rust-host receiptでの実行成功を区別して保持する。

RPCを追加したsource `e9a2e6c`の[追試run 41305668059](../php-wasix-cgi-build/evidence/ci-41305668059/provenance.json)でも、同じ3要求のnative比較と所有process回収が成功した。初回の記録へ成功を付け替えず、各runの実物SHA・原観測を別保存する。

source `a0cbade`のCGI matrixは[Cranelift](../php-wasix-cgi-build/evidence/ci-41310853946-cranelift/provenance.json)と[LLVM](../php-wasix-cgi-build/evidence/ci-41310853946-llvm/provenance.json)の両方で3要求がnativeと一致した。Craneliftはcompile11,042ms、各`run_wasm`77/75/70ms、LLVMはcompile159,182ms、75/66/66msの単発観測だった。各jobがbuildした実物の同一性をそれぞれ確認し、backend間のbit同一性や製品性能の改善率は主張しない。

## 実験の制御範囲

host環境・home・host directoryをguestへ渡さず、networkはUnsupportedVirtualNetworking、HTTP clientは無し。stdinは閉じ、stdout/stderrは各64KiBを超える書込みをpipeへ入れる前に拒否する。EOF回収にも期限を設ける。

固定PHPの単一線形memoryと小WATの演算予算を検証した。実PHP全体のメモリ上限・CPU・実行内の期限の制御は受入未達。外側の所有プロセス期限と、virtual threadingの上限を、悪意あるPHPや全process spawnの制御の証明にしない。`unsafe_code=forbid`を保ち、任意のguest入力・製品listener・実DBへ接続しない。Node.js・native PHPをホスト実行の代替に使わない。

PHP_INT_SIZEは実行値を記録し、保存済みnative基準の8Bと別項目で比較する。名前の`-64`やWasm32という情報だけで合否を決めない。版の基本実行が通った後にも、例外・Fiber・権限照会・WP・要求I/O・RPC・異常回収・全配備先・依存と配布の監査が必要である。

## 小WATの演算予算と有限のテスト実行

```text
python experiments/php-wasix-host-probe/unit_check.py --record
python experiments/php-wasix-host-probe/unit_check.py --llvm --record
```

dev依存の`wasmer-middlewares=7.2.0`を使い、[固定WAT](src/metering_limits.rs)の正常→無限loop→正常を同module・3つの新store/instanceで実行する。各開始値10000、正常値42と正の残量、loopの実trap・`Exhausted`・到達状態、最後の初期予算復帰を照合する。WATごとに新Metering/Engineを用意し、別moduleへのmiddleware再利用はしない。hostcall importとstart sectionは無い固定対象である。

[Windows Craneliftの記録](evidence/windows-cranelift-unit-contract.json)では全25件（初回24件にadapter命令検査1件を追加）が成功し、通常の残量は9996、loopだけ実`Exhausted`になった。PHPを実行した記録ではない。Linuxにはformal EHのthrow/catchと、catchから入るloopの予算trapが外側のcatchに飲み込まれないことを確認する追加テストもある。Windowsではこれをcompile対象に含めず、Linuxでの結果を別に取得する。

無限loopの試験を通常の無期限test実行へ任せず、checkerが`cargo test --no-run`のJSONから選んだ所有test executableを、一覧10秒・本実行30秒で監督する。全件成功・ignoredなし・記録の型/順序/実残量・byte hash・source不変・子process回収を要求する。これはWasm演算の重み付き予算であり、PHP opcode・CPU時間・hostcallやcompile時間の上限ではない。

Linux CIはLLVMとCraneliftを独立したmatrix jobで比較し、片方の失敗で他方を中止しない。LLVMの実績をCraneliftの合格へ移さず、Cranelift jobではhost LLVM archiveを取得しない。各backendの結果と証跡名を分け、製品engineは選定保留のままにする。

source `de33e0a`の[Cranelift](evidence/ci-41308237911-cranelift/provenance.json)と[LLVM](evidence/ci-41308237911-llvm/provenance.json)は、両方ともLinux25件・formal EHの演算予算・PHP版・module再利用・RPC・256MiB診断・依存棚卸しを通過した。両方の言語比較は基本値/例外が一致、Fiberが不一致であり、CI全体はfailureのまま保持する。Cranelift側の再利用実験ではcompile6174ms、各`run_wasm`65/56/57msだった。backend差や製品性能の改善率をこの単発観測から決めない。

Fiber差分の次の調査は[小C context実験](../php-wasix-context-probe/README.ja.md)へ分けた。`--context-contract`はPHP CLI artifactを読まず、固定Cの同job生成物だけを使う。PHPのFiber backendは変更していない。

## 固定PHPのWasm演算予算

```text
python experiments/php-wasix-host-probe/operation_check.py --record
python experiments/php-wasix-host-probe/operation_check.py --llvm --record
```

`--php-operation-budget`だけにMeteringを適用し、公式PHP moduleを1回compileして、正常→READY後の無限loop→正常回復を3つの新規環境で実行する。試験値は100,000,000 pointsで、各instanceの開始値と終了時の実`Remaining`/`Exhausted`を公開safe APIから取得する。setup/recycleの観測は各1回、loopはREADY・Runner実error・実Exhausted・EOFをすべて要求し、caseの`ok=false`を保持する。

他modeのengineは非instrumentのままにし、snapshot再開・deep sleep・start sectionを使う経路はこの診断で拒否する。所有hostの外側期限は300秒で、期限切れだけでは予算による中断としない。Windowsでのビルド・25 unitとchecker改変拒否は確認済みだが、実PHP3ケースの正実行はLinux CI待ちである。CPU時間・PHP opcode数・hostcall待機・RSS・製品の予算値・Fiber適合はこの試験で確定しない。

## 宣言された依存ライセンスの棚卸し

```text
python experiments/php-wasix-host-probe/license_inventory.py --record
python experiments/php-wasix-host-probe/license_inventory.py --llvm --record
```

固定manifest/lockからLinux x86_64を対象とするoffline metadataを、所有processの60秒期限で取得する。normal/build/dev辺の到達集合を分け、packageの宣言licenseと別LICENSEの有無、入力不変、対象compilerを記録する。proc-macroのhost分類、実compile unit、最終リンク、法的な採用・配布可否の判定ではない。集合の重複を含むため件数を加算しない。

Windows上で取得した[Cranelift graph](evidence/windows-cranelift-dependency-inventory.json)と[LLVM graph](evidence/windows-llvm-dependency-inventory.json)を保存した。Cranelift側ではLLVM compilerにどの分類からも到達しなかった。Linuxでの実compileをこのmetadataで代用しない。`wasmer-compiler-llvm=7.2.0`は別LICENSEにBusiness Source License 1.1の独自利用条件を持ち、製品採用は未判定である。`saffron=0.1.0`も宣言文字列の代わりに別LICENSEを持つ。両者は配布crateのlock checksum・内部LICENSE・ローカル本文の一致とbyte/SHAを確認し、本文やhost絶対pathは証跡へ保存しない。

PHPの28静的archiveは[CGI build receipt](../php-wasix-cgi-build/evidence/ci-41302992818/build.json)にbyte/SHAがあるが、各archiveの元repo/commit・build flags・patch・個別license/noticeの対応は未確認で、全28本が最終リンクへ入ったとの証明でもない。`libwasix_rust_libs.a`等の内部依存はhost Cargo.lockでは追跡できない。PHP本体・Zend・TSRM、host LLVM22.1とcross-build LLVM21.1も別の来歴・配布確認対象として残す。

追加調査では[sendmailの旧recipe](https://github.com/wasix-org/php-wasix-sendmail/blob/8296c22d3fc83b2f245186681b1fc075d02441c7/Makefile)が`wasix-sendmail=0.1.0`のstaticlibをdepsへコピーすると分かったが、既存archive SHAとのbuild対応、`+wasix`のtoolchain版、当時のroot license宣言は未確認である。後年の別版の宣言を流用しない。`libicrt.a`は同梱iconv headerの1.17表記との関連が候補に留まり、`libwasix_rust_libs.a`の元repo/commit/recipeも未特定である。次はlinker map等で現PHP moduleへ採用されたmemberを分け、不要なarchiveまで製品依存と決め付けずに監査範囲を確定する。
