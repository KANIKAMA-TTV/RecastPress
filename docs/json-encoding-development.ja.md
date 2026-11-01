# WordPress JSON encodingの対応範囲

2026-10-27時点。最新JSON gateはtarget/json-utf8-fallback-profile-check.logで成功し、WP45 API・310,363ケース（JSON23,365件／9flags全512組合せ）、workspace Rust256件・Python88件を確認した。今回はDB live／独立experimentを再実行せず、直前全gateの独立19件・3 fixture各39項目を保持する。固定WordPress 7.1.3のfunctions.php:4443にあるwp_json_encodeから、最初のjson_encodeが成功する入力の部分集合と、下記の明示profileだけに限ったfallbackを移植する。元ソースのidentityはcompatibility/registry.json、正式な比較結果はcompatibility/evidence/primitives.jsonで追跡する。WP全体のbootstrapやJSON endpoint、HTML／scriptへの安全な埋込みを提供する機能ではない。

## Native APIと値

```rust
wp_json_encode(value: &PhpValue, max_output_bytes: usize)
    -> Result<Vec<u8>, JsonEncodeError>
wp_json_encode_with_flags(value: &PhpValue, flags: JsonFlags, max_output_bytes: usize)
    -> Result<Vec<u8>, JsonEncodeError>
wp_json_encode_utf8_no_mbstring(value: &PhpValue, flags: JsonFlags, max_output_bytes: usize)
    -> Result<Vec<u8>, JsonEncodeError>
```

既存のwp_json_encodeはflags=0 wrapperとして保持する。JsonFlagsはprivate u32を持ち、Defaultは0、from_bitsで対応bitだけを受理し、bitsで値を取得する。unknown bitはUnsupportedFlags。対応する値はNull／Bool／signed i64／valid UTF8 bytes／ordered PhpArrayで、keyも整数またはvalid UTF8 bytesに限る。Floatは有限値も含めて未対応であり、numeric stringを数値へ変換しない。

既存PhpArrayのnumeric key正規化と挿入順を使う。integer keyが順に0..n-1の場合だけJSON arrayにし、空配列も[]となる。穴・逆順・string keyがある場合は順序を保ったJSON objectにする。objectのinteger keyは十進文字列にする。FORCE_OBJECTを指定するとlistとemptyも含む全array containerをobjectへ変換し、nestedにも適用する。

## Selected flags

| PHP flag／JsonFlags定数 | bit | 動作 |
| --- | --- | --- |
| JSON_HEX_TAG／HEX_TAG | 1 | <／>をPHP固有の大文字C／Eで`\u003C`／`\u003E`にする |
| JSON_HEX_AMP／HEX_AMP | 2 | &を`\u0026`にする |
| JSON_HEX_APOS／HEX_APOS | 4 | apostropheを`\u0027`にする |
| JSON_HEX_QUOT／HEX_QUOT | 8 | quoteを`\u0022`にする |
| JSON_FORCE_OBJECT／FORCE_OBJECT | 16 | 全array containerをordered objectにする |
| JSON_UNESCAPED_SLASHES／UNESCAPED_SLASHES | 64 | slashをrawにする |
| JSON_PRETTY_PRINT／PRETTY_PRINT | 128 | nonempty containerへ4space indent・LFとcolon後spaceを付ける。emptyはcompactのまま |
| JSON_UNESCAPED_UNICODE／UNESCAPED_UNICODE | 256 | 非ASCIIをUTF8で保持する。ただしLINE未指定のU2028／2029はescapeする |
| JSON_UNESCAPED_LINE_TERMINATORS／UNESCAPED_LINE_TERMINATORS | 2048 | UNICODEと併用した場合だけU2028／2029をrawにする |

現在のallowed maskは2527、9bitの組合せは512通り。初回の8flags／mask2399／256組合せもその部分集合として保持する。LINE単独ではU2028／2029のescapeは変わらず、LF／CR等のcontrolをrawにする指定でもない。同じstring処理をobject keyとvalueに適用する。HEX以外の非ASCII escapeはlowercase hexとUTF16 surrogate pairを用い、UNICODE指定時でもHEXの指定は維持する。

flags=0ではslashは`\/`、quote／backslashとcontrolは既定のJSON escape、非ASCIIは`\u`、U2028／2029もescapeとなる。backspace／TAB／LF／form feed／CRは短縮形、VTは`\u000b`、DELはraw byteを保持する。<>&'はHEXを選択しなければraw。これらのflagsを実装したことはHTML／script contextでのescapingやsecurity保証を意味しない。

PRETTY_PRINTではnonempty list／objectの各entry前にLFと4space単位のindentを置き、closing delimiter前も親の深さへ戻す。objectはcolon後に1spaceを置く。empty []／{}とscalarは改行を足さず、root出力末尾にもLFは追加しない。FORCE_OBJECT、key順序、HEX／Unicode／LINE／slashの指定は同時に維持する。default wrapperのcompactな出力は変わらない。

## Host制限と失敗

PHP側の引数は既定depth=512の成功経路に限定する。一方、nativeのhost制限はarray container64段で、rootとemptyも1段として数え、scalar葉は追加消費しない。この64はPHPの512と同じ制約ではない。対応domain／深度を検証しながらchecked出力長を計測する最初のpassを終えてから、escapingとindent／LF／colon後spaceを含む完全出力byte capを照合し、reserve後の2pass目でencodeする。深さとindent幅の演算もcheckedで、入力byte長をcapとして使わず、入力は変更しない。

UnsupportedValue／UnsupportedFlags／RecursionLimit／OutputLimitを返し、部分JSONは返さない。OutputLimitはhostのbyte cap・算術overflow・出力reservation失敗を含み、PHPのfalse sentinelやPARTIAL_OUTPUT指定を再現するエラーではない。出力capはRSS／peak allocation／shared poolの予算や自動容量を保証しない。

既存2 APIではinvalid UTF8とWPのfallback helpersを未対応のままとする。Float、object／resource／参照、非default PHP depth、その他flags、Wasmも未対応。numeric check、partial output、throw/error behavior等の未選択flagを適用しない。

## 明示UTF8・no-mbstring profile

新しいwp_json_encode_utf8_no_mbstringだけが、blog_charsetがUTF-8でmb_convert_encoding関数が存在しない固定profileを扱う。charsetを自動推定せず、mbstringや非UTF8の設定へ一般化しない。WordPressではconverterとwp_check_invalid_utf8が初回の関数存在／charset判定をstaticへ保持するため、実行中の設定変更も対象外。最初のjson_encodeが成功するvalid UTF8入力は既存APIと同じ経路で出力し、変換しない。

invalid UTF8がある場合は、元graph全域でFloat／Unsupportedとhost64段を先に拒否してから、_wp_json_sanity_check :4475／_wp_json_convert_string :4537、formatting.phpのwp_check_invalid_utf8、utf8 helpersに沿って全string key／valueを補修する。既存wp_scrub_utf8_bytesのmaximal ill-formed subpartごとのU+FFFD置換を使い、valid bytesは保持する。再構成したPhpArrayはnumeric keyの正規化と挿入順を維持し、補修後のkeyが衝突すれば後の値で上書きして最初のkey位置を残す。たとえばFF／middle／FEのkey列はreplacement／middleへ縮み、replacementの値はFE側になる。JSON_INVALID_UTF8_SUBSTITUTEのように同じJSON keyを複数出す処理とは異なる。

衝突で消える元の値にもhost制限を適用する。WP自体が補修後の衝突によってNaNや深い配列を消して成功しうる場合も、このnative subsetでは元graphを拒否する。拒否後に部分出力は返さず、元入力は変更しない。補修graph／key／bytesのcloneは出力長測定より前に確保され、出力capはこの追加メモリ・peak／RSSを制限しない。shared poolへの接続や低メモリ保証ではない。

比較oracleは固定WPの実helpersを抽出し、get_optionはblog_charsetにUTF-8を返す限定stubだけを使う。各batchでcharset文字列、mb_convert_encoding不存在、実is_utf8_charset=trueの型と値を照合する。元profileと明示profileの件数を別に記録し、明示profileでも9flags全512組合せを要求する。補修byte境界、衝突、literal replacement key、nested、pretty、valid fast path、transport24段の6,292追加ケースを用意した。target/json-utf8-fallback-profile-check.logで全追加ケースを照合し、既存17,073と合わせJSON23,365／全45 API310,363ケース、Rust256件・Python88件、HTTP・Clippy・Linux cross-check・license208観測と既存DB証跡を確認した。別途binlog／overlapのcurrent hash照合も成功。DB live／独立experimentを今回再実行しておらず、前回の独立19件・3 fixture各39項目を維持する。

## 検証記録と境界

default導入時の正式比較5,303件はtarget/json-encoding-and-license-check.logで成功した。PHP値transportの入れ子は24段まで、nativeの64段境界はRust契約で確認する。別のdefault探索76件はtarget/json-encode-observation.php／.jsonlにあり、transport越しの64段比較や正式corpus件数へ加算しない。

初回8flagsの探索はtarget/json-selected-flags-observation.php／.jsonの1,792件と、target/json-selected-flags-boundaries.php／.jsonの175件。これはbit値・escaping相互作用・nested／depth等を調べるPHP scratch観測で、正式なRust／PHP等価性corpusへ加算しない。実装は既存5件＋新規6件のRust契約11件とClippyを通過し、全256組合せ・key／value・force empty／nested・cap境界・unknown bit・対応domain／深度を検査した。

独立レビューでは新flags対象7,656ケース、全256組合せのPHP比較が0 mismatchだった。続くtarget/json-selected-flags-full-check.logでworkspace Rust230件・Python79件、正式比較45 API・299,957ケース（JSON 12,959件）、hooks572／dispatcher405、HTTP・Clippy・Linux向けcross-check、license観測と既存DB証跡のcurrent hash確認が成功した。PHP定数8種の厳密な型・値と、全256組合せの正整数件数・JSON総件数との一致を証跡検査で要求する。DB live／独立experimentは今回再実行していない。前回の停止はユーザーの再開指示で解除されている。

PRETTY_PRINTのscratchはtarget/json-pretty-observation.php／.jsonとjson-pretty-differential-observation.jsonにあり、全512組合せ×5sampleの2,560件で固定PHP8.5.12とdraft nativeのbytesを照合した。独立レビューの4,641件も別観測として扱い、いずれも正式corpusへ加算しない。既存Rust契約11件に4件を追加した計15件で、empty／scalar／nonempty／nested／force／key、全flags併用、escaping＋indent後のexact capと1byte不足、native64段と未対応domainを検査する。PHP値transportは24段までという境界を変えず、64段のnative契約をtransport比較と混同しない。

正式corpusはPRETTY対象4,114件追加でJSON 17,073件、WP全45 API・304,071ケース、9定数・全512組合せのmetadataを検査した。target/shared-buffer-and-pretty-json-full-check.logでworkspace Rust251件＋独立experiment19件・Python86件、hooks572／dispatcher405、3所有DB fixture各39項目、HTTP・Clippy・Linux向けcross-check、license観測208件・current provenance照合が成功し、fixtureを回収した。native Linux実行・HTML／script安全性・RSS／自動容量・製品CDCの完成を示すものではない。

独立レビューでは正式corpusとは別の狙い56ケースでbytes一致、未知／null／bool／整数profileの8拒否を確認した。target/json-utf8-profile-independent-review.{py,json}は有限の追加観測で、正式6,292へ加算しない。設定metadataの型、profile別件数、fallback専用512組合せ、元graph検査と既存APIの拒否範囲を読み直し、blockingな問題は見つからなかった。

## finite f64の未採用調査

固定PHP8.5.12 -n・serialize_precision=-1、flags=0／JSON_PRESERVE_ZERO_FRACTION=1024とRust1.96.0のstd LowerExp／Debugをtarget scratchで比較した。JSONのfloat出力はserialize_precisionに依存するため、任意設定へ一般化しない。[公式PHP仕様](https://www.php.net/manual/en/function.json-encode.php)。102,941 finite bitpatternsでは、指数体裁だけの正規化に56差分があった。代表bits `3e60000000000000`（2^-25）はRust最短表記が`2.9802322387695313e-8`、PHPが`2.9802322387695312e-8`となる。

独立したchecked u128のexact decimal midpoint判定で奇数末尾を偶数隣へ補正すると元56差分は消えたが、±2^-24（`3e70000000000000`／`be70000000000000`）に2反例を作った。正の例は補正候補`5.960464477539062e-8`に対しPHPは`5.960464477539063e-8`。power-of-two境界で偶数候補が元bitsへroundtripしないため、midpoint一致だけでは不十分だった。補正候補をparseして元bitsと一致する場合だけ採用するgate付きでは102,941件と狙い近傍401件がともに差分0だった。

source・bit列・両出力・反例はtarget/json-f64-observation.{rs,php,py,json}、json-f64-u128-midpoint-observation.json、json-f64-u128-roundtrip-gated-observation.json、json-f64-midpoint-neighbors.{py,json}に保存した。これは固定PHP設定・Rust版と有限集合に依存する挙動観測で、全finite f64の正確性やPHP内部方式を証明しない。追加gateを含む方式の十分性は未確定のため本体へ採用せず、FloatUnsupportedを維持する。PHP実装のコピー／移植、新依存・unsafeは追加していない。正式45 API・304,071ケースへこれらscratch件数を加算しない。
