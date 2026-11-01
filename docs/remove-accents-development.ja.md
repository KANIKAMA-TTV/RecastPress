# remove_accentsの明示UTF-8 / locale候補

後続の正式実測：`target/remove-accents-and-credential-pipeline-full-check.log`でAPI66・443,078ケース（新3,698）／hooks572／dispatcher405、Rust477＋独立29、Python207、8 DB fixture各55、Clippy・server/WP Linux check・license215・生成／legacy／HTTPが成功した。最終10checker全exit0とexporter--checkも確認し全fixtureを回収した。NoIntl／明示locale／validUTF8というdomainやLatin1／一般HTML処理の未対応を広げた成功ではない。

2026-10-29。固定WP 7.1.3 `wp-includes/formatting.php:1611` の `remove_accents` から、valid UTF-8のBytes、明示locale Bytes、reference processに `normalizer_is_normalized` と `normalizer_normalize` が両方存在しないprofileを対象とする。新APIは `remove_accents_utf8_explicit_locale_bytes(value, locale, max_output_bytes)`。site locale、get_locale、Intl導入、Unicode一般transliterationやNFKDによる辞書の置換は行わない。

## literalとlocale

nativeは固定PHP sourceからdata-only exporterが抽出したbase311 pairsとlocale overridesを使う。`src/remove_accents_table.rs` のbytesはsource-derived GPL-2.0-or-later dataであり、PHPコードのevalや全WP bootstrapは行わない。keyの重複、UTF-8、長さ、literal grammar、source hashとordered pairs hashをgeneratorが検査する。native側もlexicographic unique/nonempty keys、最大4 bytesと表件数をprivate契約で固定し、binary lookupの前提を拘束する。

| locale Bytes | 追加・上書き規則 |
| --- | --- |
| lowercase `de` で始まる全値 | German8。Ä/ä/Ö/ö/Ü/ü→Ae/ae/Oe/oe/Ue/ue、ẞ/ß→SS/ss |
| exact `da_DK` | Danish6。Æ/æ/Ø/ø/Å/å→Ae/ae/Oe/oe/Aa/aa |
| exact `ca` | multiscalar `l·l`→`ll` |
| exact `sr_RS` / `bs_BA` | Đ/đ→DJ/dj |
| その他 | base311だけ |

`de_DE`だけの列挙ではなく `de` prefixなので、`de`、`default`、`de`で始まる非UTF-8/NUL混在localeにもGerman規則を適用する。`DE`、先頭空白、`ca\0`、`da_DK\0`などを勝手にtrim/case foldしない。locale自体のUTF-8は要求しない。

empty localeと単一byteの`0`はUnsupportedValueに狭める。元PHPのempty判定はsite localeへfallbackするため、その暗黙値をnativeが選ばないためである。この明示profileのdomain guardはASCII/empty入力にも適用する。元PHPはASCIIを早期returnできるが、それを利用してsubset外localeを正式入力へ混ぜない。invalid UTF-8入力もUnsupportedValueとし、元sourceのISO-8859-1 fallbackを再現したとは主張しない。

source formatting.php SHA-256は `350f6b32303915df970bb0e3ea360fcd54bb0b8a7ecc3d504c3d860ab9468d32`。base ordered hex pairsは `69b78d9596b1e927213e72b0698a40a85daa76e8846a4d86d7a13c78d4594477`、locale override ordered pairsは `07c40351a07697a814d6a97ec45c2549b7f08071f3ff3cc33768fa0285e6932c`。将来のsource/table変更はdata-only再抽出と実PHP再比較を要し、Unicodeの新しい標準表で黙って置き換えない。

## 最長一致と資源境界

元のarray形式 `strtr` と同じく、元inputの各位置で最長keyを先に探す。最大4 bytesから1 byteへ探索し、同じkeyではlocale overrideをbaseより先に選ぶ。生成されたreplacementを再びinputへ戻さず、一度だけ置換する。Catalan `l·l·l` は `ll·l` となり、重なった後半を再帰処理しない。NUL、未対応scalar、BOM、noncharacter、emoji、日本語を含む未対応bytesはそのまま保つ。

Intl NFCは実行しない。precomposed éはeになるが、NFDの `e`＋combining acuteはそのまま残る。あらゆるaccent除去、Unicode normalization、case folding、locale一般化、HTML/URL escapingや出力文脈の安全性を保証しない。

同じwalkerをcountとemitで2回使い、checked byte加算で最終出力長を先に測る。適合後だけ `try_reserve_exact` で出力を確保し、部分結果を返さない。`RemoveAccentsError` はUnsupportedValue / OutputLimit / AllocationFailedの固定分類で、入力・locale本文を診断に保持しない。GBPの£は削除されるので、連続£だけなら入力が長くてもcap0で空Vecを返せる。入力長を出力capの早期拒否条件へ転用しない。

このcapはhost内部の最終output bytesだけであり、入力、Vec実capacity、allocator、peak/RSS、concurrency、shared budgetや製品manual設定の制限ではない。呼出元inputは借用するが、その確保やPHP oracleのinput/driver overheadは計上しない。DB readerがraw値を自動変換したり、公開HTTPへ接続したりする機能も追加しない。

## 契約と検証段階

public4契約は全locale枝とraw prefix、Catalan最長非再帰、NFD保持と未対応Unicode/NUL、GBP連続cap0、exact/不足/empty出力、invalid UTF-8/empty locale拒否、input不変を確認する。private2契約はtable件数・辞書前提と、checked MAX overflow・fallible capacity overflow分類を確認する。担当の6件とWP全target Clippyは通過した。正式PHP corpus/profile/registryのAPI66とreference比較は別担当・親のgateで行い、この記録では後続実測成功を先取りしない。

現在DBのdev-only rehash pipelineがWP sourceを実呼出しするため、WP Cargoと全src/libもDB evidenceのtracked sourceに含まれる。今回の新module/lib変更はDB・alpha・binlog・bootstrap・rotation・restart証跡もstaleにする。normal runtimeのDB/serverへWP依存を追加したという意味ではない。source freeze後の親gateで必要なfresh fixtureを実再測定し、hashだけの手動restampで済ませない。本担当はDB起動・正式proof生成・Cargo変更を行わない。
## API66のdata/exporterと限定比較

`scripts/export_remove_accents.py` は固定sourceだけを読み、PHPを実行せずbase literal311組とGerman8・Danish6・Catalan1・Serbian/Bosnian2組を抽出する。単純な単一引用string pair以外、重複、式、異なるlocale guard/分岐順、UTF-8でないkey、4bytesを超えるkey、ASCII0..2bytes以外のvalueを拒否する。全source SHAとsource順のhex-pair SHAを固定し、生成JSON artifactとRust tableの両方をbyte単位で `--check` する。小さいtableを含めRustfmt後も生成bytesが変わらない形式とした。

base ordered pairsのSHA-256は `69b78d9596b1e927213e72b0698a40a85daa76e8846a4d86d7a13c78d4594477`、locale override群のordered SHA-256は `07c40351a07697a814d6a97ec45c2549b7f08071f3ff3cc33768fa0285e6932c`。生成物は `compatibility/wordpress-remove-accents.json` と `crates/recastpress-wp/src/remove_accents_table.rs`。WordPress由来dataとしてsource/version/行・生成器を示し、新しい辞書方針やUnicode tableを外部から導入しない。

PHP oracleはselected loaderで固定 `remove_accents` を抽出し、Bytes textとPHP-truthyなBytes localeを明示2引数として渡す。valid UTF-8を実WP helperで確認し、noMbstring、Intl extension不在、両normalizer関数不存在をすべて要求する。localeはUTF-8検査・casefold・trimを行わない。空とASCII `0` だけを除外し、NUL・不正UTF-8・`00`・`0`+NUL等も真偽値条件を満たせばbyte比較する。初版の `get_locale` はthrowするsentinelだった。後続API67/68導入では固定l10n.phpの実getterへ置換し、API66のlocale hook差分0とverified fixtureのDB branch trapでambient locale非使用を確認する方式へ移した。旧3,698入力の順序付きSHAを固定し、実getter導入後も全件を再比較して差分0だった。実関数のPCRE errorや非string結果は成功に混ぜず、返値は共通byte codecへ包む。

API66 corpusは3698件。全base keysを6つの代表localeで比較し、override keys、Catalanの重なり、lowercase `de` 任意prefix、大文字・末尾NUL等でexact localeから外れる境界、全ASCII bytes、NFD/NFC、GBP削除、noncharacter/BOM/不正UTF-8 locale、固定seedのvalid UTF-8混在列を含む。旧65 API・439,380件のID/入力SHAは維持し、末尾にだけ追加した。限定実PHP比較は差分0件、実関数呼出3698回、`get_locale` 0回。正式3proofは未更新で、66 API・443,078件の正式全比較はroot再実測待ちである。

profile validatorはsource/table identityとnoIntl両関数不在等を型・値まで厳密に照合する。証跡のrequested locale分類6種すべての正件数・総数一致、actual reference calls総数、ambient locale callsの厳密integer0を必須にする。requested分類はcorpus入力から導くもので、ASCII fastpathの内部branchをtraceしたという意味ではない。exporter/synthetic oracleのPython5契約、追加evidence2契約、対象Clippy/fmt・既存comparison generator試験を通過し、型/domain/profile拒否46 processと診断引数のsecret marker非漏洩も確認した。最初の限定比較で共通codecの包み忘れを検出し、修正後に3698件を全再比較した。

この変更はWP source/lib/generated tableを更新するため、DBのdev-only owned credential pipelineを含む現source collector下ではDB証跡もstaleになる。手動hash更新はせず、rootが正式PHP・generation/legacy・全owned DB fixtureを実測再生成する。製品のstandard login、`sanitize_user`、HTML tag処理、credential lookup/認証・保存policyへは接続していない。
