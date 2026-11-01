# 編集用投稿textのCAS transaction試作

現在地の照合（2026-10-30）：[実装と上位設計・重要順報告](implementation-design-reconciliation-2026-10-30.ja.md)を参照。以下のcheckpoint／件数／currentという記載は各観測時点の履歴であり、直近Cookie本体取り込み後の全体合格を示さない。現在は対象Rust21／Python12・fmt／Clippyまでで、正式比較・DB・依存台帳等の再検証は未実行。今回の文書更新で製品コード開発は再開していない。

後続実測（2026-10-29）：`target/wp-bcrypt-and-alpha-publication-full-check.log`で全8つの新規所有DB fixtureが各45契約を通過した。readonly拒否は単なる任意エラーではなくMySQL/MariaDB error番号1792を要求する。両backendのα経路も固定A/B入力から別sealed HTTPまで実測し、全fixtureを回収した。以下の初期compile-only記録に続く検証であり、製品のWP保存／公開policyの完成ではない。[公開経路の境界](alpha-publication-development.ja.md)。

2026-10-29。新しい `EditingWriter` は既存投稿の `post_title/post_content/post_excerpt` だけを更新するraw editing capabilityである。`EditingRepository` / `EditingSnapshot` のreadonly契約を変更せず、別private poolと明示 `connect/close` を持つ。公開serverへDB dependency、資格情報、writerを渡さない。接続にはhostが明示したoptionsを使い、既存readerと同じsession SQL-mode除外・REPEATABLE READを適用する。grants、global settings、schema、サービスや既存DBを変更するAPIではない。

APIは `compare_and_set_post_text(expected: &PostRow, replacement: &PostTextFields)`。transaction内で最新行を `SELECT ... FOR UPDATE` し、全23列をRustの完全一致で比較する。SQLのcollation equalityや秒精度のmodified日時、rows-affected/found-rowsをtokenにしない。不在ならNotFound、expected不一致ならConflict、3fieldも不変ならNoChangeとして、明示rollbackの成功後にだけ返す。UPDATEへ進む場合は3fieldとIDをbindし、全行を再読して指定3fieldと残り20列の完全一致を要求してからCOMMITする。成功ACK後だけUpdatedを返す。

`PostTextWriteError` はStorageMismatch、Database、CommitOutcomeUnknownの固定分類。SQL error本文や投稿本文・passwordを保持せず、Debug/Displayにも出さない。段階途中のSQL/decodeエラーや読戻し不一致はrollbackし、rollback失敗ならDatabaseとして、Conflictや元値保持が確定したとは表示しない。COMMITを試みた後のエラーは保守的にCommitOutcomeUnknownとし、盲目再送をしない。async cancelは返値を持たず、SQLxのDropはrollbackをscheduleするだけで完了ACKを表さない。特にCOMMIT中のcancelはfresh authorized read等でreconcileする必要がある。

既存editing接続はstrict SQL modesを除外する。TEXTへの長過ぎる入力がSQL成功＋切詰めとなる可能性があるため、成功したUPDATEだけで保存完了としない。全文のexact readbackでStorageMismatchを検出しrollbackする。[MySQL SQL mode](https://dev.mysql.com/doc/refman/8.4/en/sql-mode.html)を参照。locking readは最新行とrow lockを使う。[MySQL locking reads](https://dev.mysql.com/doc/refman/8.4/en/innodb-locking-reads.html)

IDはu64の既存rowだけを扱うため、0/MAXでもAUTO_INCREMENTやNO_AUTO_VALUE_ON_ZEROを変えない。4つの日付String（zero dateを含む）、status/type/password、author/parent、slug/guid、負のmenu_order/comment_count等は更新せず、そのまま照合する。全行比較はvalue-based CASで、ABAや同値delete/reinsert、monotonic base_revision、別tableのmeta変更、source identityは検出しない。SELECT後にengine情報を検査し、InnoDB以外はUPDATE前に拒否する。固定schema・trigger無しの所有fixtureが検証範囲であり、同時DDL/engine置換、未知triggerの副作用や任意legacy schemaへ一般化しない。

これは `wp_update_post` の実装ではない。固定WP `post.php:4598/5327` の日時生成、slug一意化、revision、保存hooks、meta/taxonomy連携等は後続のsource-derived契約となる。WP標準準拠の採否を再質問するものではない。一方、独自revision/base_revision、job/outbox、冪等receipt、認証/認可/CSRFとHTTP接続、公開generation切替は実装していない。DB層がstatus・日時・timezone・slugや公開policyを自動選択しない。入力、DB row/driver、allocator workはunpooledで、RSSやshared budgetの保証はない。

pure unitは3件。admissionで全行比較がNoChange判定より先行すること、case/accent/trailing-spaceが異なるexpectedを拒否すること、readbackで23全fieldを1つずつ改変すると拒否すること、rollback ACK失敗の格上げ、COMMIT失敗のunknown分類と診断の内容非露出を検査する。

所有fixture向けのCHECKSは既存39項目へ次の6項目を追加し45項目とする。

- `post_text_cas_exactness`: 3field更新・NoChange・NotFound、古いreadonly snapshotの保持とfresh read一致。
- `post_text_cas_conflict`: stale expectedを拒否し、DBがcase/accent/trailing-spaceを等しいと判断してもRustでは拒否。
- `post_text_cas_unsigned_zero_dates`: ID0/MAX、zero datesと残り20列保持。
- `post_text_cas_truncation_rollback`: 65536-byte TEXT titleの切詰めをStorageMismatchとして3fieldともrollback。
- `post_text_cas_concurrent_writers`: 同じexpectedの2 writerが1成功・1Conflictとなり、勝者の完全な行だけを保持。
- `post_text_cas_schema_rejection`: 別prefixの専用MyISAM cloneと存在しないtableを拒否し、元postsを維持。元wp_postsのengineは変えない。

fixture検査は3textを元に戻し、全原rowを照合する。拒否用tableもdropする。通常のreadonly拒否等39項目は維持する。この記録時点ではDB起動は行わず、compile・3 pure units・DB all-target Clippyを通過した。実DB検証は親harnessによる新規所有fixtureの後続段階であり、まだ成功としない。

## αの明示fixture選択

`RECASTPRESS_ALPHA_POST_PATH=disposable` が明示された場合だけ、45契約後に専用ID1001を作る。既存4投稿（private/customまたはpassword付き）や11 meta rowを公開入力へ選ばない。status=publish/type=post/password空はtest controllerが明示したfixture条件であり、製品のstatus/slug/date推定policyではない。CASでAを保存してfresh read、次にBを保存してfresh readし、その固定IDのtitle/contentだけを次の形へ渡す。

```json
{"a":{"format_version":1,"posts":[{"target":"/alpha-post","title":"readback A title","body_text":"readback A content"}]},"b":{"format_version":1,"posts":[{"target":"/alpha-post","title":"readback B title","body_text":"readback B content"}]}}
```

actual Rust test PIDもtop-levelのalpha_publication_process_pidへ記録し、親のactual Popen PIDと照合する。このobjectをDB_CONTRACT_RESULTの `alpha_publication_inputs` へ追加する。NUL無しのplain Stringには改行、Unicode、`<literal>&"'` を含め、rendererはliteral表示として扱う。password/author/metaのfieldをexportせず、既存rowの本文・title・password、author値やmeta canaryが輸出文字列に含まれないことを照合する。最後に専用rowをdeleteし、元posts/metaの完全一致を確認する。flag無しの通常fixtureは追加fieldを持たない。

親harnessはこの実DB返値を別server CLIのprepared bundle/receiptと別公開process A/Bへ渡す予定である。DB側が公開serverへ接続したり公開generationを変更するものではない。この時点でα実HTTP縦断・保存/公開製品導線やデプロイ完了を主張しない。
