# RSS／Atom記事更新フィードの開発メモ

2026-10-26時点。採用済みの記事更新フィードに向け、公開用に準備済みのデータからXMLを生成するpure serializerを追加した。DB・PHP・ネットワークへの接続、記事の公開判定、エンドポイントの自動登録は行わない。

## APIと入力契約

`recastpress_server::feeds`の`FeedDocument`はid・title・home_url・self_url・description・author_name・updatedとentriesを持つ。`FeedEntry`はid・title・link・summary_html・author_name・published・updatedを持つ。文字列はRust String、日時はprivate fieldsの`UtcTimestamp`で表す。

`UtcTimestamp::new`はGregorian暦で1900～9999年、実在する月日、時0～23・分秒0～59を検査する。UTCのwhole secondsだけを扱い、うるう秒・小数秒・timezone変換は含めない。AtomはRFC3339のZ表記、RSSは曜日・英語月名とGMTを決定的に生成する。[Atomの日時構造](https://www.rfc-editor.org/rfc/rfc4287.html#section-3.3)、[RSSの日時仕様](https://www.rssboard.org/rss-specification)

`render(&FeedDocument, FeedFormat, max_bytes)`は完成した`Vec<u8>`か`FeedError`を返す。formatはRss2／Atom1。各appendで出力byte数をchecked検査し、上限ちょうどは許可、超過時はprefixを返さない。入力データ・重複検査・一時割当を含むRAM上限ではなく、手動の製品容量設定や自動配分算式でもない。

## 候補としての検証範囲

- feedのid・home／self URL、title・description・authorと、entryのid・link・title・authorを必須とする。title等は空白だけも拒否する。summaryは空でもよい。これは候補の入力検証で、標準が許す全ての入力を受け入れる契約ではない。
- metadataを持つ空feedは許可する。entry idの重複はliteral文字列一致で拒否し、URIの正規化・同一resource判定はしない。
- URL／idはabsolute ASCII HTTP(S)のみ。既存axumのUri解析に加え、percent tripletとpath／query／fragmentの構文を検査する。authority必須・userinfo不可、明示portはu16、角括弧内は実在形式のIPv6 literal。IPvFuture・scope付きIPv6・URN・一般IRIは未対応。fragmentは検証した元文字列を保持する。名前解決・宛先認可・URL canonicalizationは行わない。[URI構文](https://www.rfc-editor.org/rfc/rfc3986.html#section-3)
- 全textはXML 1.0 Charを検査し、NUL等の禁止controlとU+FFFE／U+FFFFを拒否する。許可範囲のUnicodeは保持する。`& < >`とquotesをescapeし、CRは文字参照、attributeのLF／TABも文字参照にしてXMLの改行・属性正規化で値を変えない。DTD／CDATAは生成しない。[XML Char](https://www.w3.org/TR/xml/#charsets)、[改行処理](https://www.w3.org/TR/xml/#sec-line-ends)、[属性値正規化](https://www.w3.org/TR/xml/#AVNormalize)

## RSS／Atomへの対応

RSS 2.0はchannelのtitle／link／descriptionとlastBuildDate、atom self link、dc:creatorを生成する。各itemはtitle／link／guid isPermaLink=false／description／dc:creator／pubDate、さらにatom:updated拡張を持つ。RSSのfeed.updatedはlastBuildDate、entry.publishedはpubDateへ出力する。FeedDocument.idはAtom用のmetadataで、RSSでも入力検証は行う。[RSS必須項目・拡張・guid](https://www.rssboard.org/rss-specification)

Atom 1.0は標準namespaceとfeed id／title／updated／author、subtitle、self／alternate linkを生成する。各entryにはid／title／updated／published／author／alternate linkとsummary type=htmlを出力する。[Atomのfeed／entry構造](https://www.rfc-editor.org/rfc/rfc4287.html#section-4.1)、[summary](https://www.rfc-editor.org/rfc/rfc4287.html#section-4.2.13)

summary_htmlはcallerが公開用に準備したHTMLという契約で、RSS descriptionとAtom summaryのXML文字列としてescapeする。XMLを壊さない処理とHTMLのsanitizationは別であり、このserializerは危険なHTML・非公開情報を除去しない。AtomのHTML text constructもmarkupのescapeを要求する。[HTML text construct](https://www.rfc-editor.org/rfc/rfc4287.html#section-3.1.1.2)

## 配信と検証

FeedFormat::content_typeは既存PublicPageが許可する`application/rss+xml; charset=utf-8`／`application/atom+xml; charset=utf-8`を返す。ホストがrender結果をPublicPageへ入れ、PreparedSiteの指定targetへ追加すれば既存公開HTTP経路で配信できる。CLIがfeed endpointを自動追加したり、WP queryから記事を集めたりする機能はない。[起動と既存バンドル](running.ja.md)

固定2entryのfeed_probe出力を含む開発artifactは[examples/feed-bundle.json](../examples/feed-bundle.json)。既存CLIの`--bundle examples/feed-bundle.json`で起動し、homepageの相対リンクから`/feed/rss`／`/feed/atom`を確認できる。両XML本文は独立parserで確認済み。これは固定本文のサンプルで、CMS query・実サイトの記事選別・連続生成ではなく、新しいCLI flagも追加しない。[具体的な起動手順](running.ja.md#標準feed-serializerの候補)

serializerのRust7件（Gregorian／曜日、XML属性roundtrip、必須要素、escape、禁止文字、URI subset、byte上限等）に、公開HTTPのMIME／GET／HEAD／Host試験1件を追加。独立したPython XML parserによる4件で、空feed・複数entry、namespace・日時・metadata・HTML summaryの復元を検査した。

`target/rss-atom-serialization-and-http-check.log`でworkspace Rust181件・Python48件、HTTP・Clippy・Linux向けcross-checkが成功。WP35 API・249,241ケース／フック572／dispatcher405は変更なしで、この標準serializerをAPI比較数へ加算しない。実PHP比較はtarget/html-class-filter-full-check.log時点で、後続はWPソース無変更のため再実行を省略してhash照合した。DBも再起動せず、直前のbyte-preflightログ（Rust152件＋独立experiment5件、両DB各36項目）を維持する。

完全なWP feed template／filter／query互換、公開記事の選別とprivate data対策、comment feeds、連続生成・公開世代切替は未実装。Linuxはcross-checkで、native実行の合格とは扱わない。

## 共有予算を持つ出力owner（2026-10-27、限定実装・検証済み）

新しいrender_budgeted(document, format, max_bytes, pool)は、同じprocessの明示BudgetPoolを受け取り、完成XMLのbytesとReservationを一つのBudgetedFeedOutputへ保持する候補。既存render、CLI、PreparedSite／HTTPへの登録方法、実監視未確認時のcache停止は変えない。公開GETで生成やDB／PHP実行を始める機能ではない。

入力全体をvalidateした後、既存と共通のXML traversalをcount-onlyで走査し、escaping・改行・日時を含む出力byte数とmax_bytesをcheckedで検査する。適合後にだけResourceKind::Renderingへ「出力byte数＋size_of::<BudgetedFeedOutput>()」を予約し、try_reserve_exactの後で同じtraversalから出力する。BudgetedFeedErrorはFeedのdomain／出力cap、BudgetのExhausted／Unavailable、出力Vec予約のAllocationFailedを区別する。失敗時に部分XMLを返さず、取得済みのreservationは破棄される。

ownerはas_bytes()の借用、format()、charged_bytes()だけを公開し、payloadを取り出すinto操作やClone／可変borrowを持たない。callerがArcでownerを共有した場合、最後のArcが破棄されるまで同じ予約を維持する。bytesがleaseより先に破棄される。old出力を保持したままnew出力を準備する場合は両方のchargeが必要で、pool縮小時のdebtも既存出力を解放したことにしない。

課金は出力lenとowner headerの論理量である。入力、BTreeSet／URI検査、日時String、一時writer、allocatorの丸め／余分capacity、callerのArc nodeやコピー、送信中のHTTP Bodyを含むRSS／peak上限ではない。cross-process調整、自動容量算式、cache接続、連続feed生成は未実装のままとする。

新integration8件とprivate2件で、RSS／Atomの既存bytesとの一致、escaping後exact cap／1byte不足、pool0／1byte不足、Arc最終drop、old＋new同時保持、縮小debt、他ResourceKindとの競合、全入力検証先行、予約後のwrite失敗を確認した。Vec capacityをisize::MAX超へする内部helper試験は実メモリを確保せずAllocationFailedとreservation解放を確定させ、usize::MAXのheader加算overflowも拒否する。OOM／実RSS試験ではない。

feed_probeは4文書について既存render bytesと照合し、同じbudgeted出力を独立XML parserへ渡す。poolの実観測をPython5件目で照合し、最初のArc破棄後にcharge保持・最後のArc破棄後0を確認する。比較用Vec／観測文字列のコピーは課金外であり、0という観測を全feedコピーの解放とは扱わない。独立レビューでもblockingな問題は見つからなかった。

target/budgeted-feed-output-final-check.logでworkspace Rust266件・Python89件、HTTP・Clippy・Linux cross-check・license208観測とDB証跡current hashを確認した。WP45 API・310,363ケース／hooks572／dispatcher405、binlog／overlapも別途current hashを確認し、PHP／DB live／独立experimentを再実行していない。直前の独立19件・3 fixture各39項目を維持する。初回target/budgeted-feed-output-check.logはexampleのrustfmt差分で起動前に終了し、整形後のfinal gateで成功した。
