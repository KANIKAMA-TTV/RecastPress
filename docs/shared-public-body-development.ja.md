# sealed 公開本文の共有 owner 候補

準備済み公開本文を HTTP Body へ渡すときの payload copy を減らす開発変更。対象試験に続き、retry 全体 gate `target/shared-body-full-check.log` は exit 0 で完了した。全 8 fresh DB 各 62 契約と実 HTTP 等が成功し、全 DB を回収した。最終初回の 3crypto stale 検出後、現 binary の実再観測と最終 11 checker もすべて exit 0 を確認した。未実行の性能・メモリ試験を成功扱いしない。

## 既存 payload の共有

`PublicPage` が所有する不変な `Arc<[u8]>` に対し、`shared_body(&self) -> Arc<[u8]>` を追加する。実装は既存 Arc の clone で、従来の `body() -> &[u8]` も維持する。本文生成・bundle の decode・PublicPage 構築時の確保をなくすものではなく、準備済み payload を渡す経路だけが対象。

続く [page metadata owner の共有候補](shared-public-page-development.ja.md)は、private route storage と response の page object を共有する別段階である。この文書の payload 共有結果を、その metadata 変更後の検証結果へ付け替えない。

通常の未 cache 応答は、page の共有 Arc を `Bytes::from_owner` 経由で Body に持たせる。cache 不適格、monitor 未準備、local 停止、poisoned lock、cache admission 拒否等の fallback でも、応答用に再び本文の Vec を作る経路を共有 owner へ置き換える。page／site／router を破棄しても、実際に保持した非空 Body／Bytes はその owner の寿命に従う。

単一 byte Range の 206 は、同じ共有 owner から `Bytes::slice(start..end)` を作る。offset は従来どおり raw representation bytes で、UTF-8 文字や画像 decode の境界へ揃えない。部分 payload 用の Vec を作らない一方、短い非空 slice でも元 representation 全体の Arc を保持する。部分長だけの backing allocation に縮めたり、その長さだけの新 cache lease を作ったりしない。

## cache owner と予算の境界

この変更は cache 保存方式を共有 page の alias へ変えるものではない。eligible な cold miss の cache insert では、既存の Vec コピーと cache 本文の確保を維持する。insert が拒否されて共有 fallback へ戻る場合でも、その insert 引数の一時 Vec copy がなかったとは主張しない。legacy／budgeted の cache hit は従来の `CachedOwner` を保持する。特に budgeted hit は `BudgetedCachedBody` 全体を owner にして entry lease を保持し、bare Arc だけを取り出して予約を先に返さない。

Range は cache.body を通らず、cache eligibility の bypass、LRU、entry／pool charge を変えない。非 cache の共有 page owner は既存の cache entry charge ではなく、準備済み page payload と同じくその pool の課金外である。pool 拒否を HTTP failure や monitor 停止へ変換せず、従来の fallback を維持する。

`Bytes` の empty slice や empty Body 最適化は、owner を早期に解放できる。空応答に owner-backed 参照が残り、空 Body の最終 drop まで必ず Arc や entry lease を保持するとは保証しない。別の page／Body／cache owner があれば、それらの所有権は当然残る。最終の実 owner-backed 参照がなくなった時点の解放と、HTTP 応答 object の寿命は同じとは限らない。

## 削減するものと保証しないもの

削減対象は上記経路の本文 payload の Vec copy。Arc の refcount 操作、Bytes の owner Box／metadata、Body／header／Range の管理情報、socket／HTTP transport buffer、下流で利用者が作る deep copy は残り得る。allocation-free、完全 zero-copy、RSS／peak memory／CPU／送信量の改善や上限を保証しない。小さな Range が大きな元 payload を pin するため、保持時間と backing bytes は別途考える必要がある。

準備済み bundle の format／strict admission、status／MIME／Location／ETag と条件評価、GET／HEAD／206／416、Host／exact target、no-store／nosniff は変更しない。HEAD／304 等の空 Body を、本文 owner を延命するために payload 付きへ変えない。

cache の監視・再照合・eligibility・capacity・purge 条件も維持する。通常 CLI は実 monitor／自動容量と未接続なので、従来どおり保存・hit 停止。取得済み bytes を長く保持できることは、最新 DB 内容、公開 freshness、authorization、削除・非公開化の撤回保証ではない。sealed-startup 世代だけを読む制約を保ち、live publication／送信済み Body の失効へ拡張しない。

## 対象確認と全体ゲート

対象 native 契約は page と共有 owner の pointer／bytes 同一性、page／router drop 後の非空 Body／Range lifetime、raw byte slice、empty owner の早期解放、cache bypass／warm LRU／pool stats と既存 budgeted owner lease の保持を検査する。`target/shared-body-targeted-tests.log` では server lib 54、budgeted cache 10、Range 6、新 integration 4 の計 74 件が成功した。pointer 同一性の有限観測を、allocator／transport 全体の copy 不在の証明とは扱わない。

初回 Range 試験には、pool 0 の full fallback は別の本文 allocation になるという旧 copy 前提が残っていた。共有後は full fallback と Range が同じ sealed Arc を保持するため、pool 0 は pointer 同一、正の cache capacity では既存 cache insert copy と sealed Range が別 pointer であることへ期待を修正した。最初の失敗は `target/shared-body-first-old-pointer-assumption.log` に保持する。

続く最初の全体 gate でも、条件付き HTTP 試験の旧 pointer 非同一前提が見つかった。対応する 2 assertion を pool 0 同一／正の cache capacity 別、および repeated full はいずれも同一へ修正し、条件付き対象 6 件が `target/shared-body-conditional-tests.log` で成功した。`target/shared-body-first-conditional-pointer-assumption.log` は失敗記録として保持する。どちらも本文 bytes や HTTP 条件を緩める production 変更ではなく、意図した共有化へ試験側の allocation 期待を合わせた。

retry 全体 gate `target/shared-body-full-check.log` は exit 0。workspace Rust 581 件（ignore 1）、独立 binlog 29 件、Python 374 件（368 成功・symlink 権限不足 6 skip）、全 8 fresh DB profile 各 62 契約、Clippy、core／server の Linux cross-check、license file 215 観測、実 HTTP／process smoke が成功した。全 DB fixture は回収済み。

最終 checker の初回は正式 WP 75 API／508,297 と DB 系 7 checker が current の exit 0、一方 3 専用 crypto は stale binary の exit 1 となった。workspace test が source 不変でも example binary を置換する場合があり、今回も旧結果を現 binary の成功へ付け替えなかった。初回 summary／controller は `target/shared-body-first-stale-crypto-check-summary.json` と `target/shared-body-first-stale-crypto-check-controller.log` に、各失敗 log も別に保持する。

現 binary の password generation 10、legacy password 113（actual 88／refused 25）、credential stage 33（actual 25／refused 8）を実再観測し、すべて成功した（`target/shared-body-crypto-reobservation.log` と各 compare log）。「3crypto は今回再実測しない」という当初の予定は撤回し、hash の手動付替えではなく現 binary を実行して再確認した。正式 WP 75 API／508,297 は registry current の保持確認で、今回再比較していない。pure profile 300 件も source 不変で直前観測を維持し、今回の成功数へ加算しない。旧文書の初版 Vec copy と当時の gate は過去の記録として保持する。Linux cross-check は native Linux 実行ではない。

server を依存に持つ固定 Wasm publication probe は native 6 契約が `target/shared-body-wasm-publication-native.log` で成功した。依存変更後の HTTP 再確認も exit 0（`target/shared-body-wasm-publication-http.json` と同名 `.log`）。現 artifact の producer PID 28376、別 server PID 34460 で、body 153／bundle 281／receipt 117 bytes、post_return 完了と guest memory 変更後の所有 copy 保持を確認した。

GET／HEAD／304／異なる Host 拒否、同じ所有 URL の capture 2 回と独立 8 項目の一致（changed header 名は空）を確認した。所有 handle と一時 directory は回収済みで、Windows の強制停止 exit 1 を graceful exit 0 と扱わない。producer／server の実 artifact SHA は上記 JSON に記録する。旧 Wasm checkpoint を書き換えず、別の依存変更後観測として保持する。

今回共有するのは既に host が所有・admission 済みの公開 bytes。guest から host への所有 copy や mandatory post_return を削除する変更ではなく、main normal runtime に Wasmtime を追加していない。GET 中の guest 実行、engine／SDK 採用や本番配備の成功へ読み替えない。

最終既存 11 checker は `target/shared-body-current-check-summary.json` ですべて exit 0。所有 controller PID 15310 も exit 0 で終了した。最初の stale 検出と旧 pointer 期待の失敗記録は、最終成功とは別に保持する。

関連する既存契約は [byte Range](byte-range-development.ja.md)、[HTTP cache と owner lease](http-cache-development.ja.md)、[ETag](etag-development.ja.md)、[DB snapshot から sealed HTTP の有限経路](alpha-publication-development.ja.md)を参照。
