# sanitize_titleの明示default callback候補

全体checkpoint（2026-10-29）：`target/sanitize-title-and-post-name-retry-full-check.log` exit0。Rust529＋独立29（ignore1）、Python248実行（247成功・1skip）、正式71 API466,887（新outer5365）、全8 fresh DB60、Clippy・server/WP Linux check・license215観測・実HTTPが成功。旧70 input/result SHAを実prefix再計算で保持、最終11checker全exit0（`target/sanitize-title-current-check-summary.json`）、全fixture回収済。初回DB試験のmeta行数assert訂正を含め再測定した結果で、native Linux実行の成功ではない。

固定WP 7.1.3 `wp-includes/formatting.php:2228–2269` と `default-filters.php:309` の返値に限定する。native APIは `sanitize_title_utf8_default_callback_explicit_locale_bytes(raw, fallback, context, locale, max_output)`、contextはSave／Display／Query。rawはliteral `<`無し・whole valid UTF-8、localeは空と単一`0`以外の明示Bytesを全contextで要求する。NoIntl／no-mbstringで、既定 `sanitize_title_with_dashes` callback一つ（priority 10、accepted args 3）の返値だけを構成する。任意filter、Dispatcherの登録・実行・counter・global状態の互換を意味しない。

sourceは最初にraw_titleを保存し、Saveだけremove_accentsを実行してからfilterへ渡す。raw_titleはaccent除去前だが固定callbackでは未使用。nativeもSaveだけ既存explicit-locale accent kernel→TitleDash Save、Display／QueryはTitleDash Displayへ渡す。Query wrapperはfallback空・context queryであり、callbackでの変換はDisplayと同じ。get_localeやsite設定を選ばず、明示locale値でsourceの辞書分岐を再現する。NoIntlのためNFCは実行しない。

accent除去はNUL stripや200-byte URI prefixより先である。SaveのÄはlocale en_USでa、de prefixでaeとなり、direct TitleDashの`%c3%84`とは異なる。£の削除も200制限より先なので、その後の文字がprefixに入る。raw `c3 00 84`はouterのwhole UTF-8 domainでは拒否し、direct TitleDashのpost-NUL修復域まで同一と主張しない。

callbackの結果が空の場合だけfallbackを**無加工で**選ぶ。`0`は空判定対象でない。sourceのstrict false判定は存在するが、固定callbackはfalseを返さない。fallbackは任意Bytesで、invalid UTF-8・NUL・literal `<`・percent列もそのまま返す。入力のNoLiteral<／UTF-8 guardをfallbackへ転用せず、fallbackをsanitizeし直さない。従ってURLやHTMLの安全性、route採用、slug一意性や公開許可を保証しない。

中間のaccent／callbackには最終capを適用しない。最後に選択した完成bytesだけをmax_outputと比較する。非空candidateはowned Vecを再利用し、fallback選択時だけcount済みの長さへfallible reserveしてcopyする。エラーはUnsupportedValue／OutputLimit／AllocationFailedの固定分類で、fallbackに潰さず返す。Debug／Display／Error sourceに本文・fallback・localeや任意診断を保持しない。入力・中間確保・capacity・RSS・peak・並行実行・shared poolの上限は含まない。

公開6契約はlocale別Save／Display／Query、accentとURI200の順序、空／0とraw fallback、whole-input admission、cleanup・fallback後だけのcap、入力不変と閉errorを対象とする。private契約はcandidate allocationの再利用とhelper error分類を確認する。対象native試験・正式PHP比較・全体gateの成功は実行後にだけ記録する。WordPressのsanitize_title全域、型coercion、他context値、invalid raw入力のPHP経路、任意hooks、slug／status／認証／公開policyは採用しない。WP source変更はDB dev-only pipelineのprovenanceもstaleにするため、親の実再測定を必要とし、正式proofを手動更新しない。

2026-10-29の対象native検証ではprivate1件・public6件が成功し、WP crate全target Clippy（offline/locked、-D warnings）と担当Rust file限定rustfmtが通過した。incremental hard-linkのcopy fallback環境警告はあったが終了codeは0。正式PHP比較・Linux実行・全体gate・DB再測定はこの対象確認には含まない。
## 限定actual比較と既定callbackの根拠

正式harnessは固定 `default-filters.php:309` を全file SHA-256・行・唯一のliteral statementでdata-only検査し、`sanitize_title / sanitize_title_with_dashes / priority10 / accepted_args3` のtupleだけを取り出す。全default-filters.phpをPHPとして実行せず、実 `add_filter` でその1callbackを登録し、all hookや追加callbackがないことを前後で照合する。oracleは実 `sanitize_title` とremove_accents/title helper/get_locale/mbstring binary-safe helpersをselected loaderで呼ぶ。sourceが定義するraw_titleの捕捉順を維持し、observer filterを追加して引数を書き換えない。

profileはraw NoLiteral `<`・whole valid UTF-8、全contextでPHP-truthy locale Bytes、任意fallback Bytes、Save/Display/Queryの明示、noIntl extensionと両normalizer不存在、noMbstring/関連function不存在/Cだけである。fallbackは不正UTF-8/NUL/markupを含んでも加工しない。rawの不正UTF-8はNUL削除後や200byte打切り後に修復される場合でもこのprofileでは拒否する。fixture final host cap65536をmetadataへ束縛し、中間上限や製品の公開slug policyへ読み替えない。

scope内はfilter globals3個・locale・wp_local_package・actions・hasherの7 globalsをfinallyで存在/Null/値まで復元する。実title filterは各1回、locale hookはSaveかつrawにhigh byteがある場合だけ1回、その他は0回でなければ失敗する。DB locale branchはfixture trapで未到達を確認する。callback例外・型/domain/profile拒否はfallbackへ変換せず、固定診断へ閉じる。nativeの値の互換とPHP hook bookkeepingは区別する。

限定実比較は5,365件（Save2,825 / Display1,270 / Query1,270）で差分0。実reference/filter callsとglobal復元は各5,365回、locale hook0回が2,956件・1回が2,409件だった。全311 accent keysと6 locale枝、GBP削除がURI200より前である境界、NFD、literal sentinel、空/`0`、任意fallback bytes・非再帰処理を含む。bad-protocol拒否46 processを両実装で確認し、PHP試験は `display_errors=stderr` としてstdoutの成功値・診断のsecret marker漏洩を認めない。

新Python5契約はdata-only抽出、actual192状態組合せと失敗192組合せの7globals復元、fallback非加工/zero保持、両normalizer存在の個別拒否、callback registration形と例外秘匿を検査する。evidence全32件（追加2件とmandatory-field拒否）、既存case generator5件、WP全target Clippy/fmtも通過した。旧70 API全461,522入力の順序とSHAを固定し、正式全比較時には旧prefixの両結果SHAも実再照合する。

71 APIs / 466,887比較はこの時点では後続正式gateの予定値で、今回の限定成功を正式3proofやcrypto relation/DB fixtureの再認定として扱わない。正式証跡はrootのfreeze後fullgateで実生成し、手動restampしない。このprofileはunique slug、予約名、fallbackのURL妥当性、投稿status/date、保存hook、HTTP/auth/DB/Wasmの製品policyを採用するものではない。
