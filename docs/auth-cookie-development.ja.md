# 準備済みuser・resolved saltからのraw認証cookie生成

2026-10-30現在：prepared user・解決済みsalt・完全なprepared session集合・明示nowを受ける[cookie照合部品](auth-cookie-verification-development.ja.md)をmainへ取り込んだ。ユーザー指定で中断中で、main後はfocused Rust21件／Python12件、fmt／Clippy exit0まで。先行targetのactual PHP30件・native両時刻60件・閉じたCLI拒否5件は別の履歴であり、main後のactual関係比較・全体gateは未実行。現在状態は `target/auth-cookie-verification-pause.json` に記録した。source／Cargo差があるため、以前の正式79 API／519,014件、crypto・DB・pure・inventory／license等の証跡を現在sourceの全体検証済みとは扱わない。認証済みprincipal、HTTP認証、CSRF、role／capability、session／key lifecycle、OIDCは未接続で、管理ready503を維持する。

以下の全体gate・依存観測は生成API76の完了時点の履歴。今回、private MAC builderを生成と照合で共有したが、生成の公開API・falsey token拒否・provider注入契約は維持する。共有変更後の生成public6件と既存private1件もfocused実行に含む。

`wp_generate_auth_cookie` の返却バイト列だけを扱う限定実装を追加した。callerが既存userに対応するlogin／password hash、解決済みsalt、既存tokenを明示する。user検索、session作成、認証成功やHTTPへのcookie設定は接続していない。管理側のready503、principal／session、認証後の記事編集・保存、theme公開、native Linux試用というαの未完了範囲は残る。

## 固定sourceと処理

参照は固定WordPress 7.1.3の `wp-includes/pluggable.php:951` と `wp_hash`（同2719）。ファイルSHA256は `3a2482a65b50d62ebae75a728be98b78985d3a3f5a3e007cb2d0c1338bf05777`。sourceのuser検索（952）、PHP偽値token時のsession作成（957–960）、salt解決、最後の `auth_cookie` filter（988）はこの候補へ含めない。callerは `auth_cookie` と `all` filter無しを前提にする。

hashが `$P$`／`$2y$` prefixならbyte offset8から最大4bytes、それ以外は末尾最大4bytesをfragmentとする（962–967）。prefixだけの分岐であり、hashの形式やpasswordの正しさは検証しない。短いprefixed hashではfragmentが空にもなる。

`login|fragment|expiration|token` をresolved saltでHMAC-MD5に通し、lowercase hexの **ASCII32bytes** をkeyにする。binary16bytesのdigestをそのままkeyにしない。このkeyで `login|expiration|token` をHMAC-SHA256に通し、lowercase hex64を作る。返却値は `login|expiration|token|hex64`。expirationは符号付きi64の十進表現で、負値を含め現在時刻との比較はしない。

## native契約

`PreparedAuthCookieUser::new(id, login, password_hash)` はpositive i64 IDだけを検査し、raw login／hashを借用する。IDは保持せず、行の存在・対応関係・認可はcallerの責任である。login／hashへUTF8、trim、delimiter、hash-formatの追加policyを課さない。

`wp_generate_auth_cookie_prepared_resolved_salt_empty_filters_bytes(&user, expiration, token, resolved_salt, limits)` はtoken `b""`／`b"0"` と空saltをcap／provider呼出しより前に拒否する。sourceならtoken偽値がsession作成へ進むため、その枝を生成済みtokenとして扱わない。salt `b"0"` は非空raw saltとして扱う。scheme／salt resolverをAPIへ持ち込まない。

`AuthCookieLimits::new(max_input_bytes, max_output_bytes)` の2上限は必須でDefaultを持たない。入力合計はlogin＋**全password hash**＋salt＋tokenのraw長で、fragment外のhash bytesも数える。型付きID／expirationと固定framingは入力合計へ含めない。出力上限は返却cookie全長を数え、checked arithmetic／fallible reserveで超過時に部分結果を返さない。借用入力の確保、provider状態、Vec capacity、共有budget、同時実行、CPU／peak memory／RSSの上限ではない。

返却owner `WpAuthCookie` は `as_cookie_bytes()` で借用できる。Clone／Display／Deref／可変・owned escapeを持たず、user viewと返却ownerのDebugはredacted。errorは `UnsupportedValue`／`InputLimit`／`OutputLimit`／`AllocationFailed`／`ProviderFailure` のclosed型で、入力やdriver値を保持しない。借用後にcallerが作るコピーはこのownerの管理外になる。

raw login／tokenにpipe、NUL、非UTF8等があってもframingへ保持する。従って結果はHeaderValueやparse可能なcookieのadmissionではない。Set-Cookie属性、secure／HttpOnly／SameSite、期限・失効、cookie検証、session永続化、RNG、nonce、principal発行は別途未接続である。

## providerと秘密値の寿命

既HMAC0.12はdigest0.10系列、MD5 0.11はdigest0.11系列のため、そのまま組み合わせられなかった。隔離provider調査ではHMAC-MD5に `hmac 0.13.0` を使い、既HMAC-SHA256の0.12を維持する案を検査した。現在のmanifest候補はoptional alias `hmac_md5` を加えた8 provider edge、auth専用7 test targetである。既定auth／pure分離の過去checkpointは [build profile](wp-build-profiles-development.ja.md)を参照。

`target/hmac-md5-provider-audit.json` は追加3package（hmac0.13.0、ctutils0.4.7、cmov0.5.4）のarchive SHAと隔離lockの一致、build script無し、MIT／Apache両license fileの存在を記録する。最大declared MSRV1.87はworkspace1.94以下だが、MSRV toolchainでの実build証明ではない。license選択・法務判断・security auditの完了にも読み替えない。

隔離RFC2202の3契約、fmt最終check、Clippy `-D warnings`、installed Linux std向けall-target cargo checkは成功した。ログは `target/hmac-md5-provider-{final-test,fmt,clippy,linux-check}.log`。初回fmt失敗は `target/hmac-md5-provider-style.log` に保持し、整形後に3契約を再確認した。Linux cross-checkはnative Linux実行ではない。

main側のoffline metadata resolveも成功した。`target/auth-cookie-lock-delta.json` によるlock差分は上記3packageの追加だけで、既存packageのversion変更・削除はない。既存recordの変化はdigest0.11.3とWPの依存edge追加で、独立binlog lockはbyte不変を確認した。このresolve観測だけで全体gate成功とは扱わず、下記の実gate結果と分ける。

変更後の依存一覧を実再生成し、unique218／core workspace167／isolated binlog188を確認した（`target/auth-cookie-dependency-inventory.log`）。license／notice file観測も218packageへ実更新した（`target/auth-cookie-license-observation.log`）。これらは固定lockに対する宣言・file観測であり、license選択・法務判断の完了ではない。

通常依存境界も実再観測した。`target/auth-cookie-normal-dependency-boundaries.json` のpublic serverは51 distinct package names、DBは100で、各境界の禁止対象は空だった。既存SHA2等の用途を含むgraphであり、暗号依存全般が存在しないという意味ではない。`target/auth-cookie-wp-feature-graphs.json` ではWP defaultと明示authのraw treeが一致し、distinct package namesは23、version別package数は28。過去build profileの21は名前数の観測なので28と直接比較しない。package単独pureの通常graphはWP自身の1名前だけを維持した。workspace全体のfeature統合やWasm／native Linux実行の証明ではない。

候補はsecret messageの連結bufferを作らずHMACへ順次入力する。所有decimal／digest／hex／key／出力bufferはZeroizingや明示wipeでbest effortに消去し、HMAC0.13のzeroize featureを有効にする。外側HMAC0.12の内部状態、compiler temporary、caller入力、allocator全領域の消去は保証しない。認証処理全体のconstant-time保証も主張しない。

## 検証状況

隔離provider、main offline metadata resolve、lock差分、依存／license file観測とnormal graph更新を確認済み。native新public6契約とprivate provider guard1契約は成功し、outer providerのdeprecated API置換後のtargeted Clippyもexit0（`target/auth-cookie-targeted-clippy.log`）。初回のtest file到着前compile失敗と外側providerのdeprecated warning記録も保持し、修正後の結果と区別する。到着前compileの記録は `target/auth-cookie-first-before-test-file.log`。

限定actual比較は新2,516case／5batchでPHPとRustが全一致した。`target/auth-cookie-targeted-comparison.{py,json,log}` に固定controller・結果・logを保持する。実計測のcalls／user lookup／hash salt fixture entry／auth cookie hook／restoredは各2,516、session／DB／cacheは各0。hash salt entryはactual `wp_hash` がscoped `wp_salt` fixtureへ入った回数であり、built-in HMAC呼出し自体のinstrumentationではない。hookの5引数は固定sourceに基づくwitnessで、observer callbackによる実測ではない。

actual resolverはloadしないためresolver全体の呼出し数を計測したとは主張せず、resolver counterを正式profileから除いた。option解決へのclosed tripwireは別の拒否契約である。今回の限定実行ではCargo JSONで選択した現binaryを用い、全tracked sourceの前後SHA一致と所有11 PIDの回収を確認した。入力・出力はpublic dummyであり、実cookie／saltの観測ではない。

正式76 API／510,813caseの比較を含む全体gateはexit0で完了した（`target/auth-cookie-full-check.log`）。default Rust596（ignore1）、独立pure300（60 artifact／features空／normal依存WP1）、isolated binlog29、Python384（378成功・symlink6 skip）、hooks572／dispatcher405、generation10／legacy113（actual88・refusal25）／credential stage33（actual25・refusal8）、全8 fresh DB profile各62、Clippy、default WP／server Linux cross-check、license file観測218、実HTTPが成功した。全所有DBを停止・回収した。Linux cross-checkはnative Linux実行ではない。

`target/auth-cookie-old75-preservation-check.log` は旧75のprimitive prefixとhooks／dispatcherの全件数・入力・両結果SHA保持を実確認する。旧証跡への手動restampで成功扱いしない。今回はcore／server対象sourceが不変のためWasmpubを再実測せず、static MIME phaseの既観測を保持した。最終11checkerはすべてexit0（`target/auth-cookie-current-check-summary.json`、controller58647も出力回収後exit0）。

これによりprovider trait不一致の迂回は本限定実装について解決した。αの認証成功・管理ready開放を意味しない。その後[explicit cookie parser](auth-cookie-parse-development.ja.md)は正式77としてmain採用・当時の全体検証を完了した。現在のprepared照合はさらに後続のmain採用で、静的API件数を追加せず、全体再検証は中断中。
