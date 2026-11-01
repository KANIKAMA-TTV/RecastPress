# 固定XML・UTF-8 entity normalizationの開発記録

正式比較・全workspace gateは`target/xml-utf8-and-comment-feeds-workspace-check.log`で成功した。60 API／409,524比較、normalizer HTML6,527＋XML7,544＝14,071、hooks572／dispatcher405を実PHPで再生成した。workspace402 Rust（通常ignore1）・Python141、Clippy・Linux cross-check・license208観測・実HTTP・DB current hashも通過し、XMLとコメントserializerの独立レビューでblocking指摘なし。全nativeのcurrent hashは別途照合し、DB live／独立experimentは再実行せず直前29件・6 fixture各39項目を維持する。補助観測や復元試験の呼出回数を正式corpus件数へ加算しない。

2026-10-27。`wp_kses_normalize_entities_default_xml_utf8_bytes(input, max_output_bytes)` は、固定WordPress 7.1.3 `wp-includes/kses.php:2169` の literal `xml` context、同ソースのXML5名とHTML253名、PHP 8.5.12 `ENT_HTML5=48`、`default_charset=UTF-8` の明示profileである。既存のdefault HTML kernelを維持し、同じWordPress関数の別profileとして登録するため、WordPress関数件数は60のままである。サイトcharsetや製品の既定policyを選択しない。

XML5 `amp/lt/gt/apos/quot` は参照文字列のまま保持する。残り248名だけを固定PHPから観測したUTF-8 bytesへ変換する。今回の248出力は全て単一Unicode scalarだったが、実装表は任意のbyte sliceを保持し、この観測を将来の一般法則にはしない。`quot/apos` が直接の `html_entity_decode(..., ENT_HTML5)` でも変換されないのはquote flagsを指定していないためで、欠落データではない。XML5判定が先行するため `amp/lt/gt` も文字にはしない。一般のHTML5名称、例えば `fjlig/NotEqualTilde` は許可表にないので変換しない。

数値処理・元ampersandの非再帰走査・最終出力の計測と生成はHTML kernelと共有する。decimal、hex、namedの順序を保ち、数値参照は文字へdecodeしない。`&copy;` は `©`、`&amp;copy;` はそのまま、`&#38;copy;` は `&#038;copy;` となる。生成したampersandを再走査して二重decodeしない。NULや不正UTF-8を含む他のbytesはそのまま通すため、結果のXML validityを保証しない。実WPのcontext比較はstrictであり、`true` はHTML分岐になる。開発wireは既存のliteral `html` または新literal `xml` だけを受理し、他型や `XML` を拒否する。

`scripts/wordpress_entity_names.py` は従来どおりdata literal抽出のみで、PHPを実行しない。新しい `scripts/wordpress_xml_entity_mapping.py` と専用 `.php` observerを分離した。生成時は固定PHP executable SHA-256、version、64bit、UTF-8、ENT_HTML5、KSES source SHA、厳密なXML5 literalとHTML253名順序を確認し、既存の選択関数loaderで実normalizerとhelpersだけをロードする。KSES全bootstrap、CUSTOM_TAGSやtag globalsの初期化は行わない。

観測JSON `compatibility/wordpress-xml-utf8-entities.json` と生成Rust `src/xml_entity_mapping.rs` は、253のordered `[name, output_hex]` pairsを記録する。そのcanonical SHA-256は `6810e6d66a468cf2a7b75a0969b932fd727553dc7141090bd837d6023331c25b`。JSONにはobserver/loader source hashも保持する。`--check` はPHP無しで固定identity・全pairs・JSON/Rust双方の生成bytesを照合する。`--check --php .reference/php-8.5.12/php.exe` は実関数で全253名を再観測し、固定hashと保存表を照合する。source/tool/mapping変更は自動追従せず拒否し、意図した再検証が必要となる。PHP実装や汎用変換tableのコード移植ではなく、この固定profileの出力観測データである。

oracleはBytesとliteral contextの検証を設定変更より先に行う。呼出時だけ `default_charset` をUTF-8にし、`ini_set(...) === false` と再読値で確認する。2つのKSES entity globalsは `array_key_exists` によるabsence、NULL、値を保存し、成功・例外のどちらでもfinallyでcharsetとともに完全復元する。空の元charsetも有効な復元対象である。HTML側wrapperやsite optionは変更しない。各batchのmetadataには実関数で再観測した253名のhash、source/名前順序/executable hash、UTF-8とPHP/flagの厳密型値を出し、validatorはHTML/XML両方の正整数case countも要求する。

最終bytesの長さを同一traversalでchecked計測し、caller cap検査、exact reservation、完全出力の生成を行う。cap、checked overflow、allocation failureは部分結果を返さないhost policyである。出力はunpooledで、Vec容量、allocator、driver、peak、RSS、shared budgetの計上は行わない。custom allowlists、他charset、非Bytes coercion、PHP diagnostics、PCRE resource failure、再帰decode、sanitization、escaping、出力安全性、public HTTP/DB接続は対象外である。

限定検証では新Rust4契約と既存HTML5契約、WP all-target Clippy、Python evidence16契約とscope復元2契約を通過した。native corpusはHTML6527件を維持しXML7544件を追加、全14071件が受理された。先行する実PHP観測2404件とnativeの差分0、現oracleの253名＋40境界の293件も差分0。Bytes/context不適合9入力は両processが拒否した。UTF-8/ISO-8859-1/空charset × globals absent/null/sentinelの9状態は実関数で復元を確認し、synthetic processはinner exception時のfinallyも検査した。詳細は `target/xml-utf8-profile-implementation-check.json`、`target/xml-utf8-current-wrapper-restore.json`。これらscratchを正式件数へ加算しない。正式PHP3比較と全gateは親タスクが別途実施するため、この時点で成功を先取りしない。
