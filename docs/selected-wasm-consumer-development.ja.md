# 選択pageを固定Componentへ渡す独立実験

2026-10-30 の開発記録。[同一世代の選択page](selected-public-pages-development.ja.md)を固定Componentの２引数へ渡し、guest実行と最後のfreshness確認を分離するconsumerを [`experiments/selected-wasm-consumer`](../experiments/selected-wasm-consumer/README.md) に採用した。新しい独立workspaceの６fileだけを追加し、root Cargo／lock、製品core／server、既存experimentのsourceは変更していない。

採用後のnative７契約、fmt、全target Clippyと、別processのcurrent-A main観測は成功した。当時の証跡を読む最終11checkerも全exit0で完了し、checkpointへ記録した。直前のsealed assets checkpointは保持し、workspace Rust636／Python436／DB／HTTPの全体試験を今回再実行したとは扱わない。

本書の観測は `target/selected-wasm-adopted-checkpoint.json` 保存時点の履歴である。後続の認証cookie検証MAIN変更は、このcore-only独立packageへのWP依存追加ではないが、当時の最終checker結果を現在のMAIN全体検証へ転用できない。最新MAINの全体再検証・source-bound証跡更新は未完了である。

## lock外で実行し、完成したbytesだけをgateへ渡す

hostは `/article?b=2&a=1` と `/navigation` を、既存 `read_selected` の明示件数・target byte上限で一度に選択する。入力は固定公開dummyだけ。Componentのcompileとwarmは選択前に完了する。選択のread lockを離れてから、保持したpage bodyをUTF-8と論理byte上限で検査し、fresh Storeへ２文字列を渡す。

guestは固定HTMLを組み立てる。hostはWasmStrのUTF-8・明示output capを確認してfallible copyし、同じtyped functionの `post_return` を完了させる。typed callが成功した場合は、UTF-8／cap／copyの拒否でもpost-returnを省略せず、post-return失敗を優先して結果を返さない。guestが出力memoryをXで上書きしarenaを戻した後も、コピーしたhost Vecが元のHTMLを保持することを確認する。

最後に `deliver_selected` が選択世代のfreshnessを確かめる。StateLockを持つ最終callbackは、完成済みVecをmoveするだけで、guest、compiler、imports、IO、publication再入を呼ばない。途中にB公開やwithdrawalが起きた場合、古いAの最終callbackは呼ばずStaleを返す。fresh B／Cの選び直しは明示した別操作である。すでにコピーしたAのbytesを消したり、将来のsocket配送を撤回したりする保証ではない。

## private checkpointは試験用だけ

唯一のhost import `fixture-checkpoint: () -> ()` は、固定guestがAの出力を組み立てた時点で所有controller threadへ通知する試験用同期である。別threadがB公開またはwithdrawalを行い、５秒以内のACKを返す。guestのhost callback内で同期的にPublisherを呼ばず、publication lockとguest実行を重ねない。

成功したthread完了をchannelで観測してからjoinする。ACKや完了の未確認は固有の失敗で、cleanup成功・current成功へ変換しない。このfixture importを製品host API、WIT、callback policyへ採用したものではない。WASI、host filesystem／network、WP／DB、任意module／path／visitor入力、HTTP listenerは設けない。

## 明示した実験限界

Wasmtimeは既存実験と同じ36.0.19、default featuresなし、`std/runtime/cranelift/wat/component-model`だけ。２引数用arenaは1024..5120の単調領域で、合計input4096 bytes以下を重ならないようにlowerする。固定出力領域は8192以降。個別body・合計input・output・fuel・memory・選択件数／target bytesはhostが明示する実験値で、製品の自動容量や全RAMの式ではない。

静的resource観測はmemory１、table０、初期memory１page。instantiate前にmemory65536 bytes、instance最大４／memory１／table１／table elements16とfuelを設定する。instance４はadapterを含む実験allowanceで、実際に４instanceを作ったという測定ではない。start／realloc／renderのfuel失敗、post-returnのfuel失敗、65535-byte memory capの不足を固定providerの原因型で区別する。コンパイル全体、host callback待機、全process RSS、CPU／時間／並行実行の保証へ拡張しない。

Weak試験は `Weak<[u8]>` のbody ownerを観測する。選択していないbodyは旧generation破棄で解放され、選択bodyはselectionのdropまで残り、完成HTMLはその後も独立して残る。この実験からprivate page metadataのWeak寿命まで測ったとは主張せず、既存coreのmetadata契約と区別する。

## prototypeと採用後の観測を分ける

target-only prototypeの７契約・旧main観測は履歴であり、pathを付け替えて採用後証跡とは扱わない。採用先で新たにnative７契約が成功し、fmt／全target Clippyもexit0（`target/selected-wasm-adopted-native.log`、`-fmt.log`、`-clippy.log`）。incremental cacheのhard-link／copy fallback環境warningは保持し、source lintやproviderを緩めて通したわけではない。

native７群はcurrent A、guest checkpoint中のB公開とStale／fresh B、withdrawalとStale／fresh C、選択bodyのWeak寿命、入力／出力cap・UTF-8・２引数arena境界、post-return／fuel／ACK失敗の優先、同じinstanceのcounter1→2とfresh instanceの1への復帰を扱う。

独立mainは別の現Cargo JSONから唯一の新manifest／source／bin artifactを選び、実PID36952でcurrent Aだけを実行した。format1／body51／unpublished-only true、stderr空・exit0、実PID一致を確認した。unexpected argumentの別PID38160はexit2・stdout空・固定stderr。build PID37004と両childを回収し、sourceとbinaryの前後SHA一致を確認した（`target/selected-wasm-adopted-main-observation.json`、`target/selected-wasm-adopted-main.log`）。これはmainから７契約全部やB／withdrawalを再実行した結果ではない。

`unpublished_only` は外部HTTP／deploymentを行わない意味で、内部fixtureのPublisher使用まで否定するfieldではない。返すのは固定の小さなmetadataであり、完成HTMLや秘密をstdoutへ出すAPIではない。

## lockと既存構成の保持

採用監査 `target/selected-wasm-adopted-audit.json` は独立packageの６fileとlockを記録する。registry118 packageは既存request probeのregistry146 packageのversion／source／checksum tupleの部分集合で、追加・変更は０。新lockをrootのproduction dependency inventory／license採用へ混ぜず、SPDX宣言やsubset一致を法律判断・完全な配布義務の証明へ変換しない。

既存453選択sourceはbytes同一を確認した。通常Cargo treeも別に実観測し、server51／DB100のnormal package構成に禁止対象の新edgeが無いことと、ROOTのSourceFreeze capture／verify成功を記録した（`target/selected-wasm-adopted-normal-graphs.json`）。独立experimentにWasmtimeがあることと、normal serverのrender経路へengineを採用したことは別である。

native Linux／Wasm guest Rust targetの導入・実行はしていない。認証principal／session、theme SDK／ABI、製品engine、製品HTTP render、公開選別・自動publish、socket撤回、性能・RAM改善は未採用／未測定のまま。別の `wasm-request-probe` に固定loopback HTTP実験はあるが、本consumerにはHTTP listenerがない。当時の最終11checker全exit0とdocs freezeを経て、`target/selected-wasm-adopted-checkpoint.json` を生成した。
