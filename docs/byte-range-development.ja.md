# 準備済み本文の単一byte Range

追加ETag対応後：元の5条件header全面fallbackは初版の履歴である。現在はIf-Match／If-None-Matchを先に評価し、通過したtag条件と一致するstrong If-Rangeだけprivate判断でRangeへ進める。日付presenceのFull fallbackは維持する。元headerを削除せずcachebypassも保つ。[現在の条件評価](etag-development.ja.md)。

2026-10-27時点。server内だけにsealed本文の部分応答profileを追加した。core／PublicPage／bundle／公開世代／cache ownerの契約は不変で、PHP／DB／job、外部取得、画像decoderへ接続しない。

## 受入範囲

GET・通常の準備済み応答が200・非空本文の場合だけ検討する。Range field一つ、元のfield値256bytes以下、ASCIIを要求する。unitは大小文字を区別しないbytes。field両端のSP／HTABだけを除去し、数字／hyphenの内部空白は受理しない。

- `bytes=first-last`は両端inclusive、末尾超過はclamp。
- `bytes=first-`は指定位置から末尾、`bytes=-suffix`は末尾の指定bytes。suffix過大は全文の206。
- 有効だがstartが本文長以上、または非空本文へのsuffix0は416。
- 降順、不正syntax、未知unit、multiple range、複数field、上限超過は従来のFull応答。

全桁のASCII確認と先頭ゼロ除去後、長さ／lexical比較でfirst／lastの順序を判定してからusizeへ飽和変換する。両値を先にMAXへ丸めて降順を見失ったり、巨大suffixでwrapしたりしない。256bytesはhost profileで、製品の手動容量設定ではない。

HEAD、200以外、空本文、health routeは従来応答を維持する。empty表現はRangeを無視し、正のsuffixを一律unsatisfiableにしない。[RFC9110 byte ranges](https://www.rfc-editor.org/rfc/rfc9110.html#name-byte-ranges)、[Range](https://www.rfc-editor.org/rfc/rfc9110.html#name-range)。

If-Match／If-None-MatchはETag層で先に評価し、304／412／400／431を選んだ場合はRangeへ進まない。評価を通過した要求と一致するstrong If-RangeだけRangeを許す。日付presenceやIf-Range不一致はFullを維持する。評価前のprivate Range guardをrequest headerの削除で回避せず、信頼した判断を別引数で渡す。date parserやconditional HTTP全体は未完成。[現在のETag profile](etag-development.ja.md)、[If-Range仕様](https://www.rfc-editor.org/rfc/rfc9110.html#name-if-range)。

## 応答と所有権

206は元MIME、部分のContent-Length、`Content-Range: bytes start-last/total`。416は空body、固定plain MIME、長さ0、`Content-Range: bytes */total`。no-store／nosniffを維持し、Accept-Rangesは今回追加しない。[single-part](https://www.rfc-editor.org/rfc/rfc9110.html#name-single-part)、[416](https://www.rfc-editor.org/rfc/rfc9110.html#name-416-range-not-satisfiable)。

offsetはraw representation bytesで、UTF-8文字や画像decodeの境界にはそろえない。Content-Encodingを生成せず、圧縮表現のRangeやmultipartは未対応。

初版の206 bodyはsealed sliceの新しいVec copyだった。後続の[共有本文owner候補](shared-public-body-development.ja.md)は、同じpayloadのArcをBytes ownerへ渡してsliceし、この部分Vec copyを削減する。非空sliceは元representation全体をpinし、新しい共有pool leaseにはしない。owner metadata／transport等は依然課金外で、RSS／peakの予算化を完成しない。206／416はcache.bodyを通さず、Fullへ戻るRange／条件付き要求も既存のeligibilityでbypassする。通常GETのcache契約は維持し、更新後の検証状況は専用記録で確認する。

共有化の対象74件と条件付きHTTP6件、retry全gate `target/shared-body-full-check.log` は成功した。旧copy由来のpointer非同一前提はpool0の共有と正のcache容量のcopyへ分けて修正し、初回失敗logは専用記録へ保持した。更新後の全8 fresh DB各62・HTTP等は実測済みで、3cryptoの現binary再観測後、最終11checkerも全exit0（`target/shared-body-current-check-summary.json`）。payload共有からtransport全体のcopy不在や公開撤回を推定しない。

method・Host・origin-form target・公開routeの検査を先に行い、wrong Host421、未登録404、POST等405、typed redirectのLocation／空bodyは変更しない。欠落をPHPへfallbackしない。

## 検証

private parser4契約で位置／clamp、短いdecimalの有限境界組合せ、巨大decimalと順序、256／257byte、不正／複数／非ASCII、HEAD／空／条件presenceを確認。HTTP新6契約で全256byteとUTF-8途中のslice、206／416 headers、5 redirect status、拒否経路、normalpool／pool0でのBudgetStats／cache状態保持を確認した。

2entryのA→B→Range A→CでRangeがLRUをtouchしないことと、retained Bytesのlease・最終drop／purge0も照合。local容量縮小がpurgeする既存仕様を見落とした試験setupの初回失敗は、縮小後にfresh ownerを用意するsetupへ修正した。source側で試験を緩めていない。

process smokeへ206／416、HEAD無視、If-Range fallback、multi／unknown unit、redirect維持を追加した。target/bad-protocol-and-byte-range-check.logでRust本体360件（通常ignore1）・Python112件、Clippy・Linux cross-check・license208観測・実HTTPが成功。core／DB／独立native sourceは変更せず、全current provenanceを照合して直前独立27件・5 fixture各39項目の実測を維持した。今回のDB再起動は0。WP58 API・386,865ケース／hooks572／dispatcher405は別の正式PHP比較で成功し、サンプルPNGの実206／416・HEAD・redirect維持も別途照合、全所有processを回収した。
