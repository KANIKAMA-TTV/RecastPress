# 既定bcrypt rehash hintの限定移植

後続の正式実測：65 API・439,380ケース（新2,182件・3hooks各2,182）が`target/credential-pipeline-final-primitives-remeasure.log`で差分0。hooks572／dispatcher405も再実測した。Rust471＋独立29、Python200、全8 DB fixture各55契約、生成／legacy比較、Clippy・server/WP Linux check・license215・HTTPを通過。current source/binary checkerも全exit0。predicateを使うowned credential pipelineはDB記録へWP implementationも追跡し、実fixtureで再生成した。boolがpassword一致や自動更新許可を意味しない境界は維持する。

2026-10-29。固定WordPress 7.1.3 `wp-includes/pluggable.php:2911` の `wp_password_needs_rehash` を、明示Bytes・空custom hasher・空の3filters・user_id=0・PHP 8.5.12既定bcrypt/optionsの範囲で移植する。認証やpassword照合、hash生成、保存済みcredentialの更新、rehash時期・製品cost方針を採用するものではない。

Native APIは `wp_password_needs_rehash_default_bcrypt_bytes(&[u8]) -> Result<bool, CompatibilityError>`。source-derived registryの65番目の公開WordPress関数候補として追加した。元64 API・437,198ケースの入力とIDは維持し、旧input SHA-256とも一致を確認した。正式3proofの再実行はroot担当であり、この文書作成時点では更新していない。

## source順序と対応入力

元関数は非空custom hasherならfalseを返し、その場合3filtersを呼ばない。本subsetはcustom hasherが空であることを前提とする。実関数は `wp_hash_password_algorithm`、`wp_hash_password_options` を順に呼び、literalかつcase-sensitiveな `$wp` prefixを判定する。既定bcryptで非prefixedならtrue、prefixedなら先頭3bytesを除去してPHP builtin `password_needs_rehash` に渡す。最後に `password_needs_rehash` filterを呼ぶ。

Nativeの対応範囲は次の通り。

- `$wp` で始まらない入力は任意Bytesでtrue。空、`*`、MD5、portable phpass、生bcrypt、NUL、不正UTF-8、8192bytes等の長い値もこの分岐である。先頭空白や大文字 `$WP` を正規化しない。
- `$wp` 付きはexact63 ASCII bytes・literal `$wp$2y$`・2桁cost4..31・canonical salt/checksumに限定する。既存bcrypt verify kernelと同じ固定provider `bcrypt::HashParts` parserを再利用し、costが12以外ならtrue、12ならfalseとする。
- その他のprefixed入力は `CompatibilityError::UnsupportedValue`。PHPが返すtrue/falseを推測して補わない。

password自体を受け取らず、trim・UTF-8変換・4096 password guardはない。cost31もmetadataとして解析するだけで、bcrypt/HMAC等のKDF計算やRNGを呼ばない。このためhost work allowanceは不要だが、metadataのboolはpassword一致やhashの有効性・安全性を証明しない。provider parserの短いsalt/checksum allocationはshared poolや実RSSの会計に含めていない。

## PHP builtinの緩い認識との境界

PHP 8.5.12の公開tag sourceではbcrypt識別、長さ等の確認、`sscanf`によるstored cost取得、既定costとの比較を行う。完全なbcrypt検証やcanonicalなsalt/checksum検査と同一ではない。[PHP 8.5.12 password.c](https://github.com/php/php-src/blob/php-8.5.12/ext/standard/password.c)

実PHPの公開dummy49観測では、cost12の不正body、NUL・非ASCII body、非canonical末尾bits、`XX` cost、符号付きcost等でもbuiltinがfalseを返す例があった。これらを含むPHP builtin認識全体は本subsetの対応範囲ではない。反対に、生bcrypt cost12はbuiltin単独ならfalseでもWordPressは非`$wp`分岐でtrueになる。prefix除去前後を混同しない。観測は `target/password-rehash-observation.php` と `.json` にあり、正式corpusへ49件を加算しない。

## oracle・profile・検証

実PHP oracleはselected loaderで固定sourceの実 `wp_password_needs_rehash` だけを抽出し、各受理caseで1回呼ぶ。DB/userロードや全pluggable bootstrapはしない。4globals `wp_filter`・`wp_filters`・`wp_current_filter`・`wp_hasher` を保存し、空profileで呼び出してfinallyで復元する。actionsは変更しない。3filters各1回、空current context、未設定/null/sentinel/空状態の復元、内側の例外時の復元を検査する。nativeのfilter bookkeepingや任意callback/typed returnは対応外である。

`wp_rehash_profile.py` とPHP出力のprofileは、PHP version、source SHA、Bytes、empty hasher/filters、user_id=0、algorithm/options、cost12、conditional domain、native bookkeeping=false等を厳密な型と値で照合する。既定costは既存公開dummyに対するbuiltin metadata検査で12のみfalse・11/13はtrueであることも実測し、profileへ記録する。KDFを実行して既定costを推測しない。

正式比較入口には2182ケースを追加した。非prefixedの全256byte変種・長入力・固定seedのbyte列と、全cost4..31・salt/checksumのcanonical境界・固定seedのcanonical bodyを含む。限定実PHP比較は差分0件、3filters各2182回。正式全比較の予定合計は439,380ケースだが、正式3proof全再実行の成功を先取りしない。証跡validatorは両分岐、全28costの正件数、3hook countのcase総数一致、profileおよび各必須fieldの欠落・bool/int/float混同を拒否する。

Rust契約3件、synthetic PHP wrapper契約3件、追加evidence契約2件が通過した。wrapper試験はactual-WP差分とは区別し、内側の成功・例外・型/対応外domain拒否を検査する。全体gate前の対象Clippy/fmt、comparison generator試験、26 processの両protocol拒否も通過。`SensitiveParameter`を新wrapperの両引数へ付け、trace引数表示を明示的に有効にした試験でも公開secret markerがstderrへ出ないことを確認した。

未対応はnon-Bytes coercion/診断、非空hasher、任意algorithm/options/filter、PHP builtinの緩いprefixed認識全体、Wasm境界、credential書込、HTTP/DB・session・login・製品rehash policyへの接続である。新依存や製品のmanual設定を追加していない。
