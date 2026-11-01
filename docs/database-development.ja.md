# 編集用DBリーダーの試作と検証

現在地の照合（2026-10-30）：[実装と上位設計・重要順報告](implementation-design-reconciliation-2026-10-30.ja.md)を参照。以下のcheckpoint／件数／currentという記載は各観測時点の履歴であり、直近Cookie本体取り込み後の全体合格を示さない。現在は対象Rust21／Python12・fmt／Clippyまでで、正式比較・DB・依存台帳等の再検証は未実行。今回の文書更新で製品コード開発は再開していない。

最新post-name pipeline checkpoint（2026-10-29）：Query前処理→明示UTF8 text lookupのdev-only関係を追加して61契約。`target/truncate-slug-and-post-name-pipeline-full-check.log`で全8 fresh fixture各61、Rust536＋独立29・Python254（1skip）・正式PHP72 API482,951・Clippy・Linux cross-check・実HTTPが成功。最終11checker current（`target/truncate-slug-current-check-summary.json`）、全fixture回収済。meta-view前の元4posts/6metaを保持して実測し、casevariant・fallback・UTF8境界をSQL結果と結合した。公開／URL解決／slug一意性の判定は追加しない。[有限関係の範囲](post-name-preprocessing-development.ja.md)。

最新post-name checkpoint（2026-10-29）：editing-only `posts_by_name_text_page`を追加し、両text keyのSQL比較・cursor/snapshot・固定schema拒否の3項目を加えて60契約とした。`target/sanitize-title-and-post-name-retry-full-check.log`で全8 fresh fixture各60、Rust529＋独立29・Python248（1skip）・Clippy・Linux cross-check・実HTTP・正式PHP71 API466,887が成功。最終11checker current、`target/sanitize-title-current-check-summary.json`。初回試験はmeta-view追加前6行を後段11行と誤認したassertで停止、call順を確認して6へ訂正し、完全な原状復旧比較を保って再測定した。全fixture回収済。新readerは公開／URL／status選別を行わず、raw23field storage契約を返す。[検索の限定profile](post-name-lookup-development.ja.md)。

最新phase3全gate（2026-10-27、日本時間）：target/shared-buffer-and-pretty-json-full-check.logが成功。workspace Rust251件（通常ignore1）＋独立experiment19件・Python86件、固定PHP比較45 API・304,071ケース（JSON17,073・9flags全512組合せ）／hooks572／dispatcher405、通常MariaDB／MySQLと専用GTID MySQL各39項目、HTTP・Clippy・Linux向けcross-check・license208観測を通過し全3 fixtureを回収した。両nativeのpostmeta_shared_pool_commits=3／postmeta_shared_pool_refusal_preserved=true／postmeta_shared_pool_released=trueをstrict検査。postmetaだけ一poolに行・索引・raw保持を接続し、他ProbeBufferは専用poolである。decoder／input／比較コピー／position／outer Vec等、全RSS・cross-process・自動算式・cacheは未対応。[共有ChangeBuffer開発記録](budgeted-change-buffer-development.ja.md)。

直前phase2の履歴（2026-10-27、日本時間）：target/budgeted-meta-view-and-loader-cache-final-check.logが成功。workspace Rust238件（通常ignore1）＋独立experiment19件・Python86件、固定PHP再比較45 API・299,957ケース／hooks572／dispatcher405、通常MariaDB／MySQLと専用GTID MySQL各39項目、HTTP・Clippy・Linux向けcross-check・license208 package観測、cargo graph155／188を確認し、全3 fixtureを回収した。BudgetedPostMetaViewは行／索引の論理予約と寿命だけを結ぶ候補で、全RSS・node／capacity／driver／ref map／peak・cross-process・自動算式・CLI cacheは未対応。初回gateの古いschema helper記録による終盤失敗、DDL同一の再生成と起動前guard修正は[開発記録](budgeted-meta-view-development.ja.md)に残す。MySQL overlap prefix134は今回の追加2 write由来で、旧132の履歴を置き換えない。

過去の検証履歴（2026-10-27、日本時間）は2 gateに分けて記録する。target/limited-index-native-check.logで独立experiment19件・Clippy、通常MariaDB／MySQL各36項目と専用GTID MySQL36項目、cold restore／pre-COMMIT kill／同DB選別／上限付き変更と索引各3 COMMIT／overlapが成功し、全3 fixtureを回収した。続くtarget/limited-index-and-license-check.logでworkspace Rust219件・Python77件、Clippy・Linux向けcross-check・HTTP、208 packageのlicense file観測、DB／binlog／overlap／WP証跡のcurrent hash確認が成功。WP44 API・286,998ケースは維持しPHP再実行なし。license観測は203 archiveのlock hash一致・364 fileのcache一致、checksum JSON missing 203・候補なし4・local5で、法的判断の完了ではない。論理bytesは最終candidate／indexだけでRSS・peak・shared budgetを保証しない。

以下の214件の全gateは直前の履歴。

直前全gateの履歴（2026-10-27、日本時間）：`target/limited-meta-changes-full-check.log`でworkspace Rust214件＋独立experiment19件・Python68件、通常MariaDB／MySQL各36項目と専用GTID MySQL36項目、cold restore・pre-COMMIT kill・同DB選別・上限0拒否と3 COMMIT・overlap、HTTP・Clippy・Linux向けcross-check・registry current hash照合が成功。両DBのpostmeta_logical_limit_preserved=true／postmeta_limited_commits=3を再確認し、全3 fixtureは停止・回収済み。WP44 API・286,998ケース／フック572／dispatcher405はcurrent hash検査で比較証跡を維持し、今回はPHP再実行なし。最終candidateの論理上限であり、RSS／peak／shared budget・製品容量算式の保証ではない。

直前DB gateの履歴：`target/mysql-overlap-bootstrap-full-check.log`でworkspace Rust194件＋独立experiment19件、Python68件、通常MariaDB／MySQL各36項目と専用GTID MySQL36項目、cold restore・pre-COMMIT kill・同DB選別反映、HTTP・Clippy・Linux向けcross-checkとregistry current hash照合が成功。WP41 API・274,683ケース／フック572／dispatcher405は既存比較証跡を維持する。全3 fixtureは停止・回収済み。MySQL overlapping bootstrapは独立候補として132 prefix／3 bootstrap／1 strict／12行のraw・索引一致を確認し、製品bootstrap完成や非prefix Aの実DB実証とは区別する。[専用開発記録](mysql-overlap-bootstrap-development.ja.md)を参照。

直前に成功した全体gateの履歴：R58～R61に向けた編集用リーダーと独立fixture実験。最新target/filtered-postmeta-and-cgroup-full-check.logでworkspace Rust190件＋独立experiment13件・Python61件、両DB各36項目、cold restore・pre-COMMIT client killと同一DB内選別反映、HTTP・Clippy・Linux向けcross-checkが成功。WP39 API・266,493ケース／フック572／dispatcher405は以前のform gateのWP段階で比較済みで、最新runはWP無変更のためPHP省略・current hash照合。所有fixtureは回収済みでdb-runsは空。旧失敗fixture2件の停止確認・ログ保存・回収と各歴史checkpointを保持する。製品のDB対応版・配置・プラグイン権限は新たに決定しない。

## 実装範囲

`recastpress-db`はSQLx 0.9.0で編集用DBのposts・postmeta・options・terms・term_taxonomy・term_relationships・termmeta・comments・commentmeta・users・usermeta・linksを読む。公開HTTPサーバーへはリンクしていない。PHP実行、公開切替、認証・認可、プラグインの直接SQL、スキーマ作成・更新は提供しない。

- 投稿は全23列を保持し、独自status・typeを既知enumへ潰さない。符号なしID、負のmenu_order、ゼロ日付を保持する。
- meta・optionはPHPのシリアライズを解釈せずバイト列として取得。NULLと空値、重複meta、孤立metaを区別する。autoloadも元の文字列を保持。
- 投稿・meta・optionのIDカーソル走査を用意。投稿ごとのmeta取得も可能。
- テーブル接頭辞は固定WPの`wpdb::set_prefix`に合わせ、ASCII英数字とunderscoreのみを許可。識別子のsuffixは内部enum、検索値とlimitはbind parameterを使う。
- 接続ごとに固定WPの既定の非互換SQL mode除外を適用し、REPEATABLE READを設定。プラグインによるmodeフィルターはまだ接続していない。
- 読取り専用・consistent snapshotで複数ページを走査。単一サイト用12表の存在とInnoDBを検査する。元表の同時DMLは試験したが、同時DDLやエンジン置換は対象外。
- raw rowのDebug出力は内容・パスワード・meta／option値を出さない。

明示した raw option／meta のみを上限付き WP decoder へ渡す [test-only 関係](bounded-unserialize-development.ja.md)を追加し、全 8 fresh fixture 各 62 契約を実測した。通常 reader は引き続き raw bytes を返し、WP は既存 dev-dependency に限る。過去の 61 契約 checkpoint は当時の履歴として保持する。

InnoDB以外の表は、この一貫スナップショットAPIでは明示的に拒否する。MyISAMを含む既存サイトの移行方法を却下した製品判断ではない。書込停止・ロック等を含む手順はD12の残件として扱う。

SQLx 0.9.0の要求に合わせ、開発ワークスペースのrust-versionを1.94へ更新。実測環境はrustc/cargo 1.96.0。正式SDK・配布版のコンパイラー契約はD04で引き続き具体化する。[SQLxの版情報](https://docs.rs/crate/sqlx/0.9.0)

## 固定参照DDL

```powershell
python scripts/export_wordpress_schema.py
```

固定`wp-admin/includes/schema.php`から`wp_get_db_schema`関数だけを抽出し、単一サイト・blog ID 1・接頭辞wp_・utf8mb4_unicode_ciの明示的な試験入力で12表のDDLを生成する。DB接続は行わない。生成物は`compatibility/schema/`、参照元・条件・ハッシュは`reference.json`。

これは試験入力用のDDLで、本体のDBマイグレーションではない。元のWP版を変更せず、既存DBへ自動適用しない。

## 専用の実DB試験

```powershell
python scripts/setup_mariadb.py
python scripts/setup_mysql.py
python scripts/database_smoke.py --backend mariadb
python scripts/database_smoke.py --backend mysql
```

Windows開発用のポータブルMariaDB 11.8.9とMySQL 8.4.13を`.reference/`へ保存する。Windowsサービス、システムPATH、既存DBは変更しない。MariaDB ZIPは公式SHA-256を固定。MySQL ZIPは公式公開鍵指紋`BCA43417C3B485DD128EC6D4B7B3B788A8D3785C`と署名を、Git for Windowsの既存GnuPGと専用keyringで照合した。[MySQLの署名検証説明](https://dev.mysql.com/doc/refman/8.4/en/checking-gpg-signature.html)

試験は新規の一時データ領域とloopbackの空きポートだけを使用する。VERSION、実データ領域、ポートが今回のインスタンスと一致することを確認してから、参照DDLと架空データを投入する。成功後は起動したプロセスを終了し、workspace内の専用実行フォルダーであることを検査して削除する。失敗時は停止後にログ・データを保持。`--keep`でも保持できる。

Windows MySQLの監視用forkを`--no-monitor`の起動引数で無効化し、PIDファイルと起動したプロセスのPIDも照合する。通常停止に失敗した場合は、所有する親が生きている間にそのプロセスツリーを終了する。`--backend mysql --exercise-cleanup`で、データ投入前の強制中断と一時プロセス／ファイルの回収を実試験した。初期の失敗試験の残存子プロセスは、実行ファイルと専用実行パスを照合して停止。ログはGit対象外の`target/development-logs/`へ保存した。

MySQLは試験DBで生成したCAを検証するTLS、MariaDBはloopback限定の試験用平文接続。製品のTLS設定・資格情報配布やLinux配備の合格を示すものではない。MySQLは127.0.0.1限定の架空アカウントへ試験DBの権限と停止権限だけを与える。本番プラグインの権限方針への流用はしない。

両エンジンで34項目を確認：Unicode・NUL、uint64境界、private／独自status、ゼロ日付、NULL／空／重複meta、孤立meta、シリアライズ済みバイト列、autoload、カーソル走査、同時更新中のスナップショット、書込み拒否、非トランザクション表拒否、欠落表拒否、binlog設定の読取り、先頭ページのゼロID保持、分類4表のraw値・複合主キーカーソル・同時更新中のスナップショット・各表の構造拒否、既存コメント・アカウント・legacyリンクのraw値・ページ送りとスナップショット・構造拒否、投稿メタ索引のbinary一致・走査復帰・snapshot分離・準備した変更のSQL結果一致。証跡は`compatibility/evidence/database-mariadb.json`と`database-mysql.json`。

通常の`cargo test`では実DB試験をignoreし、このハーネスだけが所有確認済みの一時DBへ明示的に実行する。実サイトを指す汎用DATABASE_URLを受け取る形にはしていない。

```powershell
python scripts/check.py --with-reference --with-process-smoke --with-databases
```

全体の契約試験・WP比較・実DB試験・HTTPスモークをまとめて実行できる。`python scripts/check_database_evidence.py`はDBへ接続せず、記録された試験と現在のDDL・コードのハッシュが一致することを確認する。

## 残件

正式な対応版、Linux・三配備先、非InnoDBの移行、マルチサイト／独自表のreader、legacy charset／collation、カスタム物理配置、同時DDL、性能目標、認証済み変更API、公開用データの生成・保存、変更監視は未完了。

## R59～R61への追従

内部自動索引とbinlog変更追跡の採否は承認済み。R60の配備検査の入口として、binlog_configurationでGLOBAL.log_bin・binlog_format・binlog_row_imageを読み取る。設定変更・トリガー設置は行わず、未知の値も文字列で保持する。グローバル既定値だけで追跡可能・権限確認済み・最新と判定しない。session差、フィルター、保持、権限、サーバー識別、ログ位置とスナップショット、実購読・COMMIT反映・索引・復帰は未実装。

観測値の意味は[MySQL](https://dev.mysql.com/doc/refman/8.4/en/replication-options-binary-log.html)・[MariaDB](https://mariadb.com/docs/server/ha-and-performance/standard-replication/replication-and-binary-log-system-variables)の仕様を参照。ライブラリ・形式・対応版・FULL等の要件をこの検査だけで製品の確定条件へ追加しない。

## COMMIT単位の保留処理

R60～R61に向け、`recastpress_db::change_buffer`を追加。信頼できる将来のログデコーダーから渡されたイベントをトランザクション単位で保留し、COMMIT前には反映用バッチを渡さない。永続データと処理済み位置を同時に反映するホスト側の成功後だけ、内部checkpointを進める。失敗時は同じバッチを保持して再試行し、後続イベントへ進まない。ホスト側が部分反映して失敗する動作を、この部品が取り消せるわけではない。

ROLLBACKは行変更を破棄し位置だけ反映する。位置の不連続・不正な入れ子・DDL・切断・予算超過は保留内容を破棄し、最後に反映成功した位置を維持する。復旧が必要な状態は予算を戻すだけでは解除しない。切断後にログが残れば永続位置からの再開、消失時は新しいsnapshotという選択は将来の復旧担当が検証する。重複・逆順・異なるsource位置を黙って適用せず停止する方針で、永続的な重複排除ストアは未実装。

位置型はsource識別・epoch・ログ座標を含める必要があり、MySQL/MariaDBの位置を文字列順で比較しない。デコーダー自身が前進・rotation・checksum・イベント分類を検証する。GTID等の開始情報を、独立して再開できるidle状態の位置更新と誤分類してはならない。行データは不透明な編集用バイト列で、公開用データ・公開ハンドルへ渡さない。

内部アロケーターからの行ペイロード予算を受け、Vecの保持capacityと行管理分を計上する。縮小時に収まらなければ一部の行を落とさずトランザクション全体を中止する。これはプロセス全体の容量管理ではなく、位置・デコーダー・索引・公開キャッシュとの共有予算は残件。

7件の状態遷移試験で、COMMIT前の非露出、反映失敗・再試行・二重適用防止、ROLLBACK、ログ欠落、source不一致、DDL、切断、入れ子、保持capacityと累積／縮小予算を確認。これは手書きの抽象イベントの試験であり、実binlogとの一致や両DBでのCDC合格を示さない。実購読、snapshotとログ位置の対応、永続ストア、索引、公開世代切替、キャッシュ全パージへの接続は未実装。

購読ライブラリ候補として[mysql_async](https://github.com/blackbeam/mysql_async)を調査した。binlogとRustls機能、MIT／Apache-2.0の選択許諾があるが、まだ依存へ追加していない。両DBの実イベント、TLS、ログ形式差と推移依存の確認後に実装へ接続する。

## 実binlogの能力試験

`experiments/binlog-probe`を本体workspaceと分離して追加。mysql_async 0.37.1／mysql_common 0.37.3を独立したCargo.lockへ固定した。本体／公開HTTPの依存は変更していない。MIT許諾を選ぶ想定で、推移依存全体のライセンス監査と製品への正式採用は未完了。

```powershell
cargo fetch --manifest-path experiments/binlog-probe/Cargo.toml
python scripts/check.py --with-reference --with-process-smoke --with-binlog-probes
python scripts/check_binlog_evidence.py
```

両DBの専用fixtureでのみROW／FULLのbinlogと別server-idを設定し、通常のraw reader試験28項目の後、架空表へINSERT・UPDATE・DELETEとROLLBACKを実行して有限のログを読む。既存DB、WP表の最適化変更、システムサービスへの設定変更ではない。

MySQL 8.4.13とMariaDB 11.8.9で、各17イベント・3行変更・3 COMMIT、操作別各1件、元値／変更値の一致、NULと非UTF-8の保持、ROLLBACK行の不在、16件のCRC32照合を確認。今回のMySQL行イベントはv2（30／31／32）、MariaDBはv1（23／24／25）だった。未知イベントは無害な進行として読み飛ばさず失敗にする。初回TLS試験は生成済みMySQL証明書の名前不一致で失敗し、専用CAでloopback SAN証明書を発行して再試験した。現在はCAとホスト名の検証を共に有効にして成功。失敗fixtureは停止確認後、ログだけtargetへ保存してデータを削除した。

証跡は`compatibility/evidence/binlog-probe-mysql.json`と`binlog-probe-mariadb.json`。プローブと独立lockfile・fixtureハーネスのハッシュを記録し、コード変更後の古い結果を検出する。本文・資格情報を証跡へ出さない。

これは有限の架空データに対する能力試験。連続購読、全型・uint64境界、GTID・rotation・再接続・ログ消失、初期snapshotと位置、永続データと位置のatomic反映、ChangeBufferへの接続、索引構築・公開世代／キャッシュ失効は未実装または未検証。FULLの試験条件を製品の必須設定に確定したものではない。ドライバー内部のpacket読取りはプローブのイベント長検査より先に割当するため、この検査で受信時の総メモリ上限を保証しない。

## スナップショット先頭とゼロID

posts_page・post_meta_page・options_pageにOption<u64>のカーソルを用意した。Noneは先頭（保存されたID=0も含む）、Some(id)は厳密にそのIDより後から読む。既存の*_after(id)はSome(id)の別名として残し、after=0を先頭の代用にしない。実DBでNO_AUTO_VALUE_ON_ZEROを使って3表へ0を格納し、先頭からは0を保持、継続からは0を再送しないことを確認した。u64::MAXの境界と既存のカーソル動作も維持する。raw exportの完全性のための変更で、WordPress APIにおけるID=0の解釈を変更するものではない。

binlog試作では、実際に取得した行イベントをローカルで再シリアライズし、本文の1 byteだけを反転した複製を作って、行デコード前にCRC不一致として拒否することも両DBで確認した。元の受信イベント・DB・ログファイルは変更しない。制御用と購読用の接続は両方ともTCPに固定し、MySQLでは両方のSsl_cipherが空でないことも検査する。正常ログの能力試験と、この限定的な破損試験は、truncated bodyや異常サイズを含む破損入力全般の安全性の証明ではない。

## 実イベントの保留と試験用の永続反映

実binlogからChangeBufferへ渡すfixture用adapterを追加した。初回の人工Rotate・FDEを要求したファイルと位置に照合し、以降のイベントサイズ・CRC・位置の連続性を検査する。MySQLのGTID前置きとBEGIN、MariaDBの互換BEGIN・トランザクション内部の単一行dummyコメントを区別する。未知のイベント、一般SQL、XAや実rotationは通さない。

MariaDBの初回Rotateはチェックサム文脈が未確定の段階でもCRC32を含んでいた。末尾4 byteを盲目的に削らず、元フレームをCRC32付きで再解析し、CRCを検証してから名前を照合する。これによりMariaDBは17イベント全部のCRCを確認した。MySQLの初回人工通知はチェックサムなしで、以降16件を確認する。

MariaDB 11.8.9の既定設定では、取引内部の終了位置が0になる。既知の取引内部フレームだけ物理サイズで位置を累積し、XIDの明示位置と一致した場合だけ反映する。算出した中間位置をcheckpointへ公開しない。1フレーム欠落の試験ではMySQLは直後の位置、MariaDBはCOMMIT位置の不一致で拒否され、どちらも初期位置と空の反映状態を維持する。設定をlegacyへ変更して穴埋めしていない。[MariaDBの変数仕様](https://mariadb.com/docs/server/ha-and-performance/standard-replication/replication-and-binary-log-system-variables#binlog_legacy_event_pos)、[互換通知の固定版ソース](https://github.com/MariaDB/server/blob/mariadb-11.8.9/sql/log_event.cc)を参照。

ChangeBufferのapply_pending_asyncでDB処理をawaitできる。エラーやキャンセルでも、COMMIT自体は済んでいる場合がある。メモリ側checkpointだけを正本にせず、永続のsource・位置とバッチ内容を検査し、同一再試行は二重反映しない契約にした。モデル試験は9件。これは外部処理のatomic性を自動提供する仕組みではない。

fixture内のrp_cdc_checkpointとrp_cdc_journalをInnoDBで作り、SELECT FOR UPDATEの後、行イベントと位置を1トランザクションで更新する。同位置の再試行は保存済みのバイト列を照合し、同じなら重複挿入せず、違えば型付きの衝突エラーを返す。実COMMIT後に応答だけ失った状態を模擬し、再試行後も3バッチ・3保存行のまま、同一再送の受理は1件、内容を変えた再送は拒否されることを両DBで確認した。

ジャーナル自身の書込みも同じサーバーのbinlogへ出るため、試験は先に確定したsource終端までの有限区間だけ読む。製品の保存先・フィルター・自己更新ループ対策をこの試験で決めたものではない。source_epochはfixture UUIDで、再接続・復元をまたぐ製品用の安定識別子ではない。保存するのは行イベントの不透明なペイロードで、再起動後に再生可能な正規化データ形式・schema/table-map保存・検索索引・公開世代ではない。初期snapshotとの対応、連続購読、DDL後の再構築／rotation／ログ消失、公開／キャッシュ連携は残件。

## 分類関連4表のraw読取り

terms・term_taxonomy・term_relationships・termmetaを追加した。name／slug／taxonomy／descriptionとmetaはバイト列で取得し、未登録のtaxonomy、保存済みのcount、孤立したterm／object参照をフィルターやJOINで落とさない。term_groupとcountは符号付き64bit、term_orderは符号付き32bit、IDとparentは符号なし64bitで保持する。分類名や説明、meta値はDebugへ出さない。公開可否の判定やWP_Term APIを提供するものではない。

各ページのNoneはゼロを含む先頭。term_relationshipsの継続位置はTermRelationshipKeyのobject_idとterm_taxonomy_idの組で、辞書順に厳密に後の行を取得する。object_idだけを継続位置にすると同じobjectの残りを失うため、複合キーを必須とした。ページサイズ1・2・3で同一object内を分割し、(0,0)、(0,u64::MAX)、(u64::MAX,u64::MAX)を含む全行が欠落・重複なく読めることを両DBで確認した。

4表の各々で欠落／非InnoDBをsnapshot開始時に拒否し、同時DMLは古いsnapshotへ見えず新しいsnapshotで見えることも確認。snapshotの検査範囲を3表から7表へ拡張したため、このAPIは分類表も存在するWP構造を要求する。同時DDLとbinlog位置の対応は引き続き未保証。

## ログ開始位置の取得方式の保留

初期snapshotとログ座標を合わせる本体APIでFLUSH TABLES WITH READ LOCKを取得する案は、自動承認レビューにより拒否された。試験範囲を越えてサーバー全体の書込みを止め、キャンセルや解放失敗がサービス全体へ影響し得るとの理由。本体へこのAPIや代替の隠れた呼出しは追加していない。ログ位置との対応を未実装として維持し、読取り・分類・失敗系の開発を続ける。権限・停止許容範囲・取得方式は別途具体化が必要。

## 移行済みコメントのraw読取り

comments全15列とcommentmetaを追加し、snapshotで扱う表は9表になった。コメント本文・投稿者名・メール・URL・IP・agent・承認状態・種別は元のバイト列として取得し、Debugへ出さない。日時はゼロ日付を失わない文字列、karmaは符号付き32bit、各IDは符号なし64bitで保持する。コメントメタはNULL／空／重複／孤立参照を保持する。

新規受付を初期無効にする設定と既存コメントの保持は別に扱い、raw readerでは承認済みだけに絞らない。spam・trash・独自承認状態、pingback・trackback・独自種別も読み出す。公開用の可否判定やコメント投稿APIは未実装。

両DBで、ゼロIDと最大IDをまたぐページ送り、全列の一致、ゼロ／有効日時、同時更新前後のsnapshot、各コメント表の欠落／非InnoDB拒否を確認。全体では各22項目が成功し、binlog試作・Rust71件・Python12件・HTTP・Linux向けcheckも成功。ログはtarget/comments-full-check.log。WordPress比較4,768ケースは直前の検証結果と整合する。

## 単一サイト用12表のrawリーダー

users全10列、usermeta、links全13列を追加した。これで固定した単一サイト用12表すべてにページ走査がある。snapshot開始時には12表すべての存在・InnoDBを要求し、使っていない表が欠けていても黙って省略しない。欠落表の修復や既存DBへのDDL実行は本体APIに含めない。

ユーザーのpassword／activation key・識別情報・metaは元のバイト列を保持し、DebugにはIDだけを出す。capabilityのシリアライズ値や重複行はそのまま保持し、このリーダーで認証・認可を行わない。legacyリンクの非表示・独自状態も保存値のまま取得し、ownerはuint64、ratingとuser_statusはint32、日時はゼロ日付を保持する文字列として扱う。

ゼロ／最大ID、全列、NULL・空・重複／孤立usermeta、同時更新をまたぐsnapshot、各表の欠落・非InnoDB拒否を両DBで検証した。各28項目が成功し、Rust73件・Python15件、binlog試作・HTTP・Linux向けcheck・依存台帳の確認も通過。ログはtarget/all-core-tables-check.log。

これは単一サイトの固定構造を読む土台。マルチサイトのglobal表・カスタムusers配置・独自表・legacy charset・同時DDL・ログ位置の対応・公開データ生成・連続CDCは未完了で、WP API全体や移行完了を示さない。公開プロセスは引き続きDBクレートへ依存せず、これらのアカウント行を受け取る経路も持たない。

## 実DDLを含む区間の停止試験

行変更／ジャーナルの区間とは分けて、同じ所有確認済みfixtureのrp_cdc_probeに列を追加し、その前後のログ位置を固定して読む。既存サイトやWP表、グローバル設定・権限・サーバー全体ロックは操作しない。

MySQLではCRCと終端位置を確認した実ALTERでSchemaChangedとなり、旧checkpointのまま反映行なしで停止する。MariaDBではALTERに先行するstandalone GTID互換通知が# Dumという短いQueryEventになり、現行adapterは未分類SQLとしてSchemaChangedで先に停止した。正の分類器は変更せず、負の観測用プローブだけが実ALTERを読むところまで進み、ALTER再送も終了処理も停止を解除しないことを確認した。この違いはddl_prefix_rejectedがMySQL=false／MariaDB=trueとして証跡へ記録する。

MariaDBで実ALTER固有の分岐へ健全な状態で到達できたとは主張しない。一般的なstandalone通知・native GTID・再接続の対応と、DDL後の再構築・公開／キャッシュの全失効は残件。両DBで今回の区間を拒否した事実と、製品のDDL復旧完成を区別する。

## 同一fixtureでの接続再開

最初のCOMMITを永続化・再送照合した後に購読を終了し、SQL接続も別connection IDの接続へ作り直す。旧SQL接続を閉じた後、期待するfixture epochとファイルを照合して保存済みの位置を読み直す。誤ったepochだけでなく誤ったファイル名も拒否する。新しいProbeBufferは別の非永続session identityと空のデコーダー状態で開始し、SQLから取得した位置から残り2 COMMITを読む。

両DBで元の3 COMMIT／3行だけが保存され、応答紛失後の同一再送は1回のまま、欠落・重複がないことを確認。再接続の人工Rotate／FDEを含め主区間19イベント、CRCはMySQL17件／MariaDB19件となった。再開位置は最初に固定したsource終端より前で、ジャーナル自身の書込みを読む連続フィルターではない。

これは同一の稼働fixture内の接続再開。プロセス／DB再起動・バックアップ復元・source置換・ログrotationをまたぐ安定識別子と再開手続きは残件。

## 欠落したログファイルへの要求

SHOW BINARY LOGSで存在しないことを確認したfixture固有名を要求し、両DBでerror 1236／HY000を確認した。通常EOF・正常イベント・timeoutはこの試験の成功にしない。失敗した要求の前後にジャーナルの3行とcheckpointを照合し、不変を確認する。既存ログを削除したり、保持設定や権限を変更したりしていない。

mysql_async 0.37.1のError::is_fatalはこのServerErrorでfalseだった。これは汎用SQL接続の分類であり、binlog購読が健全・最新である証拠にはならない。製品coordinatorの停止・キャッシュ全失効・再構築への接続は残件。

結果のversion付きJSONはscripts/binlog_probe_result.pyで厳密に検査する。booleanと1、重複key、欠落・未知field、イベント合計の不整合、backendの取り違えを拒否し、保存証跡と元の結果も照合する。製品用IPCやイベント永続化形式を採用したものではない。

## 行型の固定コーパス

別の架空表の4列で、5つのunsigned ID（0／1／i64::MAX／2^63／u64::MAX）、signed BIGINTの最小／最大・負数、NULLと空・NUL／非UTF-8のVARBINARY、UTF-8とNULを含むVARCHARを検証した。SQLの列定義とTableMapのBIGINT型・NULL bitmap・DB／表名を照合し、1 COMMITと終端位置も確認する。

MariaDBのunsigned上位2件はライブラリーがValue::Intの負値として返し、固定BIGINT UNSIGNED列でのみbitを解釈して元のuint64を得た。MySQLではこの再解釈件数は0だった。隣のsigned列はsignedの値を保持する。解釈を任意のIntや未知の列へ適用しない。証跡へtyped_rows_verified=5とtyped_unsigned_reinterpretedを記録する。

これは固定型と同時DDLのない専用fixtureに対するcodec試験で、製品のschema cache・一般の列型・正規化データ・索引・公開連携ではない。行型区間のCOMMITはメモリ上でのみ確認し、元の3行のSQLジャーナル試験と区別する。

## 投稿メタの再構築可能な索引試作

meta_view::PostMetaViewは、呼出し元が同じsnapshotから集めた行集合をmeta_id順に保持し、必要なときだけ完全一致キーの索引を作る。NULLと空キーを分け、重複meta・孤立行・元値・uint64境界を保持する。主キーの重複は拒否。索引のoffsetは行集合内部に閉じ、別の行集合へ流用できない。

matching_keyのSomeは明示的なバイト完全一致、NoneはIS NULLに対応する。通常のcollationやWP_Meta_Queryを置き換えない。fixtureでは大文字小文字・末尾空白・NUL・日本語を含むキーについて、両DBのCAST(... AS BINARY)検索、索引、全件走査の結果順とIDを比較し、通常collationとの相違も確認した。

build_key_indexは許された行数に全体が収まる場合だけ完成した索引を使い、縮小時は全索引を破棄して同じ行集合を走査する。これはホストが渡す試作用の件数制約で、自動容量式、byte／RSS上限、共有予算ではない。入力行の全DB網羅性や最新性をこの型だけで証明せず、全テーブル常駐を製品条件にしない。

prepare_changesは完全な更新前イメージを照合してinsert／update／deleteを順に適用し、別の未索引行集合を準備する。途中の不一致・主キー衝突・欠落行では以前の行と索引を変更しない。同一行への連続操作や主キー変更も照合する。実DBの更新後snapshotとの一致を確認した。ただし、このAPIはCOMMIT・source・ログ位置を証明せず、コピー用のメモリ配分、耐久化、再送識別、公開切替やChangeBufferとの統合は未完了。

各32項目と既存binlog試験が通過。索引・変更準備の単体試験は6件。最終DBログはtarget/meta-view-prepared-changes-check.log。DB証跡の必須ファイル集合も検証側で照合し、実装・harness・Cargo設定の省略や同件数の差し替えを拒否する。

欠落ログ試験で、error 1236後の接続終了時にos error 10054が出て拒否応答を隠すケースが発生した。拒否応答を先に検証し、その後の終了処理とは分離した。通常EOF・timeout・接続resetだけでは成功にしない。両DBで再検証済み。失敗fixtureは停止を確認し、server.logをtarget/development-logsへ保存して回収した。

## 件数制限付きの完全な行集合構築

EditingSnapshot::load_post_meta_viewは既存のread-only consistent snapshotからページ送りする。上限内の全postmetaが取れた場合だけ未索引のPostMetaViewを返し、超過時はLoadError::RowLimitExceededで全結果を破棄する。完成したprefixや部分索引を返さない。失敗後は同じsnapshotで通常のpaged readerを続けられる。

fixtureの11行に対して上限0／10の拒否、11／usize::MAXで全行取得、ページサイズ1／3／32を両DBで確認。meta_id=0とu64::MAX、孤立行も保持した。各34項目へ拡大。件数はホスト内部の試作用制約で、ページ・row bytes・索引の共有メモリ予算や自動算式を提供するものではない。snapshotとログ位置の対応付けも引き続き未完了。生成4,096行では1,027のキー条件を、構築前・構築後・破棄後・再構築後で独立走査と照合した。最終の一括ログはtarget/combined-meta-and-wp-check.log。

## 実binlogからの投稿メタの有限反映

別区間で、固定WPスキーマのwp_postmetaをSQL定義とTableMapに照合して4列をデコードした。2 INSERT、主キー・post_id・key・valueを変える1 UPDATE、2 DELETEの計5変更を3 COMMITごとに別PostMetaViewへ準備し、各時点の独立したSQL読取りと全raw列・順序・binaryキー索引を比較した。NULL・NUL・Unicode・serialized bytes・unsigned ownerを保持し、ロールバック更新は現れない。以前のArc行集合は変化しない。

並列の負のconsumerでは最初の反映を拒否し、checkpointと可視データが進まないことを確認。staged batchの元のrow-event bytesと、デコードした変更の元フレームも照合した。両DBでpostmeta_changes=5／postmeta_memory_commits=3と各一致・旧view保持・拒否時維持を記録する。

これは直列・有限・所有fixtureのメモリ内反映。元の3バッチの耐久SQLジャーナルや固定型5行とは別区間で、索引・正規化データの耐久化や再送receipt、並行snapshotとログの対応付け、一般schema cache、連続購読、公開・キャッシュ連携の完成ではない。既存DB・グローバルロック・追加権限を操作しない。当時のDB／binlogログはtarget/postmeta-binlog-projection-check.log。

## 最終候補の件数・論理bytesを制限する変更準備（2026-10-27）

PostMetaView::prepare_changes_with_limits(&self, changes: &[MetaChange], max_rows, max_logical_bytes)を別APIとして追加した。既存のowned IntoIteratorを受けるprepare_changesの挙動は変更しない。最初はBTreeMap<u64, &PostMetaRow>だけで旧行と借用changesを合成し、順番どおりの完全before image、欠落行、重複ID、主キー移動を検証する。全strict検証の完了後に最終行数、次に論理bytesをchecked加算で検査し、適合した場合だけ最終行bodyをcloneして未索引candidateを返す。失敗時は旧viewと索引を変更せず、部分candidateも返さない。

LimitedChangeErrorはChange(ChangeError)／RowLimitExceeded { limit }／ByteLimitExceeded { limit }を区別する。strictな変更エラーは上限エラーより優先する。NULLと空bytesを混同せず、non-UTF8・ID0／u64::MAX・同一行への連続変更・PK移動とID再利用も保持する。元viewより小さな上限へ削除で収まる例と、insert→deleteの最終0行が0行／0bytesで成功する例を含む新契約5件、既存変更契約7件が通過した。payload clone前の拒否は実装の検査順で保証し、global allocatorやunsafeの試験補助は追加しない。

論理chargeは**最終candidate**の各size_of::<PostMetaRow>()＋NULLを除くkey／value.lenだけ。old viewとcandidateの同時保持、changes、参照map、index、Vec／allocator capacity、driver、RSS、peak memory、共有budgetを計上したものではない。自動配分算式や製品のメモリ上限を完成させるAPIではなく、source位置・COMMIT・公開の保証も追加しない。

有限postmeta probeにも接続し、通常3 COMMITはborrowしたchangesを64行／64KiBのfixture内部allowanceで準備し、各COMMITの全raw行と索引を独立SQL結果に照合する構成にした。別consumerの最初のCOMMITではbyte allowance 0をcallbackへ渡し、downcastしたexact LimitedChangeError::ByteLimitExceeded { limit: 0 }と、旧行／旧索引／checkpointの維持を必須にする。単なる任意エラーを拒否成功としない。結果のpostmeta_logical_limit_preserved=true／postmeta_limited_commits=3はPythonでstrictな型とmetadata相関を検査する。64行／64KiBは所有fixtureの検証値であり、利用者向け手動product設定ではない。

対象unit・DB all-targets Clippyに加え、target/limited-meta-changes-full-check.logの全gateが成功。workspace214件＋独立experiment19件・Python68件、通常両DBと専用GTID MySQL各36項目、上限0の型付き拒否と3 COMMIT後の全raw／索引比較を確認した。両binlog証跡の新2fieldも照合し、全3 fixtureを回収。WP44 API・286,998ケースはcurrent hash検査で維持しPHP再実行なし。既存cold restore／pre-COMMIT kill／選別／overlap実験の成果と本番CDCの残件は維持する。

## 索引の最終論理bytes上限（2026-10-27）

PostMetaView::build_key_index_with_byte_limit(max_entries, max_logical_bytes)を別APIとして追加した。既存build_key_index(max_entries)の挙動は変更しない。最初に旧indexを破棄し、行数上限を検査する。次に借用keyのBTreeSetで非NULL keyのunique bytesを調べ、以下をchecked計算する。

`全行数 × size_of::<usize>() + Σ_unique非NULLkey(size_of::<(Vec<u8>, Vec<usize>)>() + key.len())`

全行のoffsetにはNULL行も含む。同じkey bytesのentry headerとkey bodyは1回だけ計上し、NULLと空keyは区別する。view内のinline KeyIndex rootは追加chargeから除外する。empty viewは0行／0bytesで索引を構築できる。全chargeが適合した後に借用setをdropし、それから既存builderでowned key／indexを構築する。overflow・超過時は部分indexを返さずScanへfallbackし、rowsは不変。拒否後の再構築も可能。

この上限は最終logical indexだけで、借用preflight、tree-node overhead、Vec／allocator capacity、行body、peak allocation、RSS、共有budget・自動算式は含まない。旧viewの行と構築中indexを合わせた実メモリ上限ではない。新契約5件でexact／-1／0／MAX、NULL／空key、duplicate keyの1回計上と全offset、non-UTF8・case・ID0／MAXの順序、indexからScanへの拒否と再構築、empty 0を確認した。

native postmeta consumerは3 COMMITのcandidateごとに、0bytesでScan、64KiBでExactKeyIndexとなることと、両状態の全raw行・各key検索結果が独立SQL期待と一致することを確認した。結果postmeta_byte_limited_indexes=3をstrictなPython metadata検査でも照合。64KiBは所有fixtureの内部試験値で、製品の手動容量設定ではない。target/limited-index-native-check.logでisolated19件・Clippy・通常両DBと専用GTID fixture各36項目、既存cold restore／kill／filter／limited changes／overlapが通過し全3 fixtureを回収した。別gateのtarget/limited-index-and-license-check.logでworkspace219件・Python77件、HTTP・Linux向けcross-check・証跡current hash確認が成功。PHPは再実行せずWP44／286,998を維持した。

## 投稿メタの試験用原子保存と復元

上記のメモリ内反映に続け、fixture専用rp_postmeta_projection表へ、型付き行集合・epoch／file・from／through位置・元フレームreceiptを1 InnoDB transactionで保存する候補試験を追加した。WP正本表の構造や製品DBを変更しない。SQL表・JSON形式の製品採用や、索引そのものの耐久化は意味しない。

最初のCOMMIT直後に応答紛失を注入すると、メモリ側のcheckpointと旧viewは進まず、保存済みの行・位置は進んでいる。同じpending batchの再送でreceiptと行データを照合し、再更新せず復帰する。変更データの再送は型付きReplayConflictで拒否し、保存済み行・位置を保持する。

別connection IDのSQL接続へ作り直し、行と位置を同時に読み戻して索引を再構築する。誤った期待epochだけでなくfileも型付きSourceContextとして拒否した。別のSQL接続から未COMMITの行・位置が見えず、ROLLBACK後に元のreceiptを含む全保存値が戻ることも確認。

両DBでpostmeta_durable_commits=3、postmeta_durable_replays=1、応答紛失・競合拒否・再読込み・rollback保持・context拒否が成功。旧ログの3バッチや固定型5行とは区別する。ログはtarget/postmeta-rollback-and-context-check.log。プロセス／DBクラッシュ、安定source identity、snapshot/log anchor、連続購読・公開連携は引き続き未実装・未検証。

## 保存projectionからの購読再開とSQL終了観測

最初のprojection保存と同一再送の照合後、SQL接続を作り直して行・位置を復元するだけでなく、購読接続も閉じて別ID 7007で再作成した。新しいProbeBufferとtable-map状態を保存済み位置から開始し、残り2 COMMITを取り込む。旧側1＋新側2で元の3 COMMIT・5変更だけを反映し、各時点のSQL結果・索引が一致。両DBでpostmeta_source_resumed=true。これは同じ稼働fixture内の再接続で、プロセス／DB再起動や安定source identityの検証ではない。

MySQLで間欠的に出た10054は、段階付き診断と反復試験により、既存ジャーナルと投稿メタジャーナルの旧SQL接続をdisconnectする箇所で観測した。固定mysql_asyncはCOM_QUIT送信後にTLSをcloseする。使用済み・消費済みのSQL接続の終了だけでConnectionReset／BrokenPipe／UnexpectedEofを記録し、新接続の行・位置・receipt照合を継続する。接続確立・購読・SQL・COMMITのエラーをこの扱いにしない。ServerError 1236やtimeout等を無視しないことを独立experimentの単体試験でも確認。

sql_disconnect_resetsは終了箇所別の観測で、ログ健全性の証拠ではない。新しいMySQL fixtureで4回反復し全回が成功。2回はそれぞれpostmeta_journal_old=1／original_journal_old=1、他2回は空だった。先に整理したbinlog終了処理を根本原因の確定と扱わず、実際に観測したSQL終了と区別する。MySQL fixtureだけ詳細error logを有効化した。失敗fixtureは停止確認後に回収し、server.logをtarget/development-logsへ保存。最終一括ログはtarget/source-resume-and-cleanup-final-check.log、反復ログはtarget/mysql-source-resume-repeat-1.log～4.log。

### 構築上限の超過確認

行数上限へ達した後の超過確認は、post_meta_exists_afterによるIDだけのSELECTへ変更。max_rows=0や上限到達後に、拒否するためだけのmeta_valueを追加取得しない。上限内のページもremaining件までに制限する。整合snapshot内で行の有無を確認し、全行か拒否のどちらかを維持する。これはrow bytesやdriver bufferを含む総メモリ上限ではない。既存の0／10／11／usize::MAX・各ページサイズの両DB試験で検証する。

当時の一括検証ログはtarget/final-meta-recovery-and-compatibility-check.log。ID-only上限確認を含む両DB各34項目とpostmetaの原子保存・再送・購読再開が成功し、記録のソースhashも一致する。試験用DBは停止・回収済み。

## 正常終了後の別プロセス復元

producerの正常終了をPopen.communicateで待ち、起動した実PIDを結果JSONと照合してから、別のOSプロセスを--restore-postmetaで起動する。新プロセスも元のfixture guard（実datadir・port・version・CA／name）を通り、CREATEより前で復元専用分岐に入る。DBサーバーは同じ所有Popenが稼働し続けたことを前後で確認する。

保存された11行、epoch・file、from<throughと期待位置、非空receiptの形状とサイズ、ログ索引のFile_sizeを照合する。receiptは元row-eventのpayloadであり、完全wire frameのCRCをここで再検証したとは扱わない。PostMetaViewと索引を新規構築し、静止したfixtureのWP正本の全raw行・キー検索と比較。前後の保存tupleも不変であることを確認した。

file・position・row数の食い違い、所有fixture flagなし、余分なCLI引数の5ケースを別プロセスで拒否する。exit1だけでなく、ケース固有の失敗理由とstdoutが空であることを検査し、接続失敗やpanicを成功に数えない。最後に再度正常復元し、拒否試験の後も元の保存状態が復元できることを確認する。実PID、起動順、各拒否理由と最終確認はbinlog証跡のfresh_process_restoreへ保存する。

MariaDB／MySQLで成功。これは正常終了後のcold restoreと同一稼働DB内の再接続試験で、強制終了・OS／DBサーバークラッシュ・電源断・安定source identity・並行snapshot/log anchorの検証ではない。ログはtarget/fresh-process-refusal-and-confirmation-check.log。製品用ストレージ形式・復元CLIの採用ではなく、独立experiment内だけの機能。

### 数値metadataとprocess lifecycleの厳密化

結果・証跡の整数はJSON bool／floatとの同値比較で通さず、型・範囲・固定値を照合する。件数・checksum数・位置・PID・必須fieldと重複キーも検査する。harnessは所有Popenの正常終了後に結果を採用し、timeout／失敗時の子process停止・回収を扱う。producerと別PIDでの復元、DBが前後で稼働していること、5拒否後の最終正常復元を照合する。これはproduction IPCやプロセス障害耐性の契約ではない。

## snapshotとbinlog位置の追加観測（2026-10-26、実測済み）

MariaDB11.8.9専用の`rp_snapshot_anchor_probe`を追加した。controlでInnoDB表と旧値を作り、別readerの同じ物理接続でRR・`START TRANSACTION READ ONLY, WITH CONSISTENT SNAPSHOT`を開始する。`SHOW SESSION STATUS WHERE Variable_name IN ('Binlog_snapshot_file', 'Binlog_snapshot_position')`で2値を取得し、reader最初の表SELECTより前にcontrolで新値へUPDATE・COMMITする。通常global位置が同じfile内で進む一方、readerは旧値と同じanchorを保持すること、書込みは1792で拒否され、その後も旧値とanchorが変わらないことを確認する設計。空statusやglobal fallbackらしい変化を成功扱いしない。MySQL側でこの専用anchorはnullとし、通常SHOW位置で代替しない。

別のMySQL8.4.13専用`rp_mysql_snapshot_race`では、read-only consistent snapshot開始後・最初の表SELECT前に別接続でCOMMITし、通常`SHOW BINARY LOG STATUS`が後続COMMITの位置を返してもSELECTは旧値になることを観測する。これは通常SHOWをsnapshot座標と見なす方式への有限の反例候補で、MySQLの全方式が不可能という証明ではない。

どちらも既存所有/version/datadir/port/TLS guard後のProduceモード末尾で専用toy表だけを使う。両固定fixtureで実測成功。MariaDBは`probe-bin.000001`のanchor73651を保持してglobal73954へ進み、旧値可視・1792拒否後のanchor／行不変を確認。MySQLはbefore69375からreported69735へ進む間もsnapshot旧値・writer新値を保持し、1792拒否を確認した。これらの数値は今回の有限fixtureの観測値であり、固定期待位置ではない。新moduleを含む証跡を再生成して一括照合し、ログは`target/snapshot-anchor-and-shared-budget-full-check.log`。本体API・連続CDC・公開連携・安定source識別や製品の版／権限方針は追加していない。サーバー全体の書込み停止と失敗時の影響を理由とするFTWRL本体APIの自動承認レビュー拒否は未解決で、再試行・別名実行をしていない。

公式根拠では、MariaDBの両snapshot statusはsnapshot後の他transactionのCOMMITと独立に照会できる。docsのScope表記はGlobalだが、11.8.9 sourceは現在THDの保存snapshot位置を優先し、保存値がなければglobal last-commit位置へfallbackする。[status定義](https://mariadb.com/docs/server/ha-and-performance/standard-replication/replication-and-binary-log-status-variables)、[11.8.9の保存・返却処理](https://github.com/MariaDB/server/blob/mariadb-11.8.9/sql/log.cc#L6246-L6260)、[返却時のfallback](https://github.com/MariaDB/server/blob/mariadb-11.8.9/sql/log.cc#L12302-L12325)

同一snapshotとbinlogを揃える処理には内部`LOCK_commit_ordered`があり、「一切のロックやcommit待ちがない」とはしない。log_bin有効・同じ物理接続・複数SELECT全体を固定するRR・InnoDB等の対応engineが前提。READ COMMITTEDや非transactional表へ保証を広げない。参照表のMDLやDDLのimplicit commit、schema変更との競合・binlog保持は別課題。[同期処理](https://github.com/MariaDB/server/blob/mariadb-11.8.9/sql/handler.cc#L3046-L3058)、[isolation別挙動](https://mariadb.com/docs/server/ha-and-performance/standard-replication/enhancements-for-start-transaction-with-consistent-snapshot)、[transactionとDDL／MDL](https://mariadb.com/docs/server/reference/sql-statements/transactions/start-transaction)

binlog対応はMariaDB5.3の公式変更履歴にも記載され、今回は11.8.9固定sourceを確認した。MySQLへ同じ変数・保証を転用しない。[導入履歴](https://mariadb.com/docs/release-notes/community-server/old-releases/5.3/changes-improvements-in-mariadb-5-3)

共有論理予算の単一process・6分類の候補は[開発メモ](development.ja.md)に記録した。BudgetedPostMetaViewはInternalRows／Index予約をowned dataの寿命へ接続する。postmeta有限windowではBudgetedChangeBufferのraw保持も同じpoolへ接続する。全strict参照検証→行予約→payload clone、索引はcharge→追加予約→offside構築→swap→旧data drop→旧lease dropの順。旧世代と新世代を同時に計上し、Arcの最終dropまでleaseを保持する。BudgetのExhausted／Unavailableでは旧indexを維持し、明示cap拒否だけScanへ戻してindex leaseを返す。既存unbudgeted APIやdecoder／比較用コピー等の保有分は未計上で、実監視・自動容量算式・node／capacity／driver／ref map／peak／実RSS・cross-process調整・CLI cache接続の完成ではない。[予算付きviewのAPI・loader・検証経緯](budgeted-meta-view-development.ja.md)。同じRRの長さpreflight後に予約して本文を読むloaderと1MiB／pool0拒否もfixtureに追加したが、初回full gateは最後の古いschema helper記録検出で失敗したが、DDL不変の再生成と起動前tool_files guard修正後のfinal gateは成功した。初回失敗と修正の履歴も維持する。

## postmetaの長さ先読みと論理byte上限

旧loaderを維持し、`load_post_meta_view_with_byte_limit(max_rows, max_logical_bytes, page_size)`を追加。同じread-only RR snapshotでIDとNULLを保持したkey/valueのOCTET_LENGTHだけを全表走査し、行数と論理bytesが適合してから本文を読む。上限到達時の超過確認はIDだけで行い、超過時は部分viewを返さずsnapshotを通常ページ読取りに再利用できる。本文取得後も行数と合計chargeを再照合する。

論理chargeは1行あたりsize_of::<PostMetaRow>()とkey/valueのbyte長。NULLのpayloadは0だが空bytesとの区別は本文へ保持する。整数変換・加算はcheckedで、上限超過とshape不一致は値を含まないエラーにする。2回走査するコストがあり、driver/page領域・Vec余剰capacity・allocator・索引を含む実RSS上限ではない。共有poolとは未接続、自動容量算式も未決。内部hostが渡すallowanceであり、製品へ手動容量設定を追加しない。既存の安定schema前提を維持し、行数と合計chargeの再照合だけで任意の並行DDL・同サイズ改変を完全検出するものではない。

両DBで0・上限ぴったり・1byte不足・usize::MAXとpage size 1/3/32、ID=0/MAX・NULL・日本語/NULのbytesを確認。別接続の更新後も古いsnapshotの長さと本文が一致する。さらにサーバー上で1MiBの値を作り、拒否前後の同接続Bytes_sent差が64KiB未満であることを確認し、本文を取得せず拒否したことを検査。値はfixture内で元に戻し、後続binlog/restoreの11行条件を維持した。各DB36項目、新pure unit3件と全gateが成功。ログは`target/byte-preflight-and-html-replacement-full-check.log`。

## 未COMMIT clientの強制終了と復元（2026-10-26）

独立experimentへ--prepare-postmeta-crash／--resume-postmeta-crashの2 modeを追加した。元のwp_postmetaと通常projectionは変更せず、rp_postmeta_crash_sourceへ11行をcloneし、専用rp_postmeta_crash_projectionへbaselineの行・位置・receiptを保存する。clone時のAUTO_INCREMENTがID0を書き換えないようSESSION NO_AUTO_VALUE_ON_ZEROだけを必要時に追加し、元session modeへ復元する。失敗時も復元を試み、global設定は変更しない。

prepareはsourceの後続COMMITまで有限ログを準備し、その更新後tupleを専用journal transactionで書き換えるが、SQL COMMITする前にREADYをflushして待つ。harnessは自分が起動したclientのactual PID、32hex nonce、fixture epoch／file、順序付き位置を照合し、まだ実行中であることを確認してそのPopenだけをkillする。DB本体や既存processはkillしない。DBは同一所有Popenで前後とも稼働中であることを検査する。

READYはcanonical JSONの1行で64KiBまで。old tupleのreceiptと行payloadはそれぞれ12KiB、receiptはexact1非空frame、11行はuint64 ID・NULL／byte列・順序を検査する。boolを整数として通さず、重複key・誤nonce／PID／epoch／file・境界・不正hex等を拒否する。stdout READY readerはthread＋queueで上限を守り、timeout・parse失敗も所有clientをkill／waitしreaderを終了させる。resumeへは検証済みmarkerをbounded stdinで渡す。余分stdout・stderrも成功扱いしない。

resumerはsessionの短いInnoDB lock wait timeoutとasync timeoutの下でjournal行のFOR UPDATEを取得する。単なるconsistent readは未完了rollback中でもbaselineを見られるため、行lockを取得してrollback完了を待ち、保存tupleがREADYの旧値と完全一致することを確認する。保存済みbaseline位置から新decoderで1 COMMIT／1 UPDATEだけを再反映し、11行の全raw値と索引を専用sourceのSQL結果へ照合する。通常projectionへ--restore-postmetaを最後に再実行し、元の行・位置・receipt／索引が引き続き復元できることも確認する。

process_crash_recoveryへactual PID・kill終了コード・nonce／epoch・READY・resumer結果・通常restore最終確認を記録する。Windowsの非zero終了コードをUnixの負signal値へ限定せず検査する。最新全gateはtarget/precommit-process-kill-final-check.logでRust185件＋独立experiment9件・Python59件、両DB各36項目と本実験が成功。旧失敗fixture2 UUIDは停止を照合しtarget/development-logsへログ保存後回収、現在のdb-runsは空。

これは**SQL COMMIT前**の専用client強制終了と同一稼働DB上の有限replay。COMMIT途中の曖昧な成功／失敗、DB server／OS crash・power loss、安定source identity、連続購読・retention・公開／cache連携は未検証。実監視・容量自動算式や共有pool接続も完成していない。

## 既知の同一DB表を選別する有限fixture（2026-10-26）

新しいfiltered_postmeta experimentは、同じrecastpress_fixture内のrp_filter_target（WP postmetaの11行clone）とrp_filter_otherだけを認識する。clone時のID0はSESSION NO_AUTO_VALUE_ON_ZEROの追加・復元で保持し、globalを変更しない。未知database／tableは拒否し、別DBへのgrantを追加しない。WP正本表・通常projectionは変更しない。journal書込みは有限source endpoint確定後に行い、自分の書込みを読む問題をこのfixture内だけで回避する。

元の全raw RowsEventは選別前と同じくstage／receiptへ保持し、MetaChangeだけをtargetに限定する。mixed、ignored-only、target-onlyの3 COMMITでtarget変更3件・除外変更2件を確認する。ignored-onlyでも行集合を保持して新COMMIT位置・全raw receiptを保存する。除外を理由にframeを捨てず、保持payload予算を削減したとはしない。core／adapterの既存Advance契約を変更せず、prod用skip／advance方式を採用したものでもない。

全TableMapをrow有無にかかわらずSQL定義に対応するdtype／null maskまで検証し、target／otherの全row imageをdecodeする。各COMMITでepoch・file・filter profile・from位置・through位置・全receipt・row dataの7tupleを期待値へ照合する。ignored-onlyのSQL適用失敗はrollbackしてpending／checkpoint／旧tupleを保持し、次のCOMMIT直後のACK lossでもmemoryは進めず、同じreceiptのreplayで二重更新を避ける。wrong profileは特定の拒否を要求し、前後の全tuple不変を検査する。任意エラーを負試験の成功に数えない。

除外RowsEventのCRC改変・欠落frameによるposition gapも拒否する。除外表へのactual ALTERもSchemaChangedをlatchし、再投入で回復せず保存tupleを保持する。MariaDB standalone互換通知によりignored_ddl_prefix_rejected=true、MySQLはfalseとなる点を分けて記録する。11行の全raw値・binary key索引は独立SQL読取りと一致した。

22 fieldの結果へcommits3／selected_changes3／ignored_changes2等を記録し、cross_database_live_verifiedはfalseとする。target/filtered-postmeta-and-cgroup-full-check.logで両DBとも成功し、既存36項目・cold restore・pre-COMMIT killと通常projectionの再確認も維持。一般のproduction filtering、cross-database live検証、安定source identity、初期snapshot／log anchor、連続購読・retention・公開／cache連携は未完成。

## MySQLの独立した重複履歴bootstrap候補（2026-10-27）

`python scripts/database_smoke.py --backend mysql --gtid-bootstrap-probe`は、GTID ON等を明示した新規MySQL 8.4.13専用fixtureでのみ動く。通常のMariaDB／MySQL probe設定は変更しない。A取得前の既commit履歴を先頭からskipし、最初のA外GTID以後はA所属も含め全transactionを非公開after-image mapへ再生する。GTIDのsource側skipを使わず全frameのCRC・位置連続性を検証し、通常のstrict classifierやAdvance契約を変更していない。

第2回専用liveではA=1～132を含む先頭132 groupを通過し、snapshot Bのページ間にPK移動・ID再利用を行って、Cまで3 COMMIT、以後strict before-imageで1 COMMITを照合した。Bと最終行数は12、元WPは11行のままで、全raw行とSQL binaryキー検索が一致。実PID・strict JSON・reader終了reset1件も検証し、fixtureを回収した。証跡は`compatibility/evidence/mysql-overlap-bootstrap.json`、再検査は`python scripts/check_mysql_overlap_evidence.py`。

実runtimeのReplayFloorを用いた非prefix A、FDE固有checksum descriptor、checked GTID group長を含むunit6件とClippyは通過。非prefix Aは純粋試験であり、このliveのAはprefixだった。Cを任意のSQL可視状態やOS耐久状態へ同一視せず、RAM構成・single file・purged空・単一origin・AUTO・ROW/FULL・ordered commits・安定InnoDB schemaに限る。rotation、匿名／tagged／手動GTID、ログ欠落・保持競合、source交換、永続化・公開・連続CDCは未完了。FTWRL本体APIの拒否方針と通常probeの制約も維持する。詳しくは[専用開発記録](mysql-overlap-bootstrap-development.ja.md)。

全体gate確認追記：専用second-live後の`target/mysql-overlap-bootstrap-full-check.log`も成功。workspace194＋experiment19・Python68、通常両DB各36項目と専用GTID MySQL36項目を記録した。WP41 API・274,683ケースの既存比較とregistryのcurrent hash照合を維持し、全3 fixtureを回収した。専用second-liveの位置・件数は前段の歴史記録として保持する。

## MariaDBのsame-session bootstrap候補（2026-10-27）

固定11.8.9の別owned fixtureで、同mysql_async sessionのRR read-only snapshotとBinlog_snapshot_file／positionを用い、旧11行のpaged取得中に3 COMMIT（PK移動／ABA含む）を行い、anchorから3件をstrict適用、reader終了後に別streamで1件を継続した。全raw／binary index、共有pool4 callbackと最後の返却、cap／pool／CRC／gap拒否、元WP12表の不変性を確認した。取得scratchは予約前で、SQLx EditingSnapshotの事前payload予算を同sessionへ転用した主張ではない。FTWRL／全体pauseを追加せず、製品bootstrap・耐久journal・source identity／rotation・連続CDC／公開は未採用。[独立開発記録](mariadb-snapshot-bootstrap-development.ja.md)。

target/mariadb-snapshot-bootstrap-full-check.logでworkspace287＋独立21 Rust・Python96、通常2 DB／MySQL GTID／Maria snapshotの全4 fixture各39項目、HTTP・Clippy・Linux cross-check・license208観測・provenance照合が成功し、全fixtureを回収した。WP45 API310,363ケース／hooks572／dispatcher405はPHP再実行なしのcurrent hash確認で維持する。
