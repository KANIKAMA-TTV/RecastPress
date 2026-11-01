# prepared auth-cookie照合の開発状況

2026-10-30現在、main取り込み後のfocused確認まで完了し、ユーザー指定で中断している。状態記録は `target/auth-cookie-verification-pause.json`。main後のactual PHP関係比較・全体gateは未実行であり、認証機能の受入完了ではない。本書は既決の[認証・SSO方針](authentication.ja.md)を変更しない。

## 境界とAPI

`recastpress-wp` の既存auth feature配下に `auth_cookie_verify` を追加した。`wp_validate_auth_cookie_prepared_get_non_ajax_resolved_salt_bytes` はexplicit cookie／scheme、callerが準備した既存user、完全なsession集合、解決済み非空salt、明示的なsigned-i64 now、必須limitを受ける。GET・非Ajaxの限定関係であり、HTTP要求を直接受理するAPIではない。

| 型 | 保持する契約 |
| --- | --- |
| `PreparedCookieVerificationUser` | positive i64 IDと借用login／FULL password hash。実在・検索結果との対応はcallerの責任 |
| `PreparedCookieSession`／`CookieSessionExpiration` | lowerhex64 verifierとtyped expiration。Legacy整数もRecordと同じ期限として扱い、0や負値をPHP falseへ変換しない |
| `PreparedCookieSessions` | positive owner ID、完全な集合、全entryの形・一意性・必須件数／key byte cap。未選択の不正entryも拒否 |
| `CookieVerificationLimits` | `new(max_input_bytes)` 必須、Default無し |
| `CookieVerificationOutcome` | `NoMatch(stage)` または同じprepared userを借用する `Match(UnfinalizedCookieMatch)` |

`prepared_record_id()` は準備済みrecordのraw ID witnessにすぎず、principalや操作許可ではない。Matchに任意recordを差し替える公開constructorはない。borrowを保持しても後のlogout・session更新・権限変更の再確認にはならない。

## admissionとsource分岐

固定WPの `wp_validate_auth_cookie`（pluggable.php:782）、`wp_parse_auth_cookie`（1013）、`wp_hash`（2719）、session baseのtoken hash／期限判定とdefault session managerを根拠にする。[生成76](auth-cookie-development.ja.md)と[raw parser77](auth-cookie-parse-development.ja.md)を利用するが、WP全入力・実bootstrapの同等性は主張しない。

complete集合の全entry検査・owner対応・cap・型や非canonical expiryの拒否は、限定profileへのadmission失敗であり、WPが返すfalseの段階ではない。cookie全体が空またはASCII `0`、salt空、userとsession owner不一致も拒否する。expiryはsigned-i64をdecimalへ戻したbytesと完全一致する場合だけ対応し、`+1`、`01`、`-0`、空、overflow等をPHPの暗黙変換へ委ねない。

admission後の順序は、3個のpipeによる分解失敗→`Malformed`、cookie expiry `< now`→`Expired`、準備user無し→`UnknownUser`、MAC不一致→`BadMac`、session不在または期限切れ→`BadSession`、それ以外→未確定Match。cookie期限とsession期限はnowと等しければ通る。POST／Ajaxの1時間grace、内蔵時計・既定nowは導入しない。

MACにはcookie内のraw usernameとraw expiryを使う。prepared rowのloginで置き換えず、lookup collation由来の対応付けをこの部品で再定義しない。password fragmentとHMAC-MD5→lowerhex ASCII32 key→HMAC-SHA256のbuilderを既存生成とprivate共有し、標準 `Mac::verify_slice` で照合する。tagはlowerhex64だけに対応し、uppercase／非hex／長さ違いはBadMacへ戻す。session verifierはSHA256(token)のlowerhex64。検証tokenは空／ASCII `0`も扱い、session生成へ進む生成側のfalsey-token拒否を流用しない。

入力capはcookie＋scheme＋salt＋prepared login＋FULL hash＋全verifier keyのraw byte長をchecked加算する。typed整数等は固定サイズなので含めない。件数・key長capは集合constructorでも必須。callerの借用storage、RSS、allocator容量、CPU、同時要求数のquotaではない。errors／prepared view／matchのDebugは内容を出さず、scratch消去はbest effort。caller入力・provider内部・compiler temporaryの完全消去や関数全体のconstant-timeは保証しない。

## 検証済み範囲と中断位置

| 段階 | 実観測 |
| --- | --- |
| 先行target-only | actual PHP30関係、PHPの前後時刻に対応するnative60評価、閉じたCLI拒否5件。`target/prepared-auth-cookie-target-checkpoint.json` |
| main focused Rust | 21件成功。matcher6＋既存生成private1、生成public6＋parser6＋probe protocol2 |
| main focused Python | 関係契約6＋evidence契約6の12件成功。main actual30比較の実行成功とは別 |
| main静的確認 | fmt／WP all-target Clippy `-D warnings` exit0。環境のincremental hardlink copy fallback警告があり、出力全体が警告ゼロとはしない |
| main actual／全体 | 未実行。中断時の所有child無し、委譲作業停止済み |

focused logは `target/auth-cookie-verify-main-lib.log`、`target/auth-cookie-verify-main-contracts.log`、`target/auth-cookie-verify-main-clippy.log`、`target/auth-cookie-relation-contracts.log`、`target/auth-cookie-evidence-contracts.log`。target-onlyの成功をmainの成功へ読み替えない。

actual PHP関係はbuiltin `time()` を置換しない。実行前後clockの短いbracketと、cookieおよび全session期限の両端で判定が変わらない条件を確認し、nativeを両端で評価する方式。時計の跳躍を排除する証明ではなく、equality±1やi64極値はnative境界契約で別に検査する。builtin HMAC／timeの呼出し数を架空に計測しない。正式primitive registryへのAPI追加はなく、専用関係比較として扱う。

既存exact serdeのderiveをdev-dependencyへ追加した。root lockはWPの既存serde edgeだけが差分で、registry tupleのversion／source／checksum変更や新provider追加は無い（`target/auth-cookie-verify-adoption-audit/lock-delta.json`）。これは新しいpure graph・inventory・license・全体検証の実観測に代わらない。

旧正式79 API／519,014件・hooks572／dispatcher405やcrypto／DB／pure／依存観測は完了当時の履歴として保持する。main source／Cargo変更後のcurrent成功とは扱わない。再開時には実metadata由来のinventory／license更新、正式79の入力・件数・両結果保持、専用auth-cookie比較を含むcrypto4関係、pure／default、全8 fresh DB、HTTP／cross compile等の統合確認、最終12checkerと所有process／DB回収を行う予定。現時点では実行しない。再開順はpause記録に固定し、旧hashの手動restampを行わない。

## 上位設計との整合と残件

実装が認証・SSOの既決方針を変更した衝突は確認していない。限定GET profileをWP全標準ログインの代替として採用したものではなく、未対応範囲を残した基礎部品である。

優先順の残件は、第一にmain後の証跡再生成という検証残である。第二にprincipal発行・HTTP認証・CSRF・role／capability・session再照合など製品への接続が未実装。管理ready503は維持する。第三に、秘密／saltの保存・更新、session永続化と失効・期限、OIDC連携／回復／一度限り許可の消費や期限など、上位文書の未決事項を契約化する必要がある。これらを本helperの引数やMatchから推測決定しない。

SSO全ユーザー向け・初期off・3モード・OIDCのみ・MFA非実装・必須時のpassword許可はOIDC連携専用という既決事項は維持する。DB user lookup、session作成、RNG、salt resolver、Set-Cookie、実user／実cookie、認証済みprincipal、HTTP認可、Linux native実行の成功は今回の範囲に含めない。
