# 編集側の user login / email SQL lookup 候補

2026-10-29。固定 WordPress 7.1.3 `wp-includes/class-wp-user.php:202` の `WP_User::get_data_by` から、**SQL の検索部分だけ**を切り出した開発候補。`EditingSnapshot::user_by_login_text_query(&str)` と `user_by_email_text_query(&str)` は `Result<Option<UserRow>, UserLookupError>` を返す。通常のユーザー検索 API や認証の完成ではなく、caller が選んだ UTF-8 text を編集側 DB へ渡す限定能力である。

## 固定ソースとの対応

元関数は field の分類、PHP coercion、trim、false 相当の判定、login 時の `sanitize_user`、cache lookup を経て、`SELECT * FROM users WHERE field = %s LIMIT 1` を実行し cache を更新する。`get_user_by` はさらに `WP_User` を生成する。本候補はこの前後処理を実装しない。`&str` を受ける型付き SQL subset であり、raw bytes、PHP の型変換や truthiness の再現を主張しない。

SQL の WHERE は `user_login = ?` または `user_email = ?` で、key は SQLx の **text bind** にする。BLOB/byte bind、WHERE 内の binary CAST、Rust 側の byte equality・case folding・Unicode normalization は使用しない。値の等価性は固定 DB column collation が決める。projection の CAST AS BINARY は既存 `UserRow` と同じ保存 bytes を返すためだけに用い、検索条件を binary 比較に変更しない。

`ORDER BY` を追加しない。基準 schema の user_login と user_email index はいずれも UNIQUE ではない。同一 collation 上で複数行が一致すれば `LIMIT 1` の任意の一行であり、最小 ID、特定の一行、重複 email の一意性を保証しない。login と email のどちらを優先するかも、この DB 層では決めない。

元関数には検索 key 自体の文字数・byte 数上限はない。保存列の `varchar(60)` / `varchar(100)` を検索入力の上限へ転用せず、独自の製品上限も追加しない。入力確保、driver/server packet limit、timeout、全 RSS をこの候補で制限する契約ではない。

## 接続・schema・エラー境界

既存 `EditingRepository` の private pool から作られた REPEATABLE READ / READ ONLY snapshot 内でのみ実行する。lookup 前に users table の InnoDB、接続の client/connection/results charset が utf8mb4、connection collation が utf8mb4_unicode_ci、検索二列が固定 varchar 幅と utf8mb4_unicode_ci であることを確認する。条件を満たさない profile は `ProfileMismatch`。これは他の charset/collation、schema 変更、同時 DDL への一般対応ではない。安定した固定 schema を前提にし、metadata 確認と検索の間に生じる外部 DDL を原子的に防ぐ保証も追加しない。

返値は既存 `users_page` と同じ `UserRow`。password、activation key、email、user_status 等の raw 値を保存するが、Debug は ID だけを表示する。既存 page reader の SQL・返値・error は変更しない。`UserLookupError` は `ProfileMismatch` / `QueryFailure` / `RowDecodeFailure` の閉じた分類で、key、row、SQLx driver error、error source を保持しない。`None` は検索不在であり、password 不一致、無効 account、権限無し、lookup failure と同一視しない。

空文字列、`"0"`、NUL、先頭・末尾空白も SQL 能力としてそのまま渡す。基準 WP の login chain はその前に sanitize/trim/PHP empty を適用するため、本能力を直接 login route へ接続して同等性を主張しない。`user_status != 0` の ban、email validation、username/email fallback、password verify、rehash、credential 更新、role/capability、session/cookie/nonce は含まれない。新 writer、公開系依存、HTTP route は追加しない。

## 契約と検証状況

通常の native 契約は SQL の列が閉じた選択で key が parameter に入ること、WHERE binary CAST / ORDER BY がないこと、閉じた error、既存 ID-only Debug を確認する。専用 DB 契約は既存の所有 fixture 内に一時 ID 1001–1009 と profile 検査用 clone table を作り、最後に削除する。元 users / usermeta は前後で全行一致を照合する。

追加する DB checks は次の三つで、既存 45 項目と合わせて **48 項目**となる。

| check | 実測する候補 |
| --- | --- |
| `user_lookup_text_collation` | 固定 collation の case/accent/trailing-space 等価、Unicode、NUL、空/0、長い不在 key、SQL injection 文字列の bind、重複 email の非一意性、同じ key の login/email 別行 |
| `user_lookup_snapshot` | 外部更新前の snapshot の旧 login/password 保持、fresh snapshot の新値、READ ONLY 更新拒否 1792、元 users/usermeta 完全復旧 |
| `user_lookup_profile_rejection` | latin1 connection、MyISAM clone、binary-collation clone、検索列幅違いの拒否、検索以外の列が欠けた clone の closed QueryFailure |

通常 native 契約の実行と ignored DB 契約の compile を確認した。root による MariaDB 11.8.9 の最初の fresh 実測は、保存 `nul\0@lookup.invalid` に対する NUL 無し key `nul@lookup.invalid` を不在と推定した契約で失敗した（`target/user-lookup-mariadb-first-live.log`）。同じ固定 utf8mb4_unicode_ci の検索では、この NUL 差でも一致する実結果だった。返す保存 bytes の保持と、WHERE の collation 等価性を混同した契約側の誤りであり、lookup SQL を binary 比較へ変更しない。

修正契約では NUL 有り／無し両 key を、control connection 上の独立した text-bound `SELECT ID, CAST(user_email AS BINARY) ... WHERE user_email=? LIMIT 1` と照合する。期待 ID 1003 と保存 NUL bytes が保たれることを確認し、lookup 返値もその ID/bytes と一致させる。この観測を他の charset/collation 全域の規則へ一般化しない。

NUL 契約修正後、root の MariaDB 11.8.9 実測では48 DB項目と binlog probe が成功した。続く MySQL 8.4.13 の初回は、metadata の型 decode が `QueryFailure` となった（`target/user-lookup-mysql-first-live.log`）。fixture 限定の型分類で `CHARACTER_MAXIMUM_LENGTH` が二列とも signed BIGINT、`Option<u64>` decode 不適合、`Option<i64>` decode 適合、実値60/100は正しいことを確認した（`target/user-lookup-mysql-metadata-diagnostic.log`）。driver/user/hash の文字列を出す診断は使用していない。

profile metadata SELECT の長さだけを `CAST(CHARACTER_MAXIMUM_LENGTH AS UNSIGNED)` に変更し、SQLx が期待する unsigned integer wire type に統一した。固定列の非負の幅60/100と NULL は保持する。検索 key の上限、検索 WHERE の text bind、collation 等価性は変更しない。確認用の一時診断コードは除去した。[MySQL 8.4 CAST 仕様](https://dev.mysql.com/doc/refman/8.4/en/cast-functions.html)も参照。

後続の`target/legacy-password-and-user-lookup-full-check.log`で固定 MySQL 8.4.13 / MariaDB 11.8.9 を含む全8 fresh fixtureが各48項目を通過した。current source hashを実測で更新・照合し全fixtureを回収した。workspace Rust458＋独立29、Python194、Clippy、server/WP Linux check、実HTTPも成功。これはnative Linux実行や認証接続の実証ではない。古いREAD ONLY snapshotが旧passwordを保持する契約は、認証時のfreshnessや変更後の失効期限を保証しない。失敗の診断logはtargetへ保存し、rootが各所有PID終了と専用絶対pathを確認して失敗dirを回収した。
