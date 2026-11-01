# Sealed本文のETagと条件付きGET

2026-10-27時点。起動時に確定した公開200本文へstrong ETagを付け、GET／HEADのtag前提条件とIf-Rangeを扱う候補を追加した。PHP／DB／jobや外部取得は起動せず、公開世代は従来どおり起動前にPublisherを破棄して固定する。validatorは認証・署名、変更監視や公開撤回のleaseではない。

## 起動時のhashと所有

既存lock内のsha2 0.10.9を`default-features=false`でserverの直接依存にした。固定domain `recastpress-sealed-etag-v1`＋NULに続き、statusのBE2bytes／MIME bytes／raw bodyを、それぞれu64BE長＋実bytesとしてSHA-256へ分割投入する。連結Vecを作らず、結果はquoteで囲んだ小文字64hex、W/なしとなる。

同じprivate publisherで確定したreaderから起動時だけ200pageを読み、exact target→tag表を作る。readerとArc BTreeMapを同じprivate PublicStateへ保持し、要求時にhashを再計算しない。起動時lookup失敗や200targetのtag欠落を、空tag／別bodyへfallbackせず拒否する。GET／HEAD／206は元のfull表現の同じtagを使う。body／MIME／statusが変われば入力が変わり、同じ内容のquery別targetには同tagもあり得る。

target複製、tag表、Arc／HeaderValue等のmetadata、parserの4KiB stack scratchは共有pool／RSS計上の対象外。圧縮や表現交渉を将来追加する場合は、hash入力とvariant keyを改めて設計する必要がある。hashを秘密の保護や署名として扱わない。

依存辺とroot lockは実Cargo解決で更新し、inventory／license観測も再生成した。workspace155／probe188／unique208のpackage集合は不変だが、server binaryは既存crypto closureへ新たに到達する。`target/etag-server-dependencies.txt`ではDB／WP packageを含まないことを確認した。server-onlyの辺でもroot lockが証跡対象のため、DB／native証跡は実fixtureで再生成する。

## 条件の評価

既存のmethod・Host・origin-form URI・公開lookupを先に検査し、元のpageが200の場合だけ条件を扱う。順序はIf-Matchのstrong比較、If-None-Matchのweak比較、その後のGET Range／If-Range。

- If-Matchはwildcardまたはlist内のstrong一致で通過、不一致は空412。弱いtagは同じopaqueでもstrong一致にならない。
- If-None-Matchはwildcardまたはopaque一致で空304。W/が付いていてもweak比較で一致し得る。
- If-RangeはGETにRangeがある時だけ、単一のstrong tagが正確に一致すれば部分応答を許す。weak／不一致／date／不正／複数fieldはFull200へ戻し、412にはしない。
- HEADとRange無しではIf-Rangeを完全に無視する。redirect／404／410／health等は従来の応答を維持する。

If-Match失敗は後段のIf-None-MatchやIf-Rangeに優先し、If-None-Match一致もRangeへ優先する。Last-Modifiedを持たないため日付を評価したり起動時刻をmtimeにしたりしない。初版はdate条件fieldの存在時にRangeをFullへ戻す狭いprofileを維持するが、先行tag条件の304／412は適用する。一般の条件付きHTTP・date parser・書込み前提条件を完成したものではない。[前提条件順序](https://www.rfc-editor.org/rfc/rfc9110.html#name-precedence-of-preconditions)、[If-Match](https://www.rfc-editor.org/rfc/rfc9110.html#name-if-match)、[If-None-Match](https://www.rfc-editor.org/rfc/rfc9110.html#name-if-none-match)、[If-Range](https://www.rfc-editor.org/rfc/rfc9110.html#name-if-range)。

## Header grammarとhost上限

If-Match／If-None-Matchの複数fieldはcommaを挟んでraw bytesのまま組み合わせ、4,096bytes以下／64members以下をhost profileとする。境界commaとempty memberも計上する。合理的な空member／empty listを受理し、wildcardとlistの混在、閉じないquote、quote内space／C0／DEL等は400、上限超過は431。If-Rangeの4,096byte上限はGET＋Rangepresence時だけ検査する。

opaque内のcommaは区切りではなく、backslashはescapeではない。obs-textをrawで扱い、W/は大小文字を区別する。同じlistの後半まで構文を確認してから一致を適用し、一致した先頭だけでmalformed suffixを通さない。[entity tag比較](https://www.rfc-editor.org/rfc/rfc9110.html#name-comparison)、[listの受信要件](https://www.rfc-editor.org/rfc/rfc9110.html#name-recipient-requirements)。

元request headerは削除・書換えしない。304／412／400／431はcache.bodyを通らず、条件が通ったFull200／206も元headerを保ってcache eligibilityのbypassを維持する。既存cache entry／LRU／retained ownerのleaseを条件評価で動かさない。

## 304のアプリケーションとwire

Axum 0.8.9は空bodyのresponseへContent-Length0を自動補完するため、304には元200本文のfull長を明示する。bodyは空、ETagを付け、Content-Rangeを付けない。存在する304 Content-Lengthは元200長でなければならない。[RFC9110 Content-Length](https://www.rfc-editor.org/rfc/rfc9110.html#name-content-length)。

locked Hyper 1.12.0のHTTP/1送信では、GETの空bodyは明示Content-Lengthを省略し、304では0を再追加しない。HEADは元full長を残す。Axum oneshotのfull長と実wireのGET省略は異なる層の観測である。

`target/etag-304-wire-observation.json`では70byte PNGについてGET304はContent-Length無し、HEAD304は70、両方ともbody0／同ETagを確認した。最初のprocess smokeはheader必須を期待してKeyErrorとなり、`target/etag-process-smoke-check.log`へ保持。検査を「省略または元full長だけ」に修正し、非空表現の0は許可せず、`target/etag-process-smoke-final-check.log`で全実process試験が成功した。

## 検証状態

独立Python hashlibの値と一致するhash契約2件、条件parser6件、HTTP6件と既存Range6件が通過。quoted comma／raw octet／late malformed／上限、優先順位、304／412／206、normalpool／pool0でのcache状態・owner維持を確認した。既存cache試験は、旧malformed tokenの200期待を、条件通過後のbypassを検査するvalid tokenへ更新した。

target/protocol-once-and-etag-full-check.logでRust本体381件（通常ignore1）＋独立27件・Python113件、全5 DB fixture各39項目、Clippy・Linux cross-check・license208観測・実HTTPが成功。依存辺とlock変更後の全native／DB証跡を実fixtureで再生成し、全fixtureを回収した。WP59 API・394,212ケース／hooks572／dispatcher405も別の正式比較ログで成功し、current provenanceを最終照合した。target/etag-sample-observations.jsonではサンプルの独立hashlib一致、別process間の安定、実304とIfRange206も確認・回収した。live公開、monitor整合、送信済みbodyの撤回、一般HTTP／SSO認可は未完了である。
