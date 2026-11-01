# 明示profileのusername前処理候補

後続全体実測：`target/username-preprocessing-and-credential-pipeline-full-check.log`でRust490＋独立29、Python214、正式PHP68 API・453,064比較（new67=2572／68=7414）、hooks572／dispatcher405、生成／legacy・8 DB fixture各56、Clippy・server/WP Linux check・license215・HTTPが成功した。最終10checkerも全exit0、元users/usermeta復旧と全fixture回収を確認。NoLiteral<とpost-stripUTF8/noIntl/explicitlocale/emptyfilters/Cというdomainや、非principalというPrepared型の境界は維持する。

2026-10-29。固定WP 7.1.3の `wp_strip_all_tags`（formatting.php:5600）、`sanitize_user`（同2149）、`wp_authenticate` 冒頭（pluggable.php:689）と `WP_User::get_data_by` のlogin前処理だけを分けて扱う。返却bytes・限定prefix stateの候補であり、login route、認証済みprincipal、認可、session/cookieや公開HTTPの完成ではない。

## wp_strip_all_tags：NoLiteralOpenAngle

API67は `wp_strip_all_tags_no_open_angle_bytes(raw, remove_breaks: bool, cap)`。入力はRawBytes、literal ASCII `<` を一つも含まないdomainに限定する。invalid UTF-8も許す。null/非string/coercion、一般HTML、PHP Cのtag parser移植は対象外。元sourceのscript/style除去はこのdomainではno-opとなり、残りは全NUL除去→任意のCR/LF/TAB/SPACE連続を一つのSPACEへ→PHP trim6（SPACE/TAB/LF/CR/NUL/VT）である。`>`、`/>`、entity文字列、非UTF-8 bytesはそのまま保つ。FFと内部VTを改行圧縮へ加えない。

nativeは最終trimを元inputのnon-trim境界選択へ融合する。NUL除去はその境界を変えず、collapse対象4 bytesもすべてtrim6に含まれるため、sourceと同じ結果になる。内部NULはrun判定より先にskipし、NULを挟んだwhitespace runが一つへ繋がることも保つ。同一walkerで最終長をchecked countしてからfallible reserve/emitする。全NULやtrim対象だけなら入力が長くてもcap0で空を返す。`"0"`は空にしない。final capを入力長の拒否へ転用しない。

先行target観測 `target/wp-strip-no-open-angle/` は固定PHP 8.5.12 `-n` のactual WP関数を2,552件（curated21＋全255 single-byte＋random1000、それぞれstrict bool両値）で比較し、差分・診断・PCRE errorは0だった。正式API67 corpusとは別の有限観測である。[PHP strip_tagsのNULL除去と注意](https://www.php.net/manual/en/function.strip-tags.php)、[固定8.5.12 sourceの初期state](https://raw.githubusercontent.com/php/php-src/php-8.5.12/ext/standard/string.c)も確認したが、HTML安全性・XSS防止を主張しない。

## sanitize_user：UTF-8 / no-Intl / C / empty filters

API68は `sanitize_user_utf8_no_open_angle_empty_filters_bytes(raw, strict: bool, explicit_locale, cap)`。localeはBytesでempty/単一`0`以外を明示し、Intl両normalizer無し、C locale、empty `sanitize_user` filterをreference profileとする。get_locale/site policyや任意hookをnativeに実装しない。元sourceの処理順を維持する。

1. NoLiteralOpenAngleのstripをremove_breaks=falseで実施する。
2. **strip後**の結果がvalid UTF-8であることを検査し、既存literal accent tableと明示localeを適用する。Latin-1 fallbackやNFCは行わない。
3. 元のその段階にある `%HH`（ASCII hex2桁）を単回削除する。percent decodeや再帰削除ではない。
4. `/&.+?;/` の最短matchを削除する。bodyは1 byte以上、LFを跨がず、即時`&;`は残る一方、`&;;`はmatchする。dotはCRを許す。
5. strict=trueならASCII `[a-zA-Z0-9 _.\-@]` だけを残す。lowercase化しない。
6. PHP trim6を行う。
7. 最後にCのwhitespace6種（09..0dとSPACE）の連続をSPACEへcollapseする。**この後で再trimしない**。

raw inputを最初からUTF-8拒否しない。例えば `c3 00 a9` はstripの内部NUL除去でéとして修復され、eへ置換できる。strip後もinvalidならUnsupportedValue。反対にFFはtrim6には含まれず最終C whitespaceには含まれるので、FFだけはS1=SPACE、FF＋A＋FFはS1=`" A "`となる。NFD combining sequenceはstrict=falseで残り、strict=trueではASCII外bytesを除去する。

処理はidempotentとは限らない。`%4%30A`→`%4A`→空、`%&x;61`→`%61`→空、Catalan `l&x;·l`→`l·l`→`ll`、`&a LF ;b`→`&a ;b`→`b`という一回目/二回目の差がある。新しく生成されたpatternへ同じpassを再帰適用しない。entity scanは失敗したLF/end suffixを再走査しない構造で、ampersandだけの長い列を二次時間へしない。

strip/accent/percent/entity・strictの中間Vecはunpooledで、最終capの外にある。最終的に削除される長い値へ、capを途中適用して誤拒否しない。最後のtrim/collapseだけ同一walkerでcount→cap→fallible reserve→emitとし、部分結果は返さない。入力・中間総量・allocator capacity・peak/RSS・concurrency・shared BudgetPoolをこのcapで制限したとは主張しない。

`StripAllTagsError` と `SanitizeUserError` はそれぞれUnsupportedValue / OutputLimit / AllocationFailedだけで、raw input・locale本文を診断に保持しない。sourceの型coercion、PHP warning、PCRE resource failure、任意filterの戻り値・side effectやbookkeeping、locale/charsetの自動選択、DB raw値の自動加工は未対応である。

## PreparedLoginInput：S1/P1とquery plan

追加のsource-derived own API `PreparedLoginInput::new(username, password, locale, cap)` はWordPress関数件数へ加算しない。S1=`sanitize_user(username,false)` のVecだけを所有し、P1=trim6(password)とlocaleは借用する。passwordのUTF-8化・NUL内部除去・hash生成は行わない。private stateでClone/Display/owned password escapeを持たず、Debugは全redacted。query planのDebugもusername/email keyを出さない。

- `username_stage1()` と `email_lookup_key()` はS1のborrowed bytes。email候補であり、email形式判定やSQL lookupを実行しない。
- `password_trimmed()` は借用P1。入力をcopy/変更しない。
- `username_php_empty()` / `password_php_empty()` は各段階のempty bytesまたは単一`0`だけをtrueとする。
- `login_lookup_plan()` はS1をtrim6してPHP-emptyならSkipped。それ以外は `sanitize_user` をもう一度実施してS2を `QueryKey(Vec)` とする。**S2が空でもQueryKey(empty)を保持し、二度目のempty checkを追加しない**。

例えばFFonlyはS1=SPACEでusername empty flag=falseだが、lookup時trimでSkipped。`&x;0`はS1=`0`でflag=true/Skipped。`%&x;61`はS1=`%61`なのでquery branchへ進み、S2が空でもQueryKey(empty)となる。これらはauth成功/失敗の判定ではない。password empty flag、email選択、任意authenticate callback、既存WP_Userの短絡、is_email、unknown-user処理や資格付与はここへ混ぜない。

capはS1と必要時S2のそれぞれの最終outputに適用し、両呼出しの中間やS1/P1全lifetimeのmemory budgetではない。借用accessorからcallerが明示copyすることや、非zeroizingなusername/query Vec・caller inputの残存は防がず、全process消去も保証しない。型名のPreparedは認証準備の一部を表すだけで、AuthUser/principal/token/sessionを生成しない。

## 契約と有限DB結合

担当pure契約はstrip public3/private1、sanitize public5、Prepared prefix public4の計13件。NUL・invalid UTF-8保持/修復、FF/VT順、strict・非再帰・最短LF境界、finalcap0/不足、S1/S2非同一性、PHP empty、query key空の保持、入力不変・P1借用・Debug秘匿を検査する。nativeとWP全target Clippyを通過した段階であり、正式API67/68 reference corpusの成功を先取りしない。既存API66以下のcorpus/profileは維持する。

別担当はowned fixtureの専用ID1001/login=`login`と公開dummy MD5で、追加check `credential_preprocess_lookup_pipeline`を構築する。raw username→S1/S2=`LoGIN`、raw password→P1、raw直接lookup不在と固定collationのS2 lookup一致、raw password不一致/P1検証一致、needs-rehash→型付き生成→raw credential CAS→fresh検証を有限に結ぶ。Skipped/QueryKey(empty)/`0`/FFの境界も確認し、元users3/usermeta5を復旧する。これを含め既存55から56 checksへ進む予定だが、この文書では実測成功をまだ記録しない。

結合はtest controllerが対象・入力・保存を明示した実験で、productionのusername/password policy、認証失敗の外部応答、rate limit、lockout、CSRF、capability、cache/action、auto rehashやHTTP endpointを採用するものではない。DBのWP path依存はdev-onlyのままで、normal runtime DB/serverへWP/crypto資格を渡さない。WP src/libをDB testが実呼出しするため、今回の変更は関連DB/alpha/binlog/bootstrap/rotation/restart証跡もstaleにする。親のfresh所有fixture再実測が必要で、手動hash restampをしない。担当はCargo・旧helpers・DB・正式proofを変更せず、DBを起動しない。

## API67/68のreference harnessと限定実測

上のnative検証段階の後、共通typed byte codecを使うprobe/oracle/corpus/registryへAPI67/68を接続した。API67はBytes・literal `<` 不在・exact boolだけ、API68はさらにstrip後valid UTF-8・明示PHP-truthy locale Bytesを要求する。空または単一`0`のlocale、型coercion、Latin-1 fallback、Intl両normalizerのいずれかが存在する環境を成功に含めない。C locale・noMbstring・noIntl・固定source/data identityをprofile validatorで型と値まで照合する。

PHP oracleは固定 `l10n.php:30` の実 `get_locale` をselected loaderで読み、throwする同名stubを廃止した。API68ではglobal `$locale` に明示Bytesを置き、実getterの最初のisset branchから `apply_filters('locale', ...)` を通す。DBへ進む別branchはfixture trapで拒否する。実getterのglobal宣言が未設定の `$wp_local_package` を作る場合も含め、`wp_filter`・`wp_filters`・`wp_current_filter`・`locale`・`wp_local_package` の存在/Null/値をfinallyで復元し、`wp_actions` と `wp_hasher` は不変を要求する。

実 `sanitize_user(raw, strict)` は各caseで一度だけ呼ぶ。strip後にhigh byteがある場合だけlocale hookが1回、それ以外は0回、sanitize_user hookは常に1回と照合する。empty filter registryでもPHP hook bookkeepingを実測するが、nativeの任意Dispatcherやbookkeeping対応を主張しない。PCRE error、非string結果、DB branch到達、想定外のglobal変更は比較成功にしない。内側の例外を入力本文や元exceptionを含まない固定診断へ変換し、finally復元後に失敗させる。

旧API66は引き続き明示2引数を実関数へ渡し、locale hook差分0とDB trap未到達でambient locale非使用を確認する。旧66 APIs全443,078入力と、そのうちremove_accentsの3,698入力はそれぞれ順序付きSHAを固定し、新caseを末尾へだけ追加する。API67は2,572件、API68は7,414件を生成し、正式全件数は**68 APIs / 453,064比較予定**である。PreparedLoginInputはこの件数へ加算しない。

`target/username-preprocessing-implementation-check.json` の限定actual比較は、旧API66の3,698件と新API67/68の9,986件、計13,684件で差分0だった。API68のlocale hookは0回が2,362件・1回が5,052件、sanitize_user hookは7,414回、DB branchは0回。型/domain/profile拒否は両実装合計78 processで確認し、stdoutへ成功値を出さず診断にも公開dummyのsecret markerを出さなかった。新Python scope/privacy契約5件、evidence契約全28件（追加2件）、比較case生成契約5件、既存accent契約5件、exporter `--check`、WP全target Clippy/fmtも通過した。

これらは限定比較・対象試験の成功であり、68 APIsの正式3比較証跡やgeneration/legacyの再実測、全8 owned DB fixture各56 checksを含む後続full gateの成功値ではない。今回の担当は正式proofを更新せず、rootによるsource freeze後の実再生成を待つ。中間allocation、公開/認証policy、一般HTML parserやsanitizer、任意locale/filter、実際のlogin成功へ対応範囲を拡大しない。
