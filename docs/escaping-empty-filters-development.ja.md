# UTF-8・empty-filter esc_html/esc_attr の開発記録

正式全gateは`target/empty-filter-escaping-and-memory-diagnostic-workspace-check.log`で成功した。63 API／436,656比較、esc_htmlとesc_attrは各1,938、hooks572／dispatcher405。workspace419 Rust（通常ignore1）・Python155、HTTP・Clippy・Linux cross-check・license208・DB current hashを通過し、独立レビューもblocking指摘なし。全native current hashも別途確認し、DB live／独立experiment再実行無しで直前29件・6 fixture各39項目を維持する。補助比較532やunsupported28、scopeの実行回数を正式case数へ加算しない。このgate完了後、ユーザー指示により停止した。

2026-10-27。固定WordPress 7.1.3 `wp-includes/formatting.php:4751/4776` の `esc_html` / `esc_attr` を、Bytes、UTF-8の固定site profile、no-mbstring validation、空のfilter registryに限定した返値kernelである。公開APIは `esc_html_utf8_empty_filters_bytes(input, max_output_bytes)` / `esc_attr_utf8_empty_filters_bytes(...)`。新規WordPress関数2件としてregistryは63件となる。両kernelは純粋なbytes返値だけをモデル化し、site設定・Dispatcher・任意callbacks・filter bookkeeping・公開HTTP/DB経路へ接続しない。

実ソースは全体へ `wp_check_invalid_utf8(text, false)` を適用し、`_wp_specialchars(safe_text, ENT_QUOTES)`、最後に `apply_filters` を呼ぶ。実hook名は `esc_html` と **`attribute_escape`** であり、後者は `esc_attr` ではない。空入力、または不正UTF-8から空になった値でもfilterを呼ぶ。空registryならbytesは変更されないが、PHPの呼出回数は増える。nativeはこのside effectを再現したとは主張しない。

全体UTF-8検査が61番helperのspecial-character guardより先行する。例えばraw `FF` だけの入力は、helper単体なら保持されるが、この2 APIでは空になる。実装は既存strip=false UTF-8 kernelと等価なvalidity predicateで判定し、有効なら元bytesを借用、不正なら空sliceを借用して61番helperのQuotes/falseへ渡す。余分なUTF-8 checked Vecは作らない。NUL、controls、Unicode noncharactersはUTF-8として有効なら保持し、HTML401による特殊文字変換だけを行う。XML validityや文脈を問わない出力安全性は保証しない。

固定HTML253 normalizer、HTML401の252名保持（`apos` 除外）、数値参照の正規化、元ampersandの非再帰処理は61番helperを再利用する。既存参照をdouble encodeせず、raw両引用符は変換する。`&apos;` は `&amp;apos;`、`&#00039;` は `&#039;`、`&#38;copy;` は `&#038;copy;` となる。元inputを変更しない。

callerのcapは最終出力だけに適用する。不正入力からの空結果はcap0でも成功する。長いleading-zero参照の圧縮前サイズへcapを適用しない。61番helperのnormalizer中間Vecはfallible allocationだがunpooledで、最終capより前に確保される。後段は共通count/emit traversal、checked長、exact reservationで完全結果または `SpecialCharsError::OutputLimit` を返す。cap、overflow、中間/出力allocation failureを同じerrorで表す。Vec容量、allocator、intermediates、peak/RSS、shared poolは計上しない。正式fixtureの65536-byte allowanceは開発host policyであり、PHP/製品の設定値ではない。

oracleは実 `esc_html` / `esc_attr` 関数を呼び、既存の `plugin.php` / `WP_Hook` / sentinelを利用する。型とliteral `filter_profile='empty'` を設定変更より先に検査し、`wp_filter/wp_filters/wp_current_filter` を全て空にする。target hookだけを消す方法では `all` callbacksが残るため、このprofileには不十分である。2つのentity globalsも固定defaultへ設定し、元INI `default_charset` を退避してUTF-8にする。3hook globals・2entity globalsのabsence/NULL/値と元INIを成功・例外の双方でfinally復元する。空の元charsetも対象とし、`ini_set(...) === false` と再読で検査する。

実呼出後、`wp_filter==[]`、`wp_current_filter==[]`、`wp_filters=={expected_hook:1}` を要求し、`wp_actions` のpresence/value不変も補助確認する。独立した観測counterへ各hookの回数を加算し、batchと正式evidenceの双方でAPI別case数に一致させる。`php_empty_filter_escaping_profile` は正確なhook名、UTF-8、no-mbstring、Quotes/false、全体UTF-8処理順、source/名前順序hash、native bookkeeping非対応を厳密型で束縛する。`empty_filter_escaping_hook_counts` は `esc_html/attribute_escape` の2keyだけを受理し、boolやfloat、hook別名、欠落や数の不一致を拒否する。

既存oracleの `get_option('blog_charset')` はprocess全体で常にUTF-8を返し、実site設定を変更しない。`wp_check_invalid_utf8` は最初の非空呼出でUTF-8判定をstaticへ保持するため、この不変profileが必須である。INIを復元するだけでsite charsetやstatic cacheを動的に切替・復元できるとは主張しない。固定ソースのfunctions/compat/UTF-8 helpers/KSES/plugin/両hook classesをsource dependenciesへ記録する。

Dispatcher統合は別候補として保留する。既存 `Dispatcher::apply_filters(name, safe_bytes, [original_bytes])` は呼出の引数順には適合するが、実WPのfilter結果はBytes以外にもなり得る。未来のfiltered APIは返値型、callback failure、bookkeeping、`all`、最後のcap評価順を別途定義する必要がある。callbackが縮める前のsafe bytesへfinal-only capを適用することも、現在の純粋profileから自動的に一般化できない。

限定検証はRust新4契約＋61番helper6契約、WP all-target Clippy/fmt、Python evidence20契約と新scope2契約を通過した。新corpusは各API1938件、計3876件でnativeが全受理し、先行する実PHP3876件との差分0。現oracleの532件も差分0で、実hook countsは `esc_html=266/attribute_escape=266`。対象外型・profileの28入力はPHP/native双方が拒否した。scope契約はUTF-8/ISO-8859-1/空INI × globals3状態 × 2 APIの18呼出を、success/inner exceptionそれぞれで確認し、actionsもabsent/NULL/valueの状態を保つ。

既存HTML6527件、XML7544件、61番helper23256件を維持し、全corpus候補は63API436656件となる。限定結果は `target/empty-filter-escaping-implementation-check.json`、先行実関数のglobal復元36観測を含む記録は `target/esc-empty-filters-observations.json`。scratchは正式件数へ加算しない。この時点では正式PHP3比較・全gateは親タスクへ引き継ぎ、成功を先取りしない。reference loader、既存data generators、DB/server、正式proof、全体docsは変更していない。
