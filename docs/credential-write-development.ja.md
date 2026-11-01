# 編集用raw credential pair CAS候補

後続実測：両backend初回54契約と、finite rehash結合を含む全8 fixture各55契約が成功した。全体ログは`target/password-rehash-and-credential-pipeline-full-check.log`、dev-only WP implementationのsourcebinding強化後の再実測は`target/credential-pipeline-complete-source-provenance-check.log`。全fixtureを回収し元users/usermeta一致を確認した。最終の通常DB/native/alphaを含む10 checkerが全exit0。DB証跡はWP Cargo＋全srcも追跡するため、今後その変更も証跡をstaleにする。normal runtimeのDB/serverはWPへ依存しない。認証済みuserの発行・session・cache/action・自動credential更新policyの完成ではない。

2026-10-29。固定 WP 7.1.3 `wp-includes/pluggable.php:3103` の `wp_set_password` にある `user_pass/user_activation_key` のSQL更新部分だけに対応する候補。新しい `EditingCredentialWriter` は別private poolと明示 `connect/close` を持つ。既存post-only `EditingWriter`、`EditingRepository` / READ ONLY `EditingSnapshot`、公開serverの資格・依存境界は変更しない。DB資格情報やSQL executorを公開するAPIは持たない。

元WP関数は `get_userdata`、`wp_hash_password`、2列UPDATE（activation keyは空文字）、`clean_user_cache`、`wp_set_password` actionという処理を行う。この候補はその関数全体を実装しない。`compare_and_set_user_credentials(expected: &UserRow, replacement: &UserCredentialFields)` はcallerが選んだ2つのraw byte列を受ける。activation keyを空にする場合もcallerが明示する。hash生成・algorithm選定・marker判定・rehash・認証・認可・session失効・cache清掃・hooks・actions・通知・公開generationを自動実行しない。production DB/serverのnormal dependency treeへWP/crypto依存を追加しない。後述の有限結合試験だけ、DBのdev-dependencyとしてWP path dependencyを使用する。

接続はhostが渡す `MySqlConnectOptions` と検証済み `TablePrefix` を使う。新poolの上限は既存editing poolと同じ4、session SQL-mode除外とREPEATABLE READ設定も同じだが、pool自体は共有しない。共通の既存SQL-mode定数だけを再利用し、旧reader/writerの接続実装は変更しない。新writerはstatement/slow-statement loggingを無効化する。接続失敗を含めてerrorを閉じた `UserCredentialWriteError` に変換し、driver本文・接続資格・credentialを返さない。新grants、global設定、schema、既存DBやserviceを変更するAPIではなく、Rustの別型がDB accountの権限を追加することもない。

## transactionの契約

1. 明示transactionで `ID = ? FOR UPDATE` により最新の全10列を取得する。返却textは既存 `UserRow` と同じbinary CAST、登録日時はCHARとしてzero dateを保持する。
2. table readの後、InnoDBであることを検査して、非transactional tableならUPDATE前に拒否する。固定schema・trigger無しを前提とする。
3. 最新行が不在ならNotFound。全10列のRust Eqがexpectedと異なればConflict。その後だけpair不変をNoChangeと判定する。SQL collationやrows-affectedをCAS tokenにしない。
4. `user_pass/user_activation_key` とIDだけをbindしてUPDATEし、全10列を同じtransactionで再取得する。指定2列のexact replacementと、残り8列のexact保持を要求する。
5. COMMIT ACK成功後だけUpdated。NoChange/Conflict/NotFoundや途中の拒否は明示ROLLBACK ACK成功後だけ確定して返す。

IDはu64の既存rowに限る。ID0/MAXでもINSERT/AUTO_INCREMENT操作は行わない。`user_registered` のzero date、login/nicename/email/URL/display name、負のuser_statusも変更・解釈せず照合する。user_statusによるban、login/email優先、sanitize/trim/case fold、PHP coercionは追加しない。入力がraw bytesでも、任意bytesが固定VARCHARへ必ず保存できるとは主張しない。

既存editing SQL-mode互換設定はstrict modesを除外するため、`varchar(255)` への長過ぎる値がSQL成功＋切詰めになる場合がある。UPDATEの成功やaffected rowsだけを保存完了としない。2列とも含むfull readbackでStorageMismatchを検出し、InnoDB transactionをrollbackする。[MySQL SQL mode](https://dev.mysql.com/doc/refman/8.4/en/sql-mode.html)、[locking reads](https://dev.mysql.com/doc/refman/8.4/en/innodb-locking-reads.html)

## outcome・失敗・限界

outcomeは `UserCredentialWriteOutcome::{Updated,NoChange,Conflict,NotFound}`。errorは `UserCredentialWriteError::{StorageMismatch,Database,CommitOutcomeUnknown}` の固定分類。replacementのDebugはfield値を表示せず、既存UserRowもID-only Debugを維持する。errorのDebug/Display/sourceにpassword、activation key、SQLx errorを保持しない。

rollback失敗時はDatabaseへ格上げし、Conflict・NoChange・元値保持が確定したとは返さない。COMMITを試みた後のerrorは保守的にCommitOutcomeUnknownとする。async cancelには返値がなく、SQLx Dropがrollbackをscheduleしても完了ACKではない。特にCOMMIT中のcancelやtransport failureでは、fresh authorized read等でreconcileし、盲目再送しない。

全行比較はvalue-based CASであり、ABA、同値delete/reinsert、monotonic revision、source identity、usermeta/role/capability変更を検知しない。固定schema・trigger無しのfixtureで検証し、同時DDL/engine置換、未知triggerが別tableや非transactional領域へ及ぼす副作用を完全rollbackする保証ではない。WPのcache/actionとSQLの原子性、外部通知・session・公開系との原子性も含まない。caller入力、raw UserRow、driver/allocator workはunpooledで、zeroize・peak/RSS・shared budgetの保証は追加しない。

## 契約と検証段階

pure private3契約は全10fieldを一つずつ変更したConflict、NoChangeより先のexpected検査、case/accent/trailing-spaceのRust区別、正しいpairと残り8列のreadback、ID0/MAX・zero date、rollback ACK failureとCOMMIT unknownの分類を確認する。public1契約はraw pairの保持とredacted Debug/closed errorの非露出を確認する。

別担当の所有fixture契約は元users/usermetaを全行一致で復旧し、専用拒否cloneも撤去する。既存48 checksへ次のraw CAS 6項目と、後述の有限結合1項目を追加して55とする。

| check | 検査範囲 |
| --- | --- |
| `credential_cas_exactness` | pair更新・NoChange/NotFound、readonly旧snapshotとfresh read、UPDATE拒否1792 |
| `credential_cas_conflict` | stale全行expected、非credential field、DB collationで等しいcase/accent/padとの差 |
| `credential_cas_unsigned_zero_dates` | ID0/MAX、zero dateと残り8列保持 |
| `credential_cas_truncation_rollback` | 両VARCHARの256-byte切詰めを拒否しpair全体をrollback |
| `credential_cas_concurrent_writers` | 同一expectedの2writerが1 Updated / 1 Conflictとなる |
| `credential_cas_schema_rejection` | MyISAM clone/missing table拒否、元usersを変更しない |

この文書時点では担当pure試験・compile段階であり、fresh DBの実測はrootの後続gateで行う。DBを起動せず、正式proofを生成・書換えしていない。直前のdispatcher gate（Rust464/Python195/64 API・437,198件、legacy113、直前8DB各48維持）と、この新writerのlive成功を混同しない。

## 専用dummyの有限rehash結合

追加check `credential_rehash_pipeline` は、明示的に所有するfixtureの専用user ID1001だけで、既存の独立した部品をtest controllerが結ぶ。公開dummy passwordと固定legacy MD5 hashを用意し、raw dispatcherによる検証、明示default-bcrypt profileのneeds-rehash=true、host maximum4での生成拒否と対象DB行不変、host maximum12でのOS-random生成を確認する。その生成結果をcallerがraw pairへ明示copyし、activation keyを空にしたCASを実行する。更新前のreadonly snapshotが旧行を保つこと、fresh readの新hashが正しいdummyにtrue・誤dummyにfalseとなりneeds-rehash=falseであること、旧expectedがreplacement一致時にもConflictとなることを確認し、最後に専用rowをdeleteして元users3行/usermeta5行を全行一致へ復旧する。

この結合を可能にする `recastpress-wp` path dependencyは `recastpress-db` のdev-dependenciesだけで、production library/serverのnormal graphや公開HTTPへは接続しない。manifest/lock/inventory/checkerの更新はroot担当であり、この文書担当は変更しない。random hashのbyte一致を期待値にせず、既存provider経路とfresh DB readbackによる相互整合を見る。実ユーザーsecret・既存accountの自動移行は行わない。

新writer自身にverify、needs-rehash判断、生成、automatic retryを組み込むものではない。どのaccount/passwordを扱うか、成功したverifyが保存を認可するか、rehashをいつ実施するか、並行更新時のユーザー体験、cache/action/session/通知・失効policyは採用しない。WPの `wp_set_password` 全体やlogin処理との互換、実サービスでの安全なpassword変更の完成も主張しない。結合契約のcompile・read-only reviewと、親のfresh DB実測成功は別段階として記録する。
