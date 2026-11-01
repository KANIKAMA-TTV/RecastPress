# prepared raw 投稿と標準 4 filter の get_the_title 候補

固定 WordPress 7.1.3 の `get_the_title` を、caller が準備した raw 投稿・frontend・標準英語 callback に限定して扱う。対象比較に続き、`target/title-default-full-check.log` は exit 0 で完了した。正式 75 API／508,297 件と全 8 fresh DB 構成各 62 契約、公開 fixture 等が成功した。最終既存 11 checker も全 exit 0 を確認した。以下の限定 profile の結果であり、一般の theme compatibility や認証の完成ではない。

## Consumer と固定した前提

`wp-includes/post-template.php:118-174` の実 getter を呼ぶ。実 `WP_Post` constructor（`class-wp-post.php:311-315`）へ positive signed i64 の ID、raw title、status `publish`、password 空文字または ASCII `0`、`filter='raw'` を明示する。password `0` は source の PHP-empty と同じ扱いで、一般の password policy を新設するものではない。

`get_post`（`post.php:1138-1178`）は WP_Post instance 枝を通っても `filter('raw')` を呼ぶ。`WP_Post::filter` の同値 raw 枝（`414-416`）だけを通すため、`filter` を unset や別値で代用しない。そうすると `get_instance`／cache／DB へ入る場合がある。class file は定義だけを読み、関数は既存 immutable reference loader で選択する。通常の WP bootstrap は実行しない。

実 `is_admin`（`load.php:1356-1363`）には、`current_screen` を scope 内で unset し、`WP_ADMIN` が未定義であることを先に確認して入る。既存 constant を書き換えず、条件違いは拒否する。固定 publish／PHP-empty password により、`Protected:`／`Private:` の翻訳・prefix branch へ入らない。global post、numeric ID lookup、nullable／不正 ID、編集／管理 context を補完しない。

この prepared 値は DB row の公開選別、principal、role、visibility や公開承認ではない。実 getter の分岐前提を caller が明示するモデルであり、既存 DB／公開 runtime へ接続しない。

## 標準 callback の実 dispatch

`default-filters.php:197-199` と `170-171` の固定 tuple を data-only で検査する。ファイル全 SHA、直接登録 3 行の唯一性、capital_P の loop と `the_title` 所属を確認し、file 自体を実行して全 default filter を導入しない。

| function | priority | accepted_args |
| --- | --- | --- |
| `wptexturize` | 10 | 1 |
| `convert_chars` | 10 | 1 |
| `trim` | 10 | 1 |
| `capital_P_dangit` | 11 | 1 |

上記を実 `add_filter` で実名登録し、実 `WP_Hook` の priority／順序／accepted_args を前後で検査する。getter は title と ID を filter へ渡すが、`wptexturize` に ID を第 2 引数として渡して reset を有効化しない。実 `capital_P_dangit` は実 `current_filter()` の `the_title` branch を通す。別 context の直接呼出しは同じ処理ではない。

`wptexturize` の英語初期化は各 getter の前に固定 nonempty `prime`／`reset=true` で別枠実行する。既存 `oracle_texturize` wrapper は呼ばず、旧 texturize 比較 counter を汚さない。prime は `_x` 11 回、run／no-tags／no-shortcodes 各 1 回。getter 本体では reset が既定 false なので `_x`／run は 0、title が空文字と `0` 以外なら no-tags／no-shortcodes 各 1 回となる。spaces filter は PHP process 内の static cache を Reflection で確認し、最初の必要な初期化時だけ 1 回を別計測する。

`convert_chars` は texturize と別の、2 bytes を消費する source regex である。例えば元 `&a;` は、この全 pipeline では `&#038;a;` になり得る。texturize の ampersand 処理だけで代用しない。trim は PHP の 6 byte whitespace、capital_P は title branch の全位置 `Wordpress`→`WordPress`。NBSP 等を勝手に trim・Unicode 正規化しない。

## Native API と上限

```rust
PreparedTitlePost::new(id, raw_title, password, status)
get_the_title_prepared_frontend_default_english_no_html_bytes(
    &post, TexturizeLimits::new(max_input_bytes, max_work_bytes, max_output_bytes),
) -> Result<Vec<u8>, TitleFormatError>
```

constructor は ID／status／password の上記 subset を確認する。title は借用し、getter で input cap を先に、その後 whole valid UTF-8／NoLiteral `<` を検査する。view は ID を principal として保持しない。error は `UnsupportedValue`／`InputLimit`／`WorkLimit`／`OutputLimit`／`AllocationFailed` の閉じた型。

texturize の中間出力、convert pass、trim、capital_P を順に扱い、最終 output cap は全 callback 相当処理の後だけに適用する。途中の長い title が trim で短くなる場合に、inner texturize の final cap を outer final cap と混同して早期拒否しない。work は各作業 buffer の論理長で、累積 allocation、cache、Vec capacity、RSS／peak／CPU／共有 memory pool の保証ではない。

返却値は entity syntax を含む表示用の整形 bytes で、文字だけの prepared-post renderer と同一の型契約ではない。その renderer へ安易に渡すと `&` を再escapeする可能性がある。HTML escaping／sanitization、HTML 属性や任意 URL の安全性、公開判定をこの結果から推定しない。view／error の Debug は値を隠すが、返却 Vec の取扱いは caller の責務。

## Oracle wire、privacy と scope

API 名は `get_the_title`、profile は `prepared_raw_publish_frontend_default_english_no_html`。title は既存 typed Bytes の `input`、`post_id` は typed positive i64、`post_password` と `post_status` は typed Bytes。`post_filter='raw'`／`frontend=true` と、input 65,536／work 1,048,576／output 65,536 の 3 fixture caps を必須指定する。これらは比較用の明示値で、製品既定ではない。

PHP の入口 `title_input`／`oracle_title` は、型と domain の拒否・source の失敗を固定診断へ変換し、元例外を previous として保持しない。ユーザー値を受ける helper／trap の引数へ SensitiveParameter を付ける。private helper を直接呼ぶ場合までを公開 parser API として保証しない。実 `wp_oracle.php` の get_option tripwire にも引数の redaction を適用し、test 用 stub の成功だけで代用しない。

scope は wp_filter／wp_filters／wp_current_filter／wp_actions／post／current_screen／shortcode_tags／wp_cockneyreplace／texturize_scope／title_scope／post_meta_cached_scope／wpdb／wp_hasher／wp_object_cache の存在と値を保存し、成功・source failure とも finally で復元する。WP_Post の raw field も前後一致を検査する。function static 全体の rollback ではなく、texturize／capital_P の状態を admission と priming で限定する。

既存の cache fixture 関数に title active 時の実 tripwire を接続し、衝突する cache 関数を新たに再定義しない。SQL trap は property／method access を数えて拒否する。get_option／sanitize_post／翻訳も閉じ、成功 case の DB／cache 呼出しは 0。関連しない getter、cache population や翻訳の空実装で処理を継続しない。

実 wrapper 呼出し、実 `the_title` と内部 hook、prime の `_x`、callback 登録形状、tripwire、scope 復元を記録する。get_post／WP_Post::filter／is_admin／4 callback 各々の内部呼出し数を手加算して「実測」とはしない。

## 対象検証と全体ゲート

`scripts/test_title_oracle.py` の 8 契約は `target/title-oracle-contracts.log` で成功した。実 constructor／raw getter／4 named callbacks、`XWordpress` の title context、空文字／`0`、一文字 entity の convert 差、trim／NBSP、prime と既存 texturize counter 分離を確認した。

拒否 fixture、14 globals の absent／Null／空 array／object sentinel の復元、source failure の固定診断、cache／DB／option／sanitize／翻訳の実 tripwire、変更した callback source の data-only 拒否、実全 glue の get_option 引数非漏洩も検証した。source failure 用の置換関数は負例契約であり、actual getter の正常比較へ加算しない。所有 PHP child を有限 timeout で回収し、外部 HTTP／DB／Cargo をこの試験から起動しない。

native は public 6 契約と convert の private 契約、Clippy／fmt を別担当で確認した。profile／corpus の Python 6 契約も成功した。`target/title-targeted-comparison.json` と同名 `.log` は、新 getter 12,961 件／7 batches の実 PHP／native 差分 0、16 counter 一致と所有 child 回収を記録する。active title は 12,944 件で、getter no-tags／no-shortcodes 各 12,944、prime `_x` は 142,571、prime spaces は 7、restored は 12,961。getter `_x`／run、DB／cache は 0。異常 protocol の拒否は別契約で、実 WordPress の正常返却値比較へ加算しない。

正式比較は 75 API／508,297 件、258 batches で成功した。新 getter は 12,961 件。旧 74 全件の API 別件数・入力・PHP／Rust 両結果 prefix SHA と hooks／dispatcher の結果を `target/title-default-old74-preservation-check.log` で実照合し、完全保持を確認した。hooks は 572、dispatcher は 405 のまま。

全体 `target/title-default-full-check.log` は既定 workspace Rust 573 件（ignore 1）、pure profile 300 件／60 test artifacts／features 空／通常 WP 1 package、独立 29 件、Python 368 件（362 成功・symlink 権限不足 6 skip）が成功した。3 専用 crypto 比較、全 8 fresh DB profile 各 62 契約、Clippy、既定 profile の Linux cross-check、license file 215 観測、HTTP／process smoke も成功し、全 DB fixture は回収済み。

固定投稿タイトルを所有 bundle へ渡す別の公開 fixture も gate 内で再確認した。`target/title-default-publication-observation.json` は body 310／bundle 440／receipt 117 bytes、GET の exact bytes、HEAD の長さ／ETag、304、異なる Host の 421、query 字面一致、起動後に両入力ファイルを変更しても読取り済み内容を配信する 6 checks を記録する。所有 child／reader は回収済みで、Windows の強制終了 exit 1 を graceful exit 0 と扱わない。初回 sandbox の loopback WinError 10013 拒否は `target/title-default-publication-first-sandbox-refusal.log` に保持し、後続の成功で消していない。これは固定公開 dummy の有限経路で、DB／認証／theme SDK や自動公開へ接続したものではない。

最終既存 11 checker は `target/title-default-current-check-summary.json` ですべて exit 0。所有 controller PID 88645 も exit 0 で終了した。native Linux／Wasm／MSRV／一般の theme compatibility の成功は主張しない。
