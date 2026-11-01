# WordPress文字判定・HTML entity正規化の開発記録

最新のPHP比較（2026-10-27、日本時間）：55件目の `wp_kses_decode_entities` に7,646ケースを追加し、`target/byte-entity-decode-reference-check.log` で55 API・366,019ケース、hooks572／dispatcher405、current provenance照合が成功した。正式3証跡は実再生成済み。続くworkspace／nativeの2段gateでRust本体319件（通常ignore1）＋独立27件・Python112件、HTTP・Clippy・Linux cross-check・license208観測、既存4 fixture各39項目と専用切替再試行live39項目、全current provenanceも成功し、全5 fixtureを回収した。ログはtarget/byte-entity-decode-and-rotation-retry-workspace-check.logとtarget/byte-entity-decode-and-rotation-retry-native-check.log。下記54 API時の成功記録は履歴として保持する。

2026-10-27時点。固定WordPress 7.1.3の`valid_unicode`と、固定default名称・HTML文脈に限った`wp_kses_normalize_entities`をnativeへ移植した。53件目のpredicateは3,171件、54件目のnormalizerは6,527件を追加し、`target/unicode-and-default-html-entities-reference-check.log` で **54 API・358,373ケースの正式PHP比較、hooks572／dispatcher405、current provenance照合が成功**した。3種類の正式証跡を実再生成済みである。続く `target/unicode-entities-and-mysql-rotation-full-check.log` でworkspace Rust313件（通常ignore1）＋独立25件・Python111件、既存4 DB fixture各39項目、HTTP・Clippy・Linux cross-check・license観測が成功した。専用切替liveを含む全5 fixtureは回収済みで、全current provenanceも別途照合した。

この機能はHTML sanitizerや安全なescapingの完成を意味しない。製品HTTP・DB・site charset設定へは接続せず、XML文脈、`_wp_specialchars`、`esc_html`は未対応のままとする。

## 固定ソースとAPI

一次ソースは`.reference/wordpress-7.1.3/wp-includes/kses.php`。SHA-256は`ead6be7192295b9e7417c2146704c1935748f92b9ef4f6adca193a0e4118469d`で、registryとデータ抽出scriptが照合する。

| 対象 | 固定ソース位置 | Native API |
| --- | --- | --- |
| 整数の文字範囲判定 | `valid_unicode`:2346 | `valid_unicode_i64(i64) -> bool` |
| HTML entity正規化 | `wp_kses_normalize_entities`:2169 | `wp_kses_normalize_entities_default_html_bytes(&[u8], usize) -> Result<Vec<u8>, EntityNormalizeError>` |
| 名前集合・callback | default名称配列:639、`wp_kses_named_entities`:2229 | 固定借用tableへの厳密membership |
| 数字callback | `wp_kses_normalize_entities2`:2286、`wp_kses_normalize_entities3`:2316 | normalizer内部の数字処理 |

`valid_unicode_i64`はHT／LF／CR（9／10／13）と、`0x20..=0xD7FF`、`0xE000..=0xFFFD`、`0x10000..=0x10FFFF`だけを受理する。surrogate、BMPのFFFE／FFFF、負値、範囲外の整数はfalse。一方、FDD0..FDEFと補助平面末尾の非文字（1FFFE／1FFFF、10FFFF等）はtrueを保持する。Unicode非文字を一律に排除するpredicate、UTF-8 parser、文字列の正規化へ置き換えない。nativeはstateless・allocation-freeで、PHP側の非整数castや診断は対応しない。

## Default名称集合の限定profile

HTML正規化は、固定KSESソースの`!CUSTOM_TAGS`分岐にあるdefault 253名称だけを扱う。名称の大小文字を区別し、PHP coreや外部ライブラリのHTML entity表はコピーしない。動的に書き換えられたWordPress global allowlistの互換性は主張しない。

`scripts/wordpress_entity_names.py`はPHPを実行せず、指定された配列代入が一つだけ存在すること、内容が名前の文字列literalとcommaだけであること、callback grammar・重複なし253名称・source SHA-256・元順序の名称SHA-256を検証する。式、comment、重複・曖昧な代入等は拒否する。元順序のcompact JSON listのSHA-256は`60c1fcc052465d98ef468f0a6080aa422a457d5e77c9b7d8d72d40179c21c512`。

生成物は`compatibility/wordpress-default-html-entities.json`と、sorted borrowed byte tableの`crates/recastpress-wp/src/entity_names.rs`。`python scripts/wordpress_entity_names.py --check`は固定ソースから再生成した両fileのwireを照合する。ソースの読取りは1MiB、corpus用artifact読取りは64KiBで上限を設け、check時は期待する出力長＋sentinelまで読む。

このcheckを`compare_primitives.py`の入口と、対象APIを持つregistry検査から呼ぶ。generator・JSON artifactはprimitives／hooks／dispatcherの3証跡共通harness manifestへ追加し、Rust tableは既存の全WP source追跡に含める。これらの束縛を実装したことと、正式証跡を今回更新済みであることは区別する。

## 処理順と数字参照の境界

元関数は全`&`の退避、十進参照、十六進参照、名前参照の順で処理する。nativeはこの結果を元入力のampersandごとの非再帰scannerで再現し、新たに出力した文字列を再走査しない。名前だけでなく`;`も必要で、文脈判定は明示的なHTMLに限定する。元のNUL・不正UTF-8等は、ASCII entity変換の対象部分以外はbyteのまま保持する。

| 入力 | 出力 | 根拠 |
| --- | --- | --- |
| `AT&T` | `AT&amp;T` | 未認識のampersandを退避 |
| `&copy;` | `&copy;` | default集合の名前は保持 |
| `&AMP;` | `&amp;AMP;` | 大小文字を区別するmembership |
| `&#00065;` | `&#065;` | 有効な十進参照はleading zero除去後、最低3桁 |
| `&#X000D;` | `&#xD;` | matched hexはprefixを小文字xへ、hex桁の大小は保持 |
| `&#X00D800;` | `&amp;#x00D800;` | matchedだが不正な文字値は元の数字桁を保持 |
| `&#X0000;` | `&amp;#X0000;` | 全ゼロは元regexにmatchせず、Xも保持 |
| `&amp;#x2E;` | `&amp;#x2E;` | 既存の二重符号化を再帰decodeしない |

数字regexは非ゼロ桁を必須とし、先頭ゼロを除く十進桁は最大7、hex桁は最大6。全ゼロ、桁数超過、semicolon欠落等の**nonmatch**と、match後に`valid_unicode`がfalseとなるcallback結果を区別する。hexで不正な文字値だった場合もprefixは小文字xになるが、nonmatchでは元のXを変えない。十進7桁／hex6桁の範囲はi64で正確に扱える。長い先頭ゼロ列は桁数の上限へ含めず、その圧縮結果を出力長へ反映する。

## 最終出力のhost cap

同じscannerを用いてchecked byte長を計測し、callerのcapを照合してから`try_reserve_exact`し、もう一度走査して完全な出力を生成する。中間entity出力用のVecは作らない。たとえば数千桁の先頭ゼロが`&#065;`へ圧縮される場合、入力長や5倍の上界だけでcapを誤拒否しない。

`EntityNormalizeError::OutputLimit`は最終出力byte cap、長さoverflow、出力allocation失敗を含み、部分出力は返さない。capはprototypeのhost制約であり、PHPの失敗仕様、製品容量設定、allocatorが実際に確保するcapacity、RSS、共有BudgetPoolの予約を示すものではない。入力・metadata・固定table等の所有全体を予算化した機能でもない。

## Oracleと限定検証

PHP oracleは固定sourceから実normalizer・数字／名前helper・`valid_unicode`だけを抽出する。KSES全体をbootstrapせず、`CUSTOM_TAGS`、XML名称集合、tag globalsを初期化しない。privateな検証済み名称集合を各normalizer呼出し中だけ`allowedentitynames`へ設定し、finallyで元の存在・Null・値を復元する。globalが未設定、Null、custom array、custom string、falseの5状態について正常呼出し後と拒否後の保持を別途観測した。

normalizerではBytes-only・明示`context="html"`をPHP／nativeの両probeで検査する。PHPはPCRE状態をresetして実関数を呼び、エラーなし・string結果を必須にする。metadataの`context`、名称数253、source SHA-256、元順序の名称SHA-256を型・値・field集合とも厳密照合する。整数predicateも両probeでInteger-onlyを検査し、共通signed-64 wire decoderを使う。

| 限定検証 | 結果・範囲 |
| --- | --- |
| 53 native契約 | 2件。文字範囲の端点、control例外、非文字の受理差 |
| 53 実PHP比較 | 3,171件一致。境界±近傍、全平面末尾、負数、MIN／MAX、有限random |
| 53 protocol拒否 | 非整数についてPHP／native計14 processで固有guardを確認 |
| 54 native契約 | 5件。名前・処理順・numeric/nonmatch・exact cap・長い先頭ゼロ・raw bytes・全253名称 |
| 54 実PHP比較 | 6,527件一致。全名称とmutant、数字境界、先頭ゼロ、全byte文脈、ampersand重複、有限random |
| 54 protocol拒否 | 型違い／非HTML contextについて計30 processで固有guardを確認 |
| Python | 新規3件のprofile／strict literal抽出／両生成物stale拒否を含む関連16件が通過 |
| その他 | focused Clippy、fmt、probe build、PHP lint、Python構文検証を通過 |

限定記録は`target/valid-unicode-focused-comparison.json`、`target/default-entities-focused-comparison.json`、`target/default-entities-global-observation.json`。source探索時のPHP有限観測やXML charset比較は別のscratchであり、正式等価性corpusへ追加計上しない。正式PHP全比較とcombined gateは冒頭のログで成功した。

## 未対応の文脈

XML名前helperは、5種類のXML名称を保持する一方、他の許可名をPHP coreの`html_entity_decode(..., ENT_HTML5)`へ渡す。その変換表と`default_charset`に依存し、実測でも`&nbsp;`はUTF-8では`C2 A0`、ISO-8859-1では`A0`となった。XML用変換表を外部から導入していないため、この文脈は対応しない。

`_wp_specialchars`にはquote設定、site charset、PHP `htmlspecialchars`等があり、`esc_html`にはさらにUTF-8検査とfilterがある。これらの移植や動的allowlist、scalar coercion／診断、PCRE resource failure、一般HTML sanitizer、安全なHTML／XML出力、公開内容の選別、製品charset選択は未完成である。既存のraw DB読み取りやpublic HTTP経路へこのnormalizerを自動適用しない。

## 数値参照decodeの追加候補（55、正式PHP比較済み）

固定 `kses.php:2376` の `wp_kses_decode_entities` と、2393／2407のcallbackは、十進参照を `chr` で変換した後に十六進参照を変換する。結果はUnicode codepointではなくraw1byteであり、0はNUL、128はbyte80、255はbyteFFになる。名前参照はdecodeしない。

候補API `wp_kses_decode_entities_byte_values` はBytes入力と、**両passで実際に成立する参照値が0..255の範囲**に限定する。十進変換が新しいhex参照を作る場合も検査する。たとえば `&#38;#x41;` と `&#38;&#35;&#120;&#52;&#49;&#59;` はAとなる一方、`&#38;#65;` は `&#65;`、`&#x26;#x41;` は `&#x41;` に止まる。各passを再帰実行しない。成立した参照が範囲外なら `UnsupportedEntityValue` とし、semicolon欠落・途中の不正byteでregexが成立しない巨大数字は元のbytesとして保持する。

PHP8.5.12の未制限source観測では256以上の `chr` にE_DEPRECATED8192、巨大な数字にはTypeErrorとなる場合があった。この診断・modulo・float変換は対応範囲へ入れず、元の失敗観測をtargetへ残している。nativeの範囲制約をPHP自身の入力仕様とは扱わない。

全入力1MiBのhost上限を最初に確認し、十進passを検証・計測して中間Vecを確保する。中間長は入力以下だが、最終出力capの確認前に確保するため、そのcap・共有pool・RSSの計上外である。中間bytes上のhex passを全検証・計測し、最後に最終出力capを確認して出力を確保する。最終的に1byteへ縮む参照を圧縮前の長さだけで拒否しない。InputLimitとOutputLimitはhost制約で、製品設定やPHPの資源制約を決定しない。

PHP oracleは同じ順序の2regexと実source callbackによるdomain preflight後に、比較対象として実 `wp_kses_decode_entities` を別途呼び出す。PCREエラーなし・string結果・Bytes型・固定profileを検査する。一般HTML sanitizer、escaping、charset選択、UTF-8保証、公開HTTP／raw DBへの自動適用は追加しない。新6件のRust契約、型／範囲外／入力capの固有拒否36 process、profileの欠落・型混同・順序を拒否するPython新1件が限定検証で成功。7,646ケースの正式PHP比較と、workspace／nativeに分けた全gateは冒頭のログで成功した。
