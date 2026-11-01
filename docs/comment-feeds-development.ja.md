# 公開準備済みコメントデータのRSS／Atom出力候補

全workspace確認は`target/xml-utf8-and-comment-feeds-workspace-check.log`で成功した。新Rust5・独立XML4を含むworkspace402 Rust／Python141、HTTP・Clippy・Linux cross-check・license208・DB current hashを確認し、旧記事feed bytesも維持した。独立レビューもblocking指摘なし。WP比較は別のXML profile追加を実測で確認した60 API／409,524ケース・hooks572／dispatcher405であり、コメント出力の全WP互換を示さない。DB live再実行無しで独立29件・6 fixture各39項目の実測とcurrent hashを維持する。以下の初回sample試験失敗と追加前のgate予定は履歴として残す。

固定サンプルは[examples/comment-feed-bundle.json](../examples/comment-feed-bundle.json)。`comment_feed_probe`の実出力からRSS1,366 bytes／Atom1,689 bytesを既存版1bundleへ収めた。`target/comment-feed-example-http-final-check.log`と`target/comment-feed-example-http-observations.json`で実GET/HEAD、XML parse、ETag条件付き304、If-Range付き206、Host／未知query／method拒否と所有serverの回収を確認した。初回`target/comment-feed-example-http-check.log`は、空本文コメントまで要約と全文の差を要求した試験側の誤りで停止した。空要素の存在と、別々の値を持つ先頭コメントを分けて確認するよう修正し、serializerは変更していない。全workspace gateはXML normalizerの追加と合わせて別途行う。

2026-10-27。R32で採用済みのコメント更新feedへ向け、`recastpress_server::feeds::comments` に独立したpure serializerを追加した。既存の記事feedのmodel・`render`・`render_budgeted`と出力bytesは維持する。この候補はcallerが公開用に準備した情報をXMLにする部分だけであり、R32全体の完成やWordPress feed template全互換ではない。

## Modelと出力

`CommentFeedDocument` は既存記事feedと同じ形のid・title・home_url・self_url・description・author_name・updatedと、コメントentriesを持つ。`CommentFeedEntry` は既存 `FeedEntry` を所有し、追加の `content_html` と必須 `CommentReply { reference, href }` を持つ。`FeedEntry.summary_html` はRSS description用であり、full contentとは独立してcallerが渡す。不要な中間article documentへの変換・内容cloneは行わない。

RSSはdescriptionと `content:encoded` を別要素へ出力する。固定WP 7.1.3 `wp-includes/feed-rss2-comments.php:102–107` が根拠である。Atomは `summary` の代わりに `content type="html" xml:base="コメントlink"` を出力し、常に `thr:in-reply-to ref="指定reference" href="指定href" type="text/html"` を付ける。根拠は `feed-atom-comments.php:109–129` と [Atom Threading仕様](https://www.rfc-editor.org/rfc/rfc4685.html#section-3)。

refとhrefはそのまま明示入力であり、親が記事かコメントかを判定しない。親commentを検索せず、同じfeed内の存在も要求しない。返却XMLは、親との実際の関係や公開権限の証明ではない。サイト全体／記事別title、翻訳済み文字列、URL、本文範囲、件数もcallerの準備するデータである。

## 共有する入力契約と出力所有権

既存のURI・XML文字検査・XML writer・UTC日時・MIME・最終出力cap・Rendering owner構築を共用する。HTTP(S) ASCII URI限定、厳密percent/port/IPv6/fragment検査、1900..9999年UTC整数秒、非空metadata、entry IDのliteral一致による重複拒否を維持する。一般IRI/URN・URI正規化・host解決・宛先認可は行わない。RSSで出力しないreply metadataとAtomで出力しないsummaryも、両formatとも事前検証する明示subsetである。[既存feed契約](feeds-development.ja.md)

HTMLはcallerが公開用に準備した文字列をXML textとしてescapeする。RSSのcontent／description、AtomのcontentはXML parserから元HTML文字列として回収でき、CDATA／DTDは生成しない。属性内のCR／LF／TABも既存writerの文字参照処理を用いる。これはHTML sanitizationとは別であり、悪意のHTMLやPIIを除去する処理ではない。[Atom HTML content](https://www.rfc-editor.org/rfc/rfc4287.html#section-4.1.3)、[XML属性値処理](https://www.w3.org/TR/xml/#AVNormalize)

`render(document, format, max_bytes)` は全入力検証→共通traversal計測→最終cap→fallible allocation→同traversal出力で完全Vecかerrorを返す。`render_budgeted(document, format, max_bytes, pool)` は計測後に既存Rendering予約を取得してから最終Vecを確保し、既存move-only `BudgetedFeedOutput` を返す。課金はescape後XML長＋owner header。bytesが先、leaseが後にdropされ、外包Arcの最終dropまで同じleaseを維持する。old＋new同時保持も同じpoolへ課金される。

入力、allowance外の検証集合／URI作業、日時文字列、Vec capacityの丸め、allocator overhead、callerコピー、Arc nodeは未課金。capは最終出力長であり、process RSSや容量算式ではない。pool縮小によるdebtは保持中bytesの撤回を意味しない。失敗時のlease返却は既存poolの保守的poison契約に従う。

## 対象検証と固定fixture

新Rust integration契約5件で、二本文区別・XMLescape・入力不変、Atom base／feed外parent／空feed、全field検証先行、duplicate ID、escape後exact cap／1byte不足、pool0／1byte不足、旧新同時charge、他ResourceKind・縮小debt・Arc最終dropを確認した。既存記事feed契約6件とbudgeted契約8件も再実行して成功した。

新 `comment_feed_probe` は公開用固定2entryと空feedをRSS／Atomで出し、plainとbudgetedのbyte一致を確認する。独立Python XML試験4件は名前空間、RSS二本文、Atom content/base／明示reply、日時／MIME、空feed／CDATA無し、実pool観測と最終dropを解析する。既存記事XML試験5件も継続する。これらの標準serializer試験をWP API台帳・実PHP比較件数へ加算しない。

起動例は `cargo run -p recastpress-server --example comment_feed_probe --offline --locked --quiet`。stdoutは `rss`／`atom`／`empty_rss`／`empty_atom` のXML文字列、format別content typeとbudgeted owner観測のJSONである。self URLは `https://example.test/comments/rss` と `/comments/atom`。これは開発固定データであり、DB queryやCMS実データの代替ではない。

新endpoint・CLI flag・bundle・HTTP process起動はこの実装では追加しない。既存PreparedSiteへcallerが本文を登録する方法は変わらない。Linuxはcrosscheckだけでnative実行合格とはしない。全体gateの成功値はこの対象試験から先取りしない。

公開コメント選別、承認／非公開化／削除／password保護、親子種別推定、メール／IP等PII対策、authentication、DB、WP query/filter/locale、author URI、xml:langやsy拡張、WPのCDATA・GMT/+0000表記等のbyte一致、継続公開・失効・自動endpoint・監視／自動容量算式は未実装である。
