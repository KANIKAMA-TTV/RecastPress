# one-shot CLI の同期実行と server runtime の分離

2026-10-30 の開発記録。既存one-shotコマンドとhelpがTokio runtimeを構築せず完了できるよう、server binaryの入口を分離した。対象CLI５件に続き、guard付き全体gateと最終11checkerも全exit0（`target/oneshot-runtime-full-check.log`、`target/oneshot-runtime-current-check-summary.json`）。session95981と最終controller28341は出力回収後exit0を確認した。

## 既存操作を同期のまま処理する

[`main.rs`](../crates/recastpress-server/src/main.rs)の `main` を通常の同期関数にし、従来の `args_os` による次の分岐とhelpをruntime構築前に処理する。

- `prepare-posts`、`prepare-post-package`
- `bundle-receipt`、`check-bundle`、`check-package`
- `unpack-package`、`inspect-memory`
- `--help`、`-h`

新しいコマンドや公開APIは増やさない。各分岐のnative path、厳密な引数、出力bytes、固定診断、write／flush失敗、exit2／partial memoryのexit3、完成・partial fileを保持する既存挙動はそのまま。非UTF-8 native pathを扱う既存one-shotの `OsString` 分岐も維持する。serve用の従来String引数変換を、一般的なnon-UTF8対応へ広げる変更ではない。

public／managementの経路だけは、private `#[tokio::main] async fn serve(args: Vec<String>)` へ進む。既存macroのmulti-thread既定と `enable_all` を保持し、runtime内部でCLI parse→router／bundle admission→`shutdown::install`→async bind→ready診断→axum serve／graceful shutdownの順に処理する。Unixのsignal登録はruntime内かつlistener／readyより前のまま。manual Builder、global runtime factory、新たなthread設定、HTTP／cache／認証policyは追加しない。

## 固定providerの一次source

`Cargo.lock` が固定するTokio1.54.1／tokio-macros2.7.2のローカルCargo registry sourceを確認した。macroの `src/lib.rs` はmain以外の関数にも付与でき、同期関数へ展開して呼出しごとにruntimeを構築することを明記する。`src/entry.rs` の実展開はmulti-thread Builder、`enable_all`、build、`block_on`。したがってprivate serveへ同じattributeを移すだけで、server側のprovider設定を置換せず入口を分離できる。

Tokio1.54.1の `src/runtime/builder.rs` と `src/loom/std/mod.rs` では、既定worker数の解決時に `TOKIO_WORKER_THREADS` を読み、値0を拒否する。この固定providerの性質を、runtime構築に到達するかどうかの実process対照として利用した。環境変数は試験childへだけ設定し、親環境や製品設定を変更しない。

## 採用前後の実観測

採用前は現Cargo JSONから唯一のserver artifactを選び、childに `TOKIO_WORKER_THREADS=0` と `--help` を渡した。exit101、既知provider拒否marker、stdout空を観測した。source／binary前後SHA一致と回収を `target/oneshot-runtime-draft/before-observation.json` に記録した。生のpanic stderrや任意診断を観測logへ出していない。これは旧入口の期待された拒否で、採用後の成功試験とは区別する。

採用後の [`test_oneshot_runtime_cli.py`](../scripts/test_oneshot_runtime_cli.py) は５件すべて成功（5.013秒、`target/oneshot-runtime-native-contracts.log`）。Nodeを必須にせず、事前に実JSで生成した公開dummyの固定ZIPを [`scripts/fixtures/oneshot-runtime-package.json`](../scripts/fixtures/oneshot-runtime-package.json) から読む。次の22 childを実際に観測し、別に現artifact選択用のCargo childも所有・回収した。

- env0でone-shot７操作の成功、閉じた拒否９件、help２件。
- env0で有効なpublic／management引数を渡すと、両roleは従来providerのruntime拒否に到達しreadyを出さない。
- 通常環境でhelpとbundle検査の出力を比較する２child。

Windowsのmemory観測は、unsupportedを0や成功値へ変えず従来のpartial exit3を返した。準備・ZIP importの２file bytes、receipt、検査集計、失敗の固定stderrも確認した。通常環境での所有HTTP成功は既存smokeの別検証であり、env0対照そのものは正常serve成功の代用ではない。

対象観測 `target/oneshot-runtime-targeted-after-observation.json` は全５件・期待した実件数・全per-test cleanup・回収・source／binary／fixture／test hash一致を確認した後だけ生成する。partial選択、skip、assertionやcleanup失敗では成功観測を生成しない。testは公開 `TestCase.run` の戻り値を使い、private unittest outcomeへ依存しない。Cargo JSONと観測JSONの書込先はtarget固定。採用前observerの特殊CLIはtarget履歴にだけ残し、main試験へ持ち込まない。

## 依存実験と今回の検証範囲

別targetで現server artifactを選ぶWasmtime publicationの所有HTTP関係も成功した（`target/oneshot-runtime-wasm-http-observation.json`）。producer22504／server25740、body153／bundle281／receipt117 bytes、post_return完了とcopy後guest memory変更、capture２回／８比較true・changed header無しを観測し全回収した。producerの選択SHAは直前phaseと同一で、native６契約の成功は保持した既観測であり、今回は再実行していない。公開GETでWasmを呼ぶ機能へ変更したわけではない。

今回の全体gateはRust626（ignore1）、Python431（424成功・skip7）、認証３種actual、fresh alpha MariaDB／MySQL２mode各62、Clippy・server/WP Linux cross compile・通常dependency inventory218・HTTPが成功した。全体試験中にも22 childの実件数とsource／binary・回収を再確認した（`target/oneshot-runtime-after-observation.json`）。

pure313（62 artifact／features空／normal WP1）、dependency license file観測218、独立binlog29、Wasmpub native６はselected-pages checkpointの既観測保持で、今回はそれらのflagを指定して再実行していない。通常inventory検査218の今回成功と、license file実再観測は区別する。fresh DBはalpha２modeだけで、残る６modeは既証跡を保持した。全８DBのfresh再測定やWP正式79 API／519,014・hooks572／dispatcher405の実再比較は主張しない。

開始baseline23のうちalpha２＋認証３の５fileだけを実更新し、残り18fileのbytesを保持、選択source３file不変とDB directory空を実assertした（`target/oneshot-runtime-retained-identities.json`）。最終11checkerは全exit0、controller28341も回収済み。checkpoint `target/oneshot-runtime-checkpoint.json` はこのdocs freeze後にrootが生成する別操作であり、ここでは生成済みとは扱わない。

## 主張しない改善

除いたのはone-shot入口での無条件runtime構築である。latency、RSS、CPU、allocation数、OS thread数、energyの比較測定は行っていない。runtimeを作らないことからprocess全体のthread不在やsandboxを推定しない。IO・hash・admission・ZIP処理の確保や実行時間上限も変更しない。Linux native・Unix signal実動作の新規観測、session／principal／管理認証の完成には数えない。
