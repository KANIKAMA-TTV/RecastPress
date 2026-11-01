# 英語・HTMLなしの wptexturize 候補

固定 WordPress 7.1.3 `wp-includes/formatting.php:37` の `wptexturize` と、同 `320` の prime 処理を、英語の固定置換・HTMLなしの入力へ限定して移す pure helper。新 9,183 ケースの対象比較に続き、`target/texturize-full-check.log` が exit 0 で完了した。正式 74 API／495,336 件、全 8 fresh DB 構成各 62 契約、native／pure profile と HTTP 等が成功した。最終既存 11 checker も全 exit 0 を確認した。一般のテーマ本文描画の完成を主張しない。

## Native API と入力 domain

```rust
TexturizeLimits::new(max_input_bytes, max_work_bytes, max_output_bytes)
wptexturize_default_english_no_html_bytes(raw, limits)
    -> Result<Vec<u8>, TexturizeError>
```

3 上限は caller が明示し、既定値はない。入力は全体が valid UTF-8 の Bytes で、literal `<` を含まない。NUL、改行、quote、Unicode、`&lt;` 等の文字はこの条件を満たせば入力になり得る。HTML tag／comment、invalid UTF-8、PHPの任意 scalar coercion はこの profile の外に置く。

英語 `_x` の source literal、標準 cockney 置換、run true、空の関連 filter と `all`、空の shortcode registry、未設定の `$wp_cockneyreplace` を前提とする。PHP 比較は毎回 `reset=true`。gettext／locale 動的解決、独自 cockney、登録 shortcode、filter callback による停止や置換を実装したものではない。shortcode 風の文字があっても、登録された shortcode を実行しない。

規則の順序は source に合わせ、静的な ellipsis／quote／商標／cockney の置換、apostrophe／quote と prime の曖昧性解決、dash、数字間の times、entity らしい形以外の ampersand 置換へ進む。`xn--`、省略年、decimal、句読点、最終 LF、ASCII whitespace／UTF-8 NBSP／`&nbsp;` などは source の規則ごとに判定する。一般の Unicode typography や自然言語の引用意味を推測しない。

返却値には source と同じ numeric entity 風 bytes が含まれる。これは HTML escaping、sanitization、private data 除去、公開承認ではなく、任意入力を属性や HTML へ安全に埋め込む契約でもない。記事編集・DB・公開 route や Wasm SDK へ自動接続しない。

## 3 上限

入力 byte 上限を最初に確認し、次に whole UTF-8／NoLiteral `<` domain を検査する。work 上限は初期コピー、各置換 pass と prime helper 内を含む作業 buffer の論理長へ適用し、最終 output 上限は変換後だけを検査する。空文字・`0`・空白等の source 短絡も、native の初期 work コピーと上限の対象である。

中間 marker や entity 展開で、一時 work が最終出力より長くなる場合がある。最終 output cap を途中の pass へ早期適用しない。逐次処理なので work 拒否が後続の変換や final output 拒否より先に起こり得る。全 pass の累積確保量・Vec capacity・allocator overhead・RSS／peak memory・CPU時間・共有 memory pool の上限ではない。

拒否は `UnsupportedValue`、`InputLimit`、`WorkLimit`、`OutputLimit`、`AllocationFailed`。fallible allocation と上限検査を使い、部分的な変換結果を成功として返さない。error の診断へ raw 本文を持ち込まない。入力・返却 Vec を保持する caller 自身の privacy と出力先選択は別の責務である。

## 比較 profile と実呼出しの観測

wire は `api: "wptexturize"`、既存 typed Bytes の `input`、固定 `texturize_profile: "valid_utf8_no_lt_default_english_empty_filters"`、`reset: true` と必須 3 limits。比較 fixture は input 65,536／work 1,048,576／output 65,536 を固定指定する。製品の容量既定値ではない。

PHP oracle は pinned `wptexturize`／`wptexturize_primes`／`wp_spaces_regexp` と split helper を実呼出しする。`_x` は source にある英語 11 literal／context だけを scope 内で返す fixture で、実 gettext の互換性を示さない。関連 filter を空にし、shortcode registry と cockney の状態を限定し、global の存在・値を復元する。PCRE 失敗や scope／入力拒否を WordPress の正常な false 結果へ変換しない。

空 Bytes と文字列 `0` は source が初期化前に返す。それ以外の入力ごとに reset の初期化、`run_wptexturize` 1 回、`_x` 11 回、`no_texturize_tags` と `no_texturize_shortcodes` 各 1 回を確認する。`wp_spaces_regexp` の static cache は reset 対象ではなく、PHP process で最初の必要な呼出しだけ filter が 1 回動く。Reflection による前状態と各 case の実 hook 数を確認し、集計だけで差が相殺されることを許さない。

`scripts/texturize_profile.py` の規則 matrix、prime の曖昧例、全 ASCII byte（`<` を除く）、entity／Unicode／NBSP／最終 LF／`0`、決定的 fuzz を含む corpus は 9,183 件。HTML・不正 UTF-8・limits の拒否は別の protocol／native 契約へ分け、実 WordPress の正常返却値比較の件数へ混ぜない。

## 対象結果と全体ゲート

`target/texturize-targeted-comparison.json` と同名 `.log` は、新 9,183 件を 5 batches（2,048 件を 4 回と 991 件）で実比較し、全差分 0 を記録する。Cargo JSON の現 artifact 選択・SHA、native／PHP の捕捉 PID、profile／実 counter、所有 child の回収を確認した。現 artifact SHA は JSON に記録し、推測した古い binary を実行した結果へ置き換えない。

対象の非短絡は 9,178 件。calls／restored は各 9,183、initialized／run／no-tags／no-shortcodes は各 9,178、`_x` は 100,958、spaces filter は 5 PHP processes で計 5。比較全体の mixed batch でも実際の `compare_primitives.batches`（最大 2,048 件／1 MiB）から期待 spaces 数を再生成する。新件数だけの単純な割算で判定しない。

native の public 7 契約、prime helper private 4 契約と checked-work の private 1 契約、WP 全 target Clippy／担当 fmt が成功した。対象 log は `target/texturize-targeted-tests.log`、`target/texturize-checked-work-tests.log`、`target/texturize-clippy.log`、`target/texturize-fmt.log`。初回の試験側 compile assertion 誤りは `target/texturize-first-compile.log` に保持し、最初から成功した log と扱わない。PHP の profile／scope／拒否契約 7 件も成功した。

Python の profile 契約 4 件は `target/texturize-python-profile-tests.log` で成功。exact field／型、bool と整数の混同、実 counter と matching expected field の同時改竄、旧 73 の input／native／PHP 各 SHA の独立拒否、corpus 決定性を確認する。registry validator は mixed corpus の batch 期待を独立に再生成して照合する。

既存 73 API 486,153 件の順序・case ID・入力 SHA を保持して、新 9,183 件を末尾へだけ追加した。正式比較は 74 API／495,336 件、251 batches で成功した。新 subset の入力 SHA は `0f780e8c077a5325e6e5bf1440ce67913ad508e694cf6d2c2687aea05984c15c`。旧 73 の入力・PHP／Rust 両結果 prefix SHA、全 API 別件数と hooks／dispatcher の結果は `target/texturize-old73-preservation-check.log` で実照合し、保持を確認した。過去の prefix 検査も維持する。hooks は 572、dispatcher は 405、新 texturize の実 counter は上記と一致した。

全体 `target/texturize-full-check.log` は既定 profile の workspace Rust 566 件（ignore 1）、pure profile 293 件／58 test artifacts／features 空／通常 WP 1 package、独立 29 件、Python 353 件（347 成功・symlink 権限不足 6 skip）が成功した。生成・legacy・credential stage の 3 専用 crypto 比較、全 8 fresh DB profile 各 62 契約、Clippy、既定 profile の Linux cross-check、license 215 観測、HTTP／process smoke も成功し、全 DB fixture は回収済み。

最終既存 11 checker は `target/texturize-current-check-summary.json` ですべて exit 0。所有 controller PID 30124 も exit 0 で終了した。Linux cross-check と native Linux、pure native profile と Wasm compile／MSRV／security policy はそれぞれ別であり、今回の有限 profile の成功からこれらを推定しない。
