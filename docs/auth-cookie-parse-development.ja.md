# 明示cookieのraw field分解

2026-10-30現在：prepared user・解決済みsalt・完全なprepared session集合・明示nowを受ける[cookie照合部品](auth-cookie-verification-development.ja.md)をmainへ取り込んだ。ユーザー指定で中断中で、main後はfocused Rust21件／Python12件、fmt／Clippy exit0まで。先行targetのactual PHP30件・native両時刻60件・閉じたCLI拒否5件は別の履歴であり、main後のactual関係比較・全体gateは未実行。現在状態は `target/auth-cookie-verification-pause.json` に記録した。source／Cargo差があるため、以前の正式79 API／519,014件、crypto・DB・pure・inventory／license等の証跡を現在sourceの全体検証済みとは扱わない。認証済みprincipal、HTTP認証、CSRF、role／capability、session／key lifecycle、OIDCは未接続で、管理ready503を維持する。

以下はparser API77の完了履歴。本parser自身の5 raw fieldは未検証のままで、後続照合部品がcanonical expiry・MAC・prepared sessionを限定条件下で調べる。parser単体を認証判定へ読み替えない。

固定WordPress 7.1.3の `wp_parse_auth_cookie`（`wp-includes/pluggable.php:1013`）から、callerがcookie文字列を明示した枝だけを移植した。source SHA256は `3a2482a65b50d62ebae75a728be98b78985d3a3f5a3e007cb2d0c1338bf05777`。これはclient由来bytesの分解であり、認証済みuser／sessionや正しいcookieを返すAPIではない。

## 入力と返却view

`wp_parse_auth_cookie_explicit_bytes(cookie, scheme, max_input_bytes)` は `Result<Option<ParsedAuthCookie>, AuthCookieParseError>` を返すpure std helperである。cookieが空またはASCII `0` なら `UnsupportedValue` をcap検査より先に返す。sourceのその枝はSSL・cookie名定数・`$_COOKIE`を参照するため、本profileでは呼び出さない。

その他のcookieではraw cookie長＋raw scheme長をchecked arithmeticで合計し、必須capを超えれば `InputLimit`。その後、literal pipeがちょうど3個なら `Some`、それ以外ならsourceのfalseに対応する `None` を返す。末尾delimiterも空fieldとして数え、`|||` は4つの空fieldを持つ有効な分解結果になる。field数が合わない入力にも先にcapを適用する。

`ParsedAuthCookie` はusername／expiration／token／hmac／schemeの5つの借用sliceを保持する。schemeは空・未知名・pipe・binaryもそのまま保持し、既定schemeを補わない。expirationはraw bytesで、数値変換・時計比較をしない。tokenの空／`0`、非hex HMAC、NUL・非UTF8のfieldも分解段階では拒否しない。trim・正規化も行わない。

viewはClone／Display／可変・owned escapeを持たず、Debugをredactedとする。closed errorは入力を保持せず、source errorを連結しない。固定数の借用viewのため出力byte capやheap allocationを要しないが、入力bufferやcallerによる明示コピーの寿命・消去はcallerの責任になる。raw長capはCPU／RSS／同時実行／process期限の保証ではない。

## 生成との有限関係

[raw cookie生成](auth-cookie-development.ja.md)はlogin／tokenのpipeを許す。従って生成結果を常に4fieldへ戻せるというuniversal roundtripは成り立たない。delimiter無しのlogin／tokenという追加前提でのみ、生成framingと本parserのfield保持関係を確認する。pipeを含むloginまたはtokenから生成した結果は3個を超えるpipeを持ち、parserでは `None` となる。どちらの結果もhash検証・認証成功の証明ではない。

SSL判定、superglobal／cookie定数読取り、HTTP header admission、時刻policy、永続session検索・失効、principal発行は未接続。後続のprepared照合部品は明示now・解決済みsalt・caller準備sessionだけでMACと期限を照合し、本parserの責務は広げない。管理ready503やαの認証後編集・保存／theme公開／native Linux試用も未完了のまま維持する。

## preflightと採用後検証の区別

main採用前の `target/auth-cookie-parser-draft/` ではpure契約5件が成功した。`target/auth-cookie-parser-oracle-draft/contracts.log` のPHP/profile契約7件も成功し、actual named sourceの2,479caseを独立field-split期待値と照合した（parsed1,455／false1,024）。SSL／`_COOKIE`アクセスはtripwireとし、9globalの存在・null・値をfinallyで復旧、source例外の秘密canaryを報告traceへ出さない契約を含む。これらはPHPとnativeを同じ2,479件で比較した結果ではない。

main受入後のdefault契約6件（pure分解5＋auth feature付き生成関係1）が成功した。delimiter無しの関係と、pipe入りlogin／tokenの非roundtrip反例も実行済み（`target/auth-cookie-parse-native-targeted.log`）。targeted Clippy `-D warnings` はexit0（`target/auth-cookie-parse-targeted-clippy.log`）。Windows incremental hardlinkのcopy fallback警告は残るため、全出力が警告ゼロだったとは扱わない。

mainの限定actual比較も2,479case／5batchでPHPとRustが全一致した。calls／restored各2,479、parsed1,455／false1,024、SSL／cookie access各0を実観測し、全11 PIDを回収、対象sourceとbinaryの前後SHA一致を確認した（`target/auth-cookie-parse-targeted-comparison.{json,log}`）。これは上段draftのPHP単独preflightとは別の統合比較である。

probeの初回は `PhpArray` import漏れでcompile失敗し、`target/auth-cookie-parse-first-probe-missing-import.log` に履歴保持した。修正後のprobe Clippyはexit0（`target/auth-cookie-parse-probe-clippy.log`）。main PHP/profile9契約と旧auth10を混在させた19契約は成功した。draftの旧checkpoint hash `None` はdraft履歴だけに残し、main profileでは完成済みactual76の510,813件・入力と両結果SHAへ拘束した。

`target/auth-cookie-parse-full-check.log` の全体gateはexit0で完了した。workspace Rust602（ignore1）、native pure305（61 artifact／features空／normal依存WP1）、isolated binlog29、Python393（387成功・symlink6 skip）、正式77 API／513,292case・hooks572／dispatcher405、generation10／legacy113（actual88・refusal25）／credential stage33（actual25・refusal8）、全8 fresh DB各62、Clippy、WP／server Linux cross compile、license file観測218、実HTTPが成功した。全所有DBを停止・回収した。Linux cross compileはLinux native実行ではない。

`target/auth-cookie-parse-old76-preservation-check.log` で旧76の入力順・全件数・両結果SHA保持を確認し、比較前の旧3 snapshotはauth76 checkpointのproof SHAとも一致した。最終11checkerはすべてexit0（`target/auth-cookie-parse-current-check-summary.json`、controller42156も出力回収後exit0）。root／WP manifest、root／binlog lock、inventory／licenseの6ファイルは、現在bytes＝phase開始snapshot＝完了auth76 checkpointの各SHAという実assertに成功した。DB fixture directoryも空を確認済み。parser checkpointはその後 `target/auth-cookie-parse-checkpoint.json` として生成された。

前段auth76は `target/auth-cookie-checkpoint.json` に完成履歴として保持し、今回source差を含む対象証跡は実gateで再生成した。過去checkpointを現在sourceの成功へ読み替えたり手動restampしたりしない。独立reviewは `target/auth-cookie-parser-review.txt` に記録した。core／serverが不変のためWasmpubは今回再観測せず、static MIME phaseの既観測を保持する。管理認証／ready503は未接続のまま。その後sanitize_textは[正式78／79の完了履歴](sanitize-text-development.ja.md)となった。現在の後続工程はprepared cookie照合のmain取り込み後の中断状態であり、これらの過去checkpointは現在sourceへの全体成功証明ではない。
