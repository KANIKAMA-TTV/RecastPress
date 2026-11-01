# 固定Component公開HTMLからsealed HTTPへの実験経路

後続の [selected-page consumer独立実験](selected-wasm-consumer-development.ja.md)は、guest実行をpublication lock外へ置き、最後のgateへ完成Vecだけを渡す別の有限実験である。こちらの事前生成→sealed HTTP経路を新SDKやrequest-time公開へ置き換えるものではない。

2026-10-29の開発用候補。所有する固定Componentが生成した公開用HTMLを、既存bundle／receiptへ準備して別のsealed serverへ渡す。任意の利用者module、theme／SDK／WITの採用、投稿選別、認証、自動公開・切替を実装したものではない。公開GETは準備済みbytesを配信し、GET中にWasm、compiler、PHP、DB、管理ジョブを起動しない。

## 分離と入口

`experiments/wasm-publication-probe/`は独立package／Cargo.lock／target directoryを持つ。Wasmtime 36.0.19の限定Component実行と、既存server libraryのbundle admission／receiptを候補として使う。root workspaceとCargo.lock、先行core／Component実験、製品server／DB／WP sourceは変更しない。依存の向きは実験から既存serverへ向かい、製品公開serverへWasmtimeを加えない。

```powershell
python scripts/prepare_wasm_publication_example.py target/wasm-publication-example
```

CLIの入力は未作成の出力dirだけ。guest、URL、module file、任意command、binary path、PIDの指定は受け付けない。成功時は固定summary JSON一行、失敗時は固定の`Wasm publication example failed`と改行、exit 2。sourceや出力path、HTML、provider診断は通常のerrorへ転記しない。成功メッセージの配送自体が失敗した場合、既に書いたstdoutや完成fileは撤回できない。

## 生成・検証・保存

固定UTF-8 HTMLをtyped `WasmStr`で受け、妥当性とbyte capを確認し所有bufferへcopyする。成功callの後処理を同じfunctionで完了し、guest memoryが後処理で変化しても所有したHTMLが元のままであることを確認する。後処理失敗を成功出力に変換しない。これは先行[Component実験](wasm-component-probe-development.ja.md)の境界を、小さな公開用canaryへ適用する作業である。

固定target `/wasm-fixture`、status 200、MIME `text/html; charset=utf-8`の版1bundleを作り、同じ完成payloadから既存`receipt_from_payload`でstrict admissionとlen／SHA receiptを得る。実DBのstatus、password、author、slugや非公開metaを解釈する変換は無い。固定HTMLの選定と、任意のサイト本文を公開してよいかという判断を混ぜない。

全生成・admissionが終わってから新規dirへ`bundle.json`と`receipt.json`をcreate-new／write-all／sync-allで保存する。既存entryは上書きせず、親dirは事前に必要。失敗時のpartialや完成fileは自動削除しない。2fileのatomic commit、Rust File dropのclose-error ACK、親dirの耐久性、敵対的な親path変更、署名・最新版・公開承認は保証しない。

## 所有HTTP観測の範囲

専用smokeはproducerとserverを別の固定Cargo manifest／target directoryへoffline／locked buildし、その回のCargo JSONが返す唯一のcurrent artifactを使う。古いroot debug binaryを流用・上書きせず、両artifactの観測を区別する。serverは生成packageを既存loader／receipt条件で受理してからlistenする。

fixtureはloopbackの空きportを一度取得してsocketを閉じ、同じportとHostで所有serverを起動する。間に競合が起きれば失敗として終了し、既存serverを使ったり停止したりしない。GETのbytes／MIME／長さ、HEAD、ETagによる304、異なるHostの拒否を確認する。さらに同じserver／URLを[HTTP capture](http-baseline-development.ja.md)で2回記録し、[offline比較](http-baseline-comparison-development.ja.md)の各boolとheader名差を検査する。exit 0だけで一致と判断しない。

smokeが作った一時資料だけは、所有server／reader／workerの回収後にfixtureとして削除する。利用者の通常CLIで作ったpackageを後段HTTP失敗時に消す処理は設けない。外部site、元WordPress、DNS切替、本番へのpublish、画面・JavaScript比較、Linux実機の試験にはしない。

## 限定結果と確認用package

Windows nativeのRust 1.96でproducer6契約・fmt／Clippyが成功（`target/wasm-publication-probe-{tests,fmt,clippy}.log`）。frontendはmock／localfileの7群中6成功・symlink権限1 skip、実guestと実HTTPをこの7群へ混ぜて数えない。全Pythonは303実行／298成功／symlink権限5 skip、exit0（`target/wasm-publication-example-python-suite.log`）。既存11証跡checkerも全exit0（`target/wasm-publication-current-check-summary.json`）。

一度のE2Eは`target/wasm-publication-example-http.log`／同`.json`へ記録した。固定HTML153 bytes、bundle281 bytes、receipt117 bytesが既存loaderを通り、producer PID26928の終了後に別server PID31620でHTTP条件を確認した。同URL capture2回は全8boolがtrue、changed header名は空だった。所有handle／reader／workerとtempは回収済み。serverのexit1はWindowsでfixtureをterminateした実終了値であり、HTTP失敗やgraceful shutdown成功へ読み替えない。

別途、確認用に`target/wasm-publication-example/bundle.json`と`receipt.json`を生成して残した（`target/wasm-publication-example-preparation.log`）。source／両artifact／確認用packageの観測は`target/wasm-publication-example-checkpoint.json`。このpackageは固定canaryだけで、通常CMSの公開承認や実サイトの取得物ではない。

新lockはregistry155＋local3の158 package。`target/wasmtime-publication-dependency-observation.json`では155 archive SHAのlock一致と、選択license候補255 fileのarchive／local一致を確認した。候補無し20、MSRV申告無し31、checksum JSON不在155件を明示し、法的採用や全source／linked binaryの成分証明とはしない。製品serverのnormal treeにWasmtime／DB／WP／KDFが無い別観測は`target/wasm-publication-server-normal-dependency-check.json`。固定pageが配信できても、実theme、認証付きCMS操作、動的な要求ごとのWasm描画が完成したとは扱わない。

次の例は新しい名前のdirへ生成し、Cargoが選ぶ専用server artifactを使って手動で起動する。前のdirは上書きしない。serverを終了するときはそのterminalでCtrl+Cを使う。

```powershell
python scripts/prepare_wasm_publication_example.py target/wasm-publication-local
$wasmExampleServer = python -c "import sys; sys.path.insert(0, 'scripts'); from wasm_publication_example import build_public_server; print(build_public_server())"
& $wasmExampleServer public --listen 127.0.0.1:8080 --host localhost:8080 --bundle target/wasm-publication-local/bundle.json --receipt target/wasm-publication-local/receipt.json
```

URLは`http://localhost:8080/wasm-fixture`。自動の所有fixture再実行は`python scripts/smoke_wasm_publication_example.py`で、手動packageを操作せず別tempで実行・回収する。native Linuxや本番への常駐・公開は行っていない。
