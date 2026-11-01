# 同一世代からの選択ページbatch

2026-10-30照合。本書のgate件数・current hash・最終11checkerは、`target/selected-pages-checkpoint.json` 保存時点の歴史的な観測である。その後の認証cookie検証MAIN変更後の全体再検証・source-bound証跡更新は未完了であり、現在treeの全成功を示さない。APIの現在の実装と当時の実行結果を区別する。

`recastpress-core::publication` に、callerが明示した複数targetを同一世代から選ぶ `PublicReader::read_selected` と、まとめてfreshnessを確認する `deliver_selected` を追加した。R61の世代混在防止へ向けたhost側の小さな境界であり、製品theme hostや実HTTPへの新規接続ではない。後続の[独立Wasm consumer](selected-wasm-consumer-development.ja.md)はこのAPIを利用するが、製品SDKではない。既存sealed serverは要求ごと1ページを読むため、現HTTPの不具合を直したとは扱わない。

## 選択契約

`SelectionLimits::new(max_pages, max_target_bytes)` の2値をcallerが必ず渡し、Defaultを持たない。`read_selected(&[&str], limits)` は空listを拒否し、件数とchecked合計UTF8 target byte数、既存 `valid_target`、`try_reserve_exact` をstate lock取得前に検査する。target数／長さのlimitは本文や保持metadataのbyte予算ではなく、capacity／RSS／CPU／同時実行の上限でもない。

targetはtrim・正規化・URL decodeせず、query字面と順序を保持する。同じtargetを複数指定した場合も件数・byte数へ毎回算入し、caller順に同じpage ownerを並べる。独自の重複排除や優先順位は加えない。

1つのstate read lockの下で全targetを検索し、選択pageの `Arc<PublicPage>` と共通 `Revision` だけを返す。missing等はall-or-nothingで拒否し、途中までのselectionを返さない。新しい `SelectionError` は `Limits`／`InvalidSelection`／`Allocation`／`Read(ReadError)` に分け、既存 `ReadError` のvariantを増やさない。

`PublicSelection` はprivateなpage集合を持ち、`revision()`／`len()`／`is_empty()` と `pages()` のExactSizeIteratorで借用できる。`try_clone()` は既受理件数の範囲だけをfallible reserveして選択ownerを共有する。無条件のVec cloneを公開しない。selection自体はwhole `PreparedSite` や非選択pageを保持しない。個々のpage ownerはbody／Location等を保持するので、本文の一部分だけを保持するものでもない。

## deliveryと寿命

`deliver_selected` は現stateのread lockを取り、共通Revisionと公開状態を1回検査して、bounded synchronous callbackを実行する。replace／withdraw後や別channelのselectionは `Stale` とし、callbackを呼ばない。lock取得失敗も閉じたerrorとして返す。古いselectionのbytesがinspect可能であることと、配信を許可することは別である。

callbackから同じchannelへ再入してはならない。PublisherだけでなくPublicReaderの再読取りも、writer待機中の再帰readでdeadlockする可能性がある。publication完了をcallback内で待つことも禁止する。lockを保持したままawait・guest実行・無期限処理へ進まず、deferred transportへこの検証を転用しない。すでにsocket等へ渡したbytesの撤回、非同期streamの全期間freshness、期限保証は追加していない。

## 固定consumerと検証

固定article＋navigationのhost-only consumerで順序付きpage bytesを組み合わせる。独立した2回の `read` の間へpublishを入れるpositive controlでは、A articleとB navigationを保持できることを確認した。これは別readの対照であり、現sealed1page HTTPのbugでも、実際のconcurrent batch race試験でもない。同一state lockのbatchでBの2pageを選び、replace後の旧A selectionはinspectできてもgroup deliveryが拒否されることを確認する。

別のlock保持契約はcallback内の同state `try_write` がWouldBlockとなること、publisherの試行後もcallback解放前には完了しないこと、解放後にA delivery／publish完了／B selectionへ進めることを確認する。同期は5秒のchannel `recv_timeout` とcompletion観測後のjoinを使い、sleepや無期限Barrierに依存しない。20msの未完了観測だけをatomicityの根拠にせず、lock所有の直接確認と併せる。fixtureのtimeoutは製品のlatency契約ではない。

UTF8 exact caps、query完全一致、重複順序、missing／invalid／未公開／poison拒否、replace／withdraw／cross-siteでcallback無し、選択ownerだけのWeak寿命も契約に含む。target prototypeは8契約（新6＋既存Weak2）が成功し、`target/selected-pages-draft/` にdiff／proof／観測を保持する。初positive controlのBarrier待機は独立reviewで指摘され、同じ契約をfinite channelへ置換して再成功した。

main採用後のcore crate native50契約（新6と既存全件）とClippyは成功した（`target/selected-pages-native-contracts.log`、`target/selected-pages-clippy.log`）。guard付き全体gateはexit0で回収済み（`target/selected-pages-full-check.log`、session75334）。Rust626（ignore1）＋pure313（62 artifact／features空／normal WP1）＋独立29、Python426（419成功・skip7）、認証3種の実再観測、全8 fresh DB mode各62、Clippy・Linux向けcross compile・license218・HTTPが成功した。native Linuxで実行したという意味ではない。WP formalの再比較や新API件数の追加は行わず、正式79 API／519,014・hooks572／dispatcher405の3証跡はbytes同一で保持した。

依存するWasmpubもnative6契約と所有HTTPが成功した（`target/selected-pages-wasm-native.log`、`target/selected-pages-wasm-http.log`、`target/selected-pages-wasm-http-observation.json`）。固定body153／bundle281／receipt117 bytes、post_returnとcopy後guest mutation、GET／HEAD／条件付き応答／Host拒否、capture2回と比較8項目trueを確認した。所有producer7776／public33056／check34224／receipt17748は回収後に不在、publicの所有停止exit1、handle回収と一時dir削除を確認した。このcore変更からGET中にWasmを実行する新経路は追加していない。

開始baseline23証跡のうち、今回実再生成したDB10＋認証3の計13が更新され、WP3＋依存7の計10はbytes同一で保持した。選択source2fileの一致、全disposable DB directory空も実assertした（`target/selected-pages-retained-identities.json`）。8 fixture modeと10 DB証跡JSONを同じ数として扱わない。当時の最終11checkerは全exit0（`target/selected-pages-current-check-summary.json`）、controller20541もexit0で回収済み。checkpoint `target/selected-pages-checkpoint.json` は文書freeze後に生成済みである。直前の固定ZIP import完成は `target/package-unpack-checkpoint.json` に履歴保持する。管理認証／ready503、DB編集公開policy、theme engine採用、native Linux実行の未完了範囲は維持する。
