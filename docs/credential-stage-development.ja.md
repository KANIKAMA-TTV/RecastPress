# username/email credential stage（未認証のraw候補）

2026-10-30現在：prepared user・解決済みsalt・完全なprepared session集合・明示nowを受ける[cookie照合部品](auth-cookie-verification-development.ja.md)をmainへ取り込んだ。ユーザー指定で中断中で、main後はfocused Rust21件／Python12件、fmt／Clippy exit0まで。先行targetのactual PHP30件・native両時刻60件・閉じたCLI拒否5件は別の履歴であり、main後のactual関係比較・全体gateは未実行。現在状態は `target/auth-cookie-verification-pause.json` に記録した。source／Cargo差があるため、以前の正式79 API／519,014件、crypto・DB・pure・inventory／license等の証跡を現在sourceの全体検証済みとは扱わない。認証済みprincipal、HTTP認証、CSRF、role／capability、session／key lifecycle、OIDCは未接続で、管理ready503を維持する。

このstageはusername／password経路のraw候補を返す部品で、後続cookie照合とは別の入口である。両者をつなぐsession発行や通常ログイン経路は実装していない。以下の全体checkpoint・stage33関係は当時の成功履歴として保持し、共有WP source変更後のcurrent証跡へ読み替えない。

全体checkpoint（2026-10-29）：`target/credential-stage-and-pipeline-full-check.log` exit0。Rust498＋独立29、Python222、正式68 API453,064、別枠stage33（actual25/refusal8）、全8 fresh DB57、Clippy・server/WP Linux check・license215・実HTTPが成功。最終11checker全exit0、`target/credential-stage-current-check-summary.json`。全fixture回収、db-runs空。以下の担当段階の記述は途中経過を含み、全体gate完了はこの段落に記録する。

2026-10-29。固定WP 7.1.3 `user.php:153/242` のusername/email password callbacksの関係を、既存PreparedLoginInput・raw password dispatcher・needs-rehash hintと組み合わせる純粋候補。型名は `CredentialStage` と `UnfinalizedCredentialMatch<R>`。照合したraw rowとmetadata intentを返すだけで、認証済みprincipal、session、認可、IDのPHP int castやlogin permissionを生成しない。

前提はinitial user=null、previous WP_User無し、関連filters empty、default bcrypt hint、single-site相当の限定関係である。application passwords、multisite/spam、任意authenticate/`wp_authenticate_user`、prior WP_Errorの完全保持、HTTP errorや翻訳済みWP_Error message、role/capability、cookie/nonce、lockout/rate limitは未実装。元default-filters.php:516/517のusername→email順に対応する有限関係であり、全 `wp_authenticate` / `wp_signon` / login routeの同等性ではない。

## 外部lookupと消費型state

`CredentialStage::begin(prepared, HostPasswordWorkLimits, hash_getter: fn(&R)->&[u8])` はPreparedLoginInputを消費し、両typed costを必須保持する。新crate・DB依存・外部package・runtime/async配置を追加しない。S1/P1/localeは既存prefixの所有・借用契約を継承する。

| state | hostが行う操作 |
| --- | --- |
| `NeedLogin(LoginLookupRequest)` | borrowed `query_key()` のS2でlookupし、一度だけrespond |
| `NeedEmail(EmailLookupRequest)` | borrowed `query_key()` のtrim6(S1)でlookupし、一度だけrespond |
| `Complete(CredentialStageOutcome)` | terminal。respond操作は存在しない |

replyは `CredentialLookup::{Found(R),NotFound,LookupUnresolved}`。DB errorをNotFoundへ潰さない。Login/Emailのrequest型を分け、private fieldsとconsuming respondによりphaseを取り違えて再利用しない。state/request/lookup reply/matchはClone/Displayを持たず、すべてのnew Debugはgeneric RのDebugを呼ばず常にredactedである。記録ID・password/hash・username/email key・localeをDebugへ出さない。

generic RにはDB DTOもopaque fixture recordも使える。hostがqueryとrowを正しく対応させること、getterがそのrowのhashを返すことはtrusted adapterの前提である。getterはFoundごとに一度だけ呼び、machineがその同じborrowed hashを内部のcombined verifyとmetadata hintへ渡す。外部からtrue/falseだけを返して、別Rへ差し替えるverification返信APIは作らない。ただし虚偽のhost、getterのpanic/side effect、Rのinterior mutability、staleな外部row/source identityまでgeneric型で証明するsecurity boundaryではない。

## 正確な分岐

S1またはP1がPHP-empty（empty bytes/単一`0`）ならlookup前にNoCredentialMatchで停止する。login planのSkippedはusername不在相当として次段を検討する一方、S2の空/`0`はQueryKeyとして実際にhostへ渡す。`%&x;61`→S1=`%61`→S2空、`%&x;310`→S1=`%310`→S2=`0`を再empty判定で消さない。

username Found(R)ではP1をrawのまま内部 `wp_check_password_raw_empty_filters_bytes` へ渡す。matchならmetadata hintを計算して即terminalとし、emailを一切検討しない。NotFoundまたは確定Ok(false)のときだけemail候補へ進む。raw P1の4096超をstage全体の入口で拒否しない。既存dispatcherのMD5先行と、残りの形式解析前4096 guardを保持する。

email判定は**S1**へ既存 `is_email` を実行する。callerのmutable Dispatcherを受けず、privateなfresh empty Dispatcherを一度だけ使う。expected resultはfalse、またはS1そのもののBytesだけであり、予期しない型・変更されたbytes・HookErrorはEmailProfileのUnresolvedで停止する。HookError内の任意messageは保持しない。accepted S1をtrim6したkeyでemail lookupを要求し、sanitizeをもう一度かけない。例えばS1=`%31@example.invalid`に対するlogin S2は`@example.invalid`だが、email keyは前者である。email NotFound/不一致はterminal NoCredentialMatchで、usernameへ戻らない。

lookup failure、UnsupportedHash、BcryptCostLimit、PortableCostLimit、ProviderFailure、RehashMetadata、preprocessing失敗はclosed `Unresolved {at, reason}` にする。falseや通常のwrong passwordと見なしてemailへ進まない。未対応username hashが実sourceで成功し得る状況を無視して、別email rowを誤って選ぶことを避けるためである。この停止はprototypeの安全な未解決扱いで、sourceのunsupported branchを失敗だと断定するものではない。

## matchと残されたpolicy

`UnfinalizedCredentialMatch<R>` のaccessorは `record()->&R`、`matched_via()`、`rehash_intent()` のみ。raw ID0/MAXも値として保持し、WP_User構築時のint cast/ID lookup条件やprincipal適格性を勝手に採用しない。RehashIntentはSuggested/NotSuggestedというdefault-profileのhintであり、hash生成・DB更新・cache/action・session失効を実行しない。metadataが解決できなければUnresolvedで止める。

opaque rowのcredential一致は、accountが認証を許可されることを意味しない。snapshot freshness、source identity、他のauth callbacks、role/capability、同時変更、CASConflict/CommitOutcomeUnknown後のlogin可否やrehash retryはpendingである。外側のCASがUpdatedとなってもAuthUser/session permissionへ自動変換しない。呼出元入力、S1/query Vec、is_email/metadata providerのallocation、generic R、crypto workはshared pool/RSS/全消去の保証外。host costはwall timeやbounded async CPU cancellationの契約ではない。

## 独立harnessとDB結合の区別

専用PHP oracleは実username/email callbacksと実 `WP_User::get_data_by` の前処理を使う一方、cacheは**record-only fixtureのalways-miss stub**、SQLはexact-keyのfake `$wpdb`、userはReflectionによるfake objectである。`update_user_caches` も明示no-opで、実cacheへ書き込まない。sourceのcache-miss経路をこのfixtureで通すことと、real WP cache-miss/SQL/cache更新との同等性は別である。native自身にWP cacheを接続せず、host-supplied raw lookupを受ける。exact-byte fake keyと実DB collationの等価性、実cache hit/miss/invalidationは主張しない。setterもintentだけを観測し、実credential更新・action副作用を行わない。native NoCredentialMatchとPHPのsource error codes/messagesは別の観測として扱う。

public6/private2契約は、username一致でemail0、不在/確定不一致だけS1 emailへ進むこと、Unsupported/cost/lookup/provider/future metadata errorのterminal停止、S2空/`0`のQuery保持、PHP-empty、ID0/MAX、same-row getter一回、raw長passwordのMD5順序、全面Debug秘匿を検査する。private seamはfuture error分類用であり、外部provider/plugin注入APIではない。担当8試験とWP全target Clippyは通過した。独立harnessはWordPress関数件数68へ新関数として加算せず、後続gateの成功をこの段階で先取りしない。

別担当の `credential_username_email_stage_pipeline` はowned fresh fixtureのID1001/1002と公開dummyだけを使い、既存56へ1項目を加えて57 checksとする。username rowの確定不一致→email row matchでlookup[1,1]、username一致/unsupported/costで[1,0]、empty/`0`で[0,0]を確認する。stage実行中のDB不変を照合し、外側controllerだけが型付き生成→email rowへのraw CAS→fresh検証を行い、username rowを維持する。最後に専用2行を削除し元users3/usermeta5を全行一致に戻す。これはfinite dev-only結合で、normal DB/serverへWP dependency・HTTP authentication・principal権限を追加しない。

今回もWP src/lib変更はDB dev-only pipelineが追跡する全関連証跡をstaleにする。親の全fresh fixture再測定と証跡checker確認が必要で、手動hash restampをしない。本担当はCargo・DB library/tests・全体checker・正式proofを変更せず、DBを起動しない。直前68 gateのRust490＋独立29/Python214/68 API453,064件/8DB各56成功は歴史記録であり、このstage実測と混同しない。

## 専用relation比較の対象実測

`scripts/compare_credential_stage.py` は現在のCargo compiler-artifact JSONから一意に選んだ `credential_stage_probe` と、固定PHP8.5.12 `-n` / 128MiBの `credential_stage_oracle.php` を所有childとして起動する。WordPress manifestでselected sourceのhashを、PHP tool manifestでbinary identityを照合してから実関数を呼び、childの実PIDと報告PID・exit0・stderr空を要求する。共通owned-probe registryのcleanup retryを成功artifact書込み前と失敗finallyで行う。JSONのduplicate/unknown fields、型混同、入力1MiB・出力128KiB等のprotocol boundを拒否する。出力boundはcapture後の検査であり、child全体のRSS上限とは主張しない。

正式WP関数corpusへ加算しない独立33 relation casesを比較した。25件は初期null・empty relevant filtersで実username/email callbackを各1回呼び、8件はnativeのlookup/hash/host-costのUnresolvedに対応する明示refusalで、PHP callbackは呼ばない。refusalをWordPressのfalseやincorrect_passwordとして扱わない。email側のrefusalはusernameの短いhashによる確定false後にも確認する。ProviderFailureやmetadata faultの誘発はnative private seam契約だけであり、実provider障害を観測したとはしない。

actual25件ではcache always-miss＋fake SQL exact-keyのsource経路を通し、実 `WP_User::get_data_by` が作るS2/trim6(S1)をnative lookup requestと照合する。`%&x;61@example.test` のlogin keyは `@example.test`、email keyは `%61@example.test`。S2空は実query、FF由来S1=SPACEはtrim後Skipped、`&x;0`由来S1=`0`と `&x;`由来S1空はlookup前停止として区別した。opaque Rのdummy tag/via・P1のNUL/不正UTF-8維持・4097-byte passwordのMD5優先・cost4/12/portableのmetadata hintも検査する。WP_UserのID101/202はfixture内tag対応だけで、製品ID/principal policyにしない。

nativeのNoCredentialMatchにはPHP_Errorのcode/message契約を加えない。PHP側のerror codesとusername error同一object保持を別fieldで照合する。invalid emailはusernameのinvalid_usernameを保持し、valid email不在はinvalid_emailへ置換する。username成功ではemail lookup/is_emailが0回、username確定false後はemail別rowで成功し得る。sourceのsanitize_user/is_email/wp_authenticate_user/check_password/algorithm/options/needs-rehash hook数は実 `did_filter` で測り、observer filterを登録せず検査する。

各caseではsource関係global7個（filter registry/counters/current stack、hasher、locale、locale package、actions）の未設定/Null/sentinel/emptyを使い分け、finallyで存在と値を復元する。WP_Errorに伴うactionsは不変と仮定せず**復元**を確認する。translation・error HTML・lost-password URLは明示fixture stubで、message/escaping全互換は対象外。実WP_User classをReflection without constructorで準備するため、constructorのroles/metadata/ID変換は通さない。setterはintentを記録するだけで、DB/cacheへのwriteを行わない。

対象33件のrelationとcurrent evidence checkerは成功した。new Python8契約はUnresolvedとfalseの分離、S1/S2/empty、source error/hook/hintの独立性、strict types・完全coverage・globals、PID/source/binary/UTC、重複・読取bound・cleanup retry、PHP9件/native7件のbad-protocol拒否と固定診断の秘匿を検査し、WP全target Clippy/fmtも通過した。出力 `compatibility/evidence/credential-stage-relations.json` は有限relation証跡であり、68 API / 453,064比較は変えない。後続の全体gate・全owned DB57再実測やproduction authenticationの成功を先取りしない。
