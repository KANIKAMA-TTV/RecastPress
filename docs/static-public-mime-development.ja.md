# 公開 page の固定 MIME 参照候補

既存の許可済み MIME だけを `PublicPage` の private `&'static str` に保持し、入力 String の余剰 capacity 保持と明示 page clone の MIME String copy を減らす開発変更。core 対象 21 件に続き、`target/static-mime-full-check.log` は exit 0 で完了した。全 8 fresh DB 各 62 契約、依存 Wasm publication、最終既存 11 checker も成功した。性能・RSS の未実測値を先取りしない。

## 公開 signature と受理範囲

`PublicPage::new(status: u16, content_type: String, body: Vec<u8>)` と `content_type(&self) -> &str` の公開 signature は維持する。caller は引き続き String を渡し、借用 getter を使う。private storage が static でも、公開 getter の lifetime 契約を新しい static 参照 API へ変えない。

legacy text／feed constructor は従来どおり status を先に検査し、200／404／410 だけを受理する。その後、次の 4 MIME と byte 完全一致した場合だけ、対応する固定 literal を保持する。

- `text/html; charset=utf-8`
- `text/plain; charset=utf-8`
- `application/rss+xml; charset=utf-8`
- `application/atom+xml; charset=utf-8`

case 変換、trim、一般的な MIME parse、追加 parameter の許容は行わない。未知値、余分な空白や parameter、別表記は引き続き `InvalidContentType`。status と MIME が両方不正なら従来どおり `InvalidStatus` が先に返る。

`new_image` は既存の typed `RasterImageKind` から PNG／JPEG／WebP の固定 MIME を保持し、status-first と既存 signature subset の検査順を維持する。legacy `new` に画像 MIME を渡すことを新しく許可しない。typed redirect も既存 status／Location と固定 `text/plain; charset=utf-8`／空本文を維持する。新しい MIME、画像 decode／CRC、encoding 判定、HTML 検査や出力安全性の検証ではない。

## 固定参照とコピーの境界

受理した String 自体の buffer を保存せず、既存値に対応する canonical static literal を選ぶ。文字列長が短くても大きい capacity を持つ入力 String を page の寿命まで保持しない。明示的な `PublicPage::clone` も MIME 参照をコピーするだけで、MIME String を新しく作らない。

caller が String を作る先行 allocation は残る。入力 String の drop は Rust 側の所有領域解放であり、その容量分がただちに OS RSS へ返るという保証ではない。image／redirect constructor の MIME String 化も省ける一方、本文の準備や Arc 化、Location、site／route 構築等の別 allocation は残る。

先行の [page metadata 共有](shared-public-page-development.ja.md)による route ごとの Arc allocation／refcount、[本文 owner](shared-public-body-development.ja.md)、cache insert copy／owner lease は維持する。cache key の representation String 等も別の確保である。HTTP header／Bytes owner Box／socket／transport／下流 deep copy をなくす変更ではなく、zero-allocation、RSS／peak memory、latency、CPU 上限を保証しない。値の固定化を一般の string interning や動的 MIME registry として提供しない。

## 維持する意味と対象確認

status／MIME の返却 bytes、bundle の strict admission と report、ETag の入力、HEAD／Range／redirect、exact route／query は変えない。cache の eligibility／monitor／通常 CLI の停止、sealed-startup、旧 response の inspect と stale delivery 拒否も従来どおり。固定 MIME の保持は公開承認・freshness・authorization・公開撤回の保証ではない。

`target/static-mime-source-fmt.log` の file 限定 fmt に続き、`target/static-mime-targeted-core-tests.log` で core lib 4、publication 6、raster 2、redirect 3、shared-page 3、新 static MIME 3 の計 21 件がすべて成功した。独立 source／API／error priority／domain／HTTP header・hash audit に blocking finding はなかった。

新 public 契約では全 4 text 値×受理 status 3、raster 3 種×受理 status 3、typed redirect 5 種を確認した。未知／case／trim／追加 parameter と legacy image MIME の拒否、status 優先、明示 Page clone の canonical reference も検証した。caller が 1 MiB capacity の String に短い許可 MIME を入れたケースでも、page が元 buffer でなく canonical reference を保持することを確認した。これは allocator の予約／解放量や OS RSS を測定した試験ではない。

旧 shared-page 契約は「constructor 前の入力 String buffer と一致」という内部保存の前提をやめ、受理後の page MIME pointer を基準に read／response clone の共有を確認した。本文・Location・strict admission や HTTP 値の期待を緩めたものではない。

全体 `target/static-mime-full-check.log` は exit 0。workspace Rust 589 件（ignore 1）、独立 binlog 29 件、Python 374 件（368 成功・symlink 権限不足 6 skip）、今回の 3 専用 crypto 比較の生成 10／legacy 113／credential stage 33、全 8 fresh DB profile 各 62 契約、Clippy、core／server の Linux cross-check、license file 215 観測、実 HTTP／process smoke が成功した。全 DB fixture は回収済み。正式 WP 75 API／508,297 と pure profile 300 件は source 不変の保持・current 確認であり、今回再比較・再実測した結果へ加算しない。

依存する固定 Wasm publication probe は native 6 件が `target/static-mime-wasm-publication-native.log` で成功し、HTTP も exit 0（`target/static-mime-wasm-publication-http.json` と同名 `.log`）。現 artifact の producer PID 17948／server PID 31460 で body 153／bundle 281／receipt 117 bytes、post_return と guest memory 変更後の所有 copy、capture 2 回と独立 8 項目一致を確認した。artifact SHA は JSON に記録し、所有 handle と一時 directory は回収済み。Windows の強制停止 exit 1 を graceful exit 0 と扱わない。

最終既存 11 checker は `target/static-mime-current-check-summary.json` ですべて exit 0。所有 controller PID 77911 も exit 0 で終了した。過去の shared-page／shared-body 成功や hash を今回の source 成功へ付け替えていない。Linux cross-check は native Linux 実行ではなく、固定 host 観測から Wasm compile／MSRV／security policy を推定しない。新しい依存・Wasm SDK・DB／認証／公開 HTTP route の採用を示すものではない。
