# 明示UTF-8 `_wp_specialchars` の開発記録

正式比較は`target/specialchars-and-bundle-check-workspace-check.log`で成功した。61 API／432,780比較、うち新23,256＝6styles×両bool各1,938、hooks572／dispatcher405。workspace411 Rust（通常ignore1）・Python149、Clippy・Linux cross-check・license208・実HTTP・DB current hashを通過し、独立レビューもblocking指摘なし。全native current hashも別途通過し、DB live／独立experimentは再実行せず直前29件・6 fixture各39項目を維持する。補助観測16544／409やunsupported27、scope復元の回数を正式case数へ加算しない。

2026-10-27。固定WordPress 7.1.3 `wp-includes/formatting.php:945` の `_wp_specialchars` を、Bytes、明示UTF-8、6種のquote引数、明示bool `double_encode` に限定した独立kernelである。公開Rust APIは `wp_specialchars_utf8_bytes(input, SpecialCharsQuoteStyle, double_encode, max_output_bytes)`。registryでは新しいWordPress関数として61件目に数える。`esc_html` / `esc_attr` 本体・filter接続・サイトcharset・製品の既定policyは採用しない。

`SpecialCharsQuoteStyle` は `NoQuotes/Compat/Quotes/Single/Double/Xml1`。実WPへ渡す値は順に `0/2/3/'single'/'double'/16` である。literal `ENT_XML1=16` だけがWP内部で `ENT_QUOTES|ENT_XML1` に変わる。入力として複合値19、`ENT_SUBSTITUTE=8`、`ENT_IGNORE=4`、`ENT_HTML5=48` などを渡すとWPはHTMLのQuotesへ正規化するため、一般ENT bitmaskのAPIとして扱わない。wireも選択style名だけを受理し、raw数値・bool・未選択文字列やcoercionを拒否する。

元入力に `&<>"'` が無ければ、WPは早期returnする。不正UTF-8もそのまま返す。一方、この5byteのどれかが1つでも含まれ、不正UTF-8が存在すれば、変換結果は全体が空となる。NoQuotesやSingleでもguardの集合は変えない。既存の `wp_check_invalid_utf8` を無条件に先行適用すると異なる結果になる。UTF-8として有効なNUL/control/noncharacterはscrubしない。例えばraw U+FFFEと `&` を含む入力も、前者を保持し後者だけ変換する。XML validityや任意文脈での安全性を保証するものではない。

`double_encode=false` では、既存の固定HTML/XML normalizerを呼び、その完全な中間bytesを後段で処理する。HTMLは253名の正規化後にHTML401の252名を保持し、`apos` だけは `&amp;apos;` へ再escapeする。Xml1はXML5を参照のまま保持し、残り248名を固定PHP 8.5.12 / ENT_HTML5 / UTF-8観測表で変換する。後段の参照保持はXML5だけになる。中間全bytesを読む二段構成なので、mappingがASCII特殊文字を生成しないという現観測には依存しない。

数値参照は既存のdecimal/hex正規化後も文字にdecodeしない。`&#00039;` は `&#039;`、`&#X00027;` は `&#x27;`、`&#38;copy;` は `&#038;copy;` となる。生成ampersandを再帰decodeしない。SingleはPHPのno-quotes処理後にraw apostropheを `&#039;` にするback-compat処理であり、既存の数値参照を一度decodeしてから置換する処理ではない。この最終置換だけを同じ出力traversalに組み込んでいる。Xml1のraw apostropheは `&apos;` となる。`double_encode=true` はnormalizerを呼ばず、既存参照のampersandも変換する。

正規化中間Vecはfallible allocationで、最終capの前に作られるunpooled dataである。長いleading-zero参照は中間処理で縮むため、中間サイズや入力長へ最終capを適用しない。後段は同一escape traversalでchecked長を計測し、最終cap検査、exact reservation、完全な出力を生成する。`SpecialCharsError::OutputLimit` は最終上限、checked overflow、中間/出力allocation failureを含み、部分結果を返さない。容量・allocator・temporaries・peak・RSS・shared poolは計上しない。正式fixtureの最終allowanceは65536 bytesであり、PHPや製品の設定値ではない。

oracleはBytes、選択style名、exact boolを設定変更より先に検査し、実 `_wp_specialchars(input, selected_argument, 'UTF-8', bool)` を呼ぶ。`_canonical_charset` と依存predicateも固定ソースから選択ロードし、明示非空UTF-8により `get_option` へ進まない。XML内の `html_entity_decode` はINIを参照するため、`default_charset` も呼出時だけUTF-8にする。`ini_set(...) === false` と再読を確認し、2つのentity globalsのabsence/NULL/値と元INI値をfinallyで復元する。空の元charsetも復元対象である。既存loader、data generator、HTML/XML APIの契約は変更しない。

metadata `php_specialchars_utf8_profile` はUTF-8、PHP8.5.12、6引数の厳密型値、conditional UTF-8順序、formatting/KSESのsource hash、HTML名順序hash、XML観測mapping hashを束縛する。`specialchars_style_double_case_counts` は6×2の12cellを全て正整数で要求し、合計を当該API件数へ一致させる。source_dependenciesにはKSES、functions、compatを記録する。一般のPHP `htmlspecialchars` の全flags/charset、非Bytes cast、PCRE resource failure、custom allowlists、filter、サイト設定、HTML5一般decode、XML validity、public HTTP/DB接続は対象外である。[PHP公式の変換・無効encoding仕様](https://www.php.net/manual/en/function.htmlspecialchars.php)も参照した。

限定検証は新Rust6契約＋既存HTML5/XML4契約、WP all-target Clippy、source registry61件、Python evidence18契約と新oracle scope2契約を通過した。新corpus23256件は12cellそれぞれ1938件でnativeが全受理した。既存HTML6527/XML7544件は維持し、全corpus候補は61API432780件になる。先行する実PHP selected-style観測16544件とnativeの差分0、現oracleの409件も差分0。対象外の型・raw flagsなど27入力はPHP/native両processで拒否した。scope契約は3charset × 3global状態 × 12組の108呼出をsuccess/inner exception双方で確認する。

限定記録は `target/specialchars-implementation-check.json`、先行調査は `target/wp-specialchars-observations.json` / `wp-specialchars-supplemental.json` / `wp-specialchars-feasibility-summary.json`。先行PHP観測は未選択styleも含め17024件で診断0、例外0、PCRE error0だった。scratchは正式件数へ加算しない。この記録時点では正式PHP3比較・全gateは親タスクへ引き継ぎ、成功を先取りしない。
