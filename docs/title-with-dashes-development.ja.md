# title-with-dashesの限定pure候補

全体checkpoint（2026-10-29）：`target/uri-title-and-credential-pipeline-full-check.log` exit0。Rust520＋独立29（ignore1）、Python241実行（240成功・1skip）、正式70 API461,522（新URI5920／title2538）、全8 fresh DB57、Clippy・server/WP Linux check・license215観測・実HTTP成功。旧68のinput/result SHAを実prefix再計算で保持、最終11checker全exit0（`target/uri-title-current-check-summary.json`）、全DB回収済。native Linux実行は未実施。

固定WordPress 7.1.3の`wp-includes/formatting.php:2282-2400`にある`sanitize_title_with_dashes`から、raw入力にliteral `<`が無く、strip後がvalid UTF-8で、mbstringが無い環境のbyte変換を切り出す。新native関数は`sanitize_title_with_dashes_utf8_no_open_angle_no_mbstring_bytes(raw, context, max_output)`。contextは`TitleDashContext::Display`／`Save`の明示enumに限定する。sourceの`raw_title`引数は関数内で未使用のためnative APIへ持ち込まず、PHP比較wireでは空文字へ固定する。

`sanitize_title`全体、`remove_accents`、filter、locale取得、slug一意化、空slugのfallback、投稿status／author／日付、URL routeの採用、DB保存、認証・公開へは接続しない。標準WPの新規投稿保存APIを実装したものではない。

## 固定sourceとの対応

| 固定source | このsubsetでの処理 |
| --- | --- |
| 2283 `strip_tags` | literal `<`をraw段階で拒否。この域ではNUL削除だけを行い、trimしない |
| 2285–2289 | `%HH`を`---HH---`へ保護、残る`%`を削除、`---HH---`を`%HH`へ戻す。元から入力にあったliteral sentinelも復元対象 |
| 2291–2295 | NUL削除後のwhole UTF-8を確認し、no-mbstringのためUnicode小文字化を行わず、`utf8_uri_encode_utf8_bytes(value, 200, false, usize::MAX)`を使う |
| 2298 | ASCIIだけを小文字へ変換 |
| 2302–2388 | Save時だけ下記の固定ordered replacements |
| 2392 | `/&.+?;/`を非再帰・最短一致で削除。LFをまたがず、bodyは1 byte以上必要 |
| 2393–2398 | `.`を`-`へ、whitelist `%a-z0-9 _-`以外を削除、残ったspaceをhyphenへ、hyphen連続を縮約し両端hyphenを除去 |

Saveのreplacement順は、4種類のencoded dash／NBSPをhyphenへ、7種類のHTML entityをhyphenへ、`/`をhyphenへ、40種類のencoded記号・結合文字・不可視文字を削除、14種類のencoded幅ありspaceをhyphenへ、encoded multiplication signを`x`へ、のsource順。各一覧をnative moduleの定数に固定し、各`str_replace`相当の結果を次のreplacementへ渡す。一括Unicode正規化やunordered辞書による代替はしない。

最後の`\s+`より先にwhitelistがあるため、元のTAB／LF／CR／VT／FFはhyphenにならず消える。`a<TAB>b`は`ab`、ASCII spaceの`a b`は`a-b`になる。NUL削除後のUTF-8 admissionは、例えば`c3 00 84`を`c3 84`として受け入れるが、残る不正UTF-8にLatin-1 fallbackを適用しない。`wp_strip_all_tags`候補はtrimを含むため、このstrip段階へ流用しない。

## 順序と上限

200はcleanup前のURI出力byte制限。Unicode characterはURI helperの単位で収まるものだけを出す一方、ASCIIの既存percent列をoctetとして保護する境界ではない。`a`199個の後に`%41`がある入力は`a`199個と`%`で止まり得る。URLとして有効なpercent encodingを保証する出力とは扱わない。200 spacesの後の`A`は、先にtrimして`a`へ変えず、200 spaceでURI出力が止まってからcleanupで空になる。`&a;`67個も200 byteで止まってからentity削除され、最後に`a`が残る。

mbstring無しの`Ä`は`%c3%84`であり、`%c3%a4`や`a`にはならない。Displayでは`a×b`は`a%c3%97b`、Saveでは`axb`。`a&NBSP;b`はASCII lowercase後にSave replacementへ入り`a-b`となる。literal sentinel `---41---`は`%41`へ復元され、`a`へdecodeはしない。

`max_output`は完成byte数だけに適用する。途中のsentinel展開やURI列が上限を超えても、それだけで早期拒否せずcleanup後に判断する。空になる入力はcap 0でも成功する。whole inputは先にsubset検査するため、200 byteの後にある不正UTF-8やliteral `<`を無視しない。

allocationはchecked長と`try_reserve_exact`を用い、完成Vecは同じfinal walkerのcount／emitで確保する。途中のallocationを含む`AllocationFailed`、subset拒否の`UnsupportedValue`、完成上限の`OutputLimit`という閉じたerrorだけを返し、partial output・raw入力・任意parser診断をerrorへ保持しない。Debug／Display／Error sourceから本文を出さない。final byte上限は入力・中間確保・RSS・peak・同時実行の上限ではなく、共有poolへも未接続。

## 検証の範囲

新public契約は、percent／literal sentinelの非再帰順、Save／Display差、Unicode小文字化・accent除去の不実行、entity最短／LF境界、whitelistとwhitespaceの順序、NUL repair／不正UTF-8／NoLiteral<、200 byte境界、cap 0・exact／one-less cap、入力不変とerror非漏洩を対象にする。private契約はordered literal群の各replacementと、sentinel中間形・確保失敗を対象にする。

PHP実比較と正式API registry／証跡は別担当のgateで確認する。この文書の作成だけでnative試験、PHP比較、DB fixture再実測や全体gateを成功扱いしない。実行されたcheckpointだけを後続記録へ追加する。

2026-10-29の限定native確認では、private 2件・新public契約6件、WP crate全targetのClippy（`-D warnings`）、担当Rust 2ファイルのrustfmtが成功。固定sourceから抽出したdelete 40項目・width 14項目はnative定数と順序込みで一致を確認した。これはPHP実比較・全体gate・変更後DB証跡の成功を含まない。

## 限定actual比較とprofile

新API70の2,538件とURI API69の5,920件、計8,458件の限定PHP比較は差分0だった。titleはdisplay/save両context、全ASCII（literal `<` を除く）、NULによるUTF-8修復、preexisting `---HH---`、percentの断片・200byte打切り・短いentity/LF、source Saveの4 octet dash・7 entity dash・40 delete・14 width・timesを含む。40 deleteそれぞれによる次pattern生成も加え、sourceのarray順を単一の同時置換や再帰削除へ置換しないことを検査する。先行source反例候補だった `&a;` 67回→`a`、199個のa＋`%41`→末尾`%`、TAB/LFはdashでなく削除、ÄはSaveでも`%c3%84`を実sourceで確認した。

oracleは実 `sanitize_title_with_dashes(raw, empty_bytes, context)` を呼び、outer `sanitize_title`、remove_accents、locale選択やfilterを呼ばない。raw_titleは未使用でも明示empty Bytesだけに限定する。NoLiteral `<` と**NULだけを除去した直後**のwhole UTF-8を事前検査し、percent除去後なら修復され得る `c3 25 84` 等もこのsubsetでは拒否する。noMbstring/両関連function不存在/C profile、実source hash、top-level call数・両context正件数をmetadataで厳密に照合する。title内URI呼出しはURIのtop-level comparison counterへ含めない。

型/domain/profile拒否64 processとwrapper/metadataのPython契約も確認した。旧68 APIの全入力・順序をSHAで固定して末尾追加し、正式比較時は旧prefixの両出力SHAも再照合する。70 APIs / 461,522比較は後続正式gateの予定値であり、今回の限定actual成功やnative契約から正式3proof、crypto relation、DB fixture、公開slug保存・一意性・status/date/hooksの完成を先取りしない。最終出力capと中間allocationの区別、一般HTML/URL安全性や公開policyを採用しない境界は維持する。
