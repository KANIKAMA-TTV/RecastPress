# 公開 page metadata の共有 owner 候補

公開 reader の read と response clone で、準備済み `PublicPage` の metadata object を共有する開発変更。対象 core 16 件に続き、`target/shared-page-full-check.log` は exit 0 で完了した。全 8 fresh DB 各 62 契約と実 HTTP、依存する固定 Wasm publication も成功した。最終既存 11 checker もすべて exit 0 を確認した。性能・メモリの未実測値を先取りしない。

## private storage と公開 API

`PreparedSite` の private map を `BTreeMap<String, Arc<PublicPage>>`、`PublicResponse` の private page field を `Arc<PublicPage>` にする。`PreparedSite::new` の入力は引き続き `(String, PublicPage)`、constructor／targets／read／response.page／deliver／Revision の公開 signature を維持する。呼出し側に Arc page の作成や新しい generation token を要求しない。

site 構築時に各 route の page を 1 個の Arc owner に包む。通常の sealed server では起動前の準備に当たる。read はその Arc を clone し、response clone も同じ metadata object を共有する。従来の read／response clone ごとの MIME String copy と、page 内の body／Location の nested Arc clone を省く。Revision の clone と page Arc の参照操作は残る。

この metadata Arc 段階では `PublicPage` の MIME field は String のまま維持した。当時、caller が `response.page().clone()` 等を選べば MIME String をコピーし、body／Location の所有参照を clone した。後続の[固定 MIME 参照候補](static-public-mime-development.ja.md)は、この String 保持と明示 clone の MIME copy を別段階で削減する。Location は既に `Arc<str>` を使う `OriginFormLocation` であり、metadata Arc 段階で新しく文字列共有へ変えた対象ではない。以下の gate／hash は当時の記録として保持する。

## generation と寿命

同じ generation の同じ route を複数回 read した response は、同じ page object を保持する。別 route は、値が同じでも route ごとに準備した owner である。pointer の一致は有限試験で共有を確認する手段で、opaque `Revision`、公開権限や永続 identity の代替ではない。

generation を置換／withdraw した後も、既に取得した response の `page()` で旧 metadata を検査できる点は従来と同じ。一方 `PublicReader::deliver` は既存の revision 比較と read lock を維持し、旧 response は `ReadError::Stale` で拒否する。共有 owner の保持を許可済み delivery や公開 freshness と扱わない。既に送信した bytes の撤回や、HTTP 返却後の poll ごとの認可は追加しない。

response とその clone が保持するのは選択した旧 page と revision token であり、旧 site の route map 全体ではない。他に所有参照がなければ、置換後の未選択 page は解放できる。選択した page も最後の response owner 等が消えれば解放される。

先行の [`shared_body()`](shared-public-body-development.ja.md) で取り出した body Arc は metadata page の所有者ではない。metadata の最終 owner を捨てても、その独立 body owner が本文を保持できる。逆に body owner を保持しただけで、MIME／status／Location の object を延命する契約にはしない。page owner と payload owner の Weak／drop を別々に検査する。

## 費用と維持する境界

route ごとの Arc allocation と refcount 操作を追加する。read の特定の field copy を減らしても、zero-allocation、低 latency、RSS／peak memory 削減、CPU 上限や allocator 全体の挙動を保証しない。HTTP header／Body／Bytes owner の metadata、socket／transport、下流の deep copy は別の層である。

元の本文・Location の準備や validation、strict bundle admission、exact target、status／MIME／ETag、HEAD／Range／redirect の応答契約は変更しない。cache insert 用 Vec copy、legacy／budgeted cache owner lease、monitor／eligibility、通常 CLI の cache 保存・hit 停止も維持する。公開 page metadata と Arc 管理情報は新しく shared pool 課金へ接続しない。

通常 server は sealed-startup の世代を扱う。core の置換／Stale 契約を試験することは、live publication、DB 同期、非公開化の失効期限、authorization、PHP／Wasm 実行や新しい管理 HTTP route を提供したことではない。

## 対象検証と全体ゲート

file 限定 fmt は `target/shared-page-source-fmt.log` で確認した。`target/shared-page-targeted-core-tests.log` は core lib 4 件（既存 2＋新 Weak 2）、publication 6 件、redirect 3 件、新 public 3 件の計 16 件が成功した。同じ route の page／MIME pointer と response clone の共有、同じ本文を共有する別 route の metadata object 独立、redirect の保持、generation 置換／withdraw 後の旧 response 検査と stale delivery 拒否、別 site の response に対する Stale を確認した。独立 read-only API audit に blocking finding はなかった。

private Weak 2 契約では、旧 generation の未選択 page が解放されること、選択 page は response clone の最終 drop まで残ること、独立 body owner だけ残したとき metadata Weak が失効して body Weak は残ることを確認した。これらは private storage の有限寿命確認で、任意 pointer の永続 identity や heap 全体の解放保証ではない。

全体 `target/shared-page-full-check.log` は exit 0。workspace Rust 586 件（ignore 1）、独立 binlog 29 件、Python 374 件（368 成功・symlink 権限不足 6 skip）、3 専用 crypto の生成 10／legacy 113／credential stage 33、全 8 fresh DB profile 各 62 契約、Clippy、core／server の Linux cross-check、license file 215 観測、実 HTTP／process smoke が成功した。全 DB fixture は回収済み。正式 WP 75 API／508,297 と pure profile 300 件は source 不変で直前結果を保持し、今回再比較・再実測した結果へ加算しない。

server を依存に持つ固定 Wasm publication probe も native 6 件が `target/shared-page-wasm-publication-native.log` で成功し、HTTP 再確認は exit 0（`target/shared-page-wasm-publication-http.json` と同名 `.log`）。現 artifact の producer PID 18508／server PID 9192 で body 153／bundle 281／receipt 117 bytes、post_return と guest memory 変更後の所有 copy 保持、GET／HEAD／304／Host 拒否、capture 2 回と独立 8 項目一致を確認した。artifact SHA と回収結果は JSON に記録し、所有 handle／一時 directory を回収済み。Windows の強制終了 exit 1 を graceful exit 0 と扱わない。

この観測は host 側で既に所有した bytes と page metadata の経路であり、guest→host copy／post_return を省くものではない。GET 中の guest 実行や engine／SDK 採用の成功へ広げない。旧 [本文 owner 共有の成功記録](shared-public-body-development.ja.md)を今回へ付け替えず、metadata 変更後の観測を別に保持する。

最終既存 11 checker は `target/shared-page-current-check-summary.json` ですべて exit 0。所有 controller PID 85375 も exit 0 で終了した。Linux cross-check は native Linux、pure profile の前回観測は Wasm compile／MSRV／security policy の検証ではない。
