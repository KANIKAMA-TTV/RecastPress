# MariaDB snapshot位置からの非公開bootstrap候補

2026-10-27時点、限定実装・4 fixture全gate検証済み。固定MariaDB 11.8.9の所有する使い捨てfixtureだけで、既存の同session snapshot anchor観測をpostmetaの有限再構築へ接続する。MySQLのGTID overlap候補とは独立したprofileで、製品bootstrap・耐久journal・連続CDC・公開切替の採用を意味しない。FTWRLやサーバー全体の書込み停止は追加しない。

## 同じsessionのsnapshotと位置

専用tableはwp_postmetaのcloneで、元のWP12表を変更しない。ID0を保持するためclone時だけSESSION sql_modeのNO_AUTO_VALUE_ON_ZEROを適用し、元の値へ復元する。fixtureは11行、NULL／空値／binary key／unsigned ID境界を維持する。

mysql_asyncの同一reader接続でREPEATABLE READとREAD ONLY, WITH CONSISTENT SNAPSHOTを開始し、Binlog_snapshot_file／Binlog_snapshot_positionからAを読む。同じ接続で3行ずつraw値を取得する。別sessionのSQLx EditingSnapshot loaderと組み合わせて同一snapshotとは扱わない。取得scratchは予約前に存在し、BudgetedPostMetaView::clone_from_rowsによる保持課金とは区別する。行数／byteのfixture上限は実RSSや取得前のpayload予約を保証しない。

最初のpageより前にT1でid3を更新し、page間のT2でid3を100へ移動、T3で100を削除して3へ再挿入する。3 COMMIT後も同readerの全行集合が開始時の値で、anchorがAのままであることを検査する。MariaDB内部のsnapshot開始同期を含むため、lock-freeや同時DDL対応を主張しない。

## 有限ログ適用とstrictへの継続

AからT3のXID終端Cまで通常strict decoderと同じ経路で適用する。同poolのBudgetedChangeBuffer／BudgetedPostMetaViewを使い、各COMMITで全raw行とbinary key索引をSQL期待値に照合する。初回Rotateは既存normalize_maria_initial_rotateを用い、人工FDEは既存の記述情報の開始条件に従う。MySQLのfile4走査用補正を転用せず、一般の未知event／standalone # Dum／DDL／実rotationは拒否を維持する。

snapshotをROLLBACKしてreaderを閉じ、T4をCOMMITする。新streamをCから開き、通常strict適用でDへ進む。期待する有限結果はA<C<D、bootstrap3 COMMIT＋strict1 COMMIT、snapshot／最終11行。callback拒否・pool拒否・CRC破損・gapで旧view／索引／checkpointが残ることを、具体的な理由まで確認する。任意の接続失敗を負例の合格に使わない。

## 課金と証跡の境界

4 COMMITでInternalRows／Index／ChangeBufferの予約が同じpoolに存在し、最後のownerを破棄した後に返却されることを検査する。raw取得scratch、driver／decoded input、期待値と照合コピー、ページと位置のmetadata、allocator／RSS／peakは課金外。source identity、rotation／保持期間、再起動越しの復元、本番snapshot protocol、全tableの自動最適化を証明しない。

独立profileはmariadb_11_8_9_unpublished_snapshot_bootstrap。専用CLI modeと所有fixture、厳密な型／件数／位置／process PID／固定version／source hashを持つ独立証跡compatibility/evidence/mariadb-snapshot-bootstrap.jsonを追加した。既存通常MariaDB／MySQL、MySQL overlapの証跡も新harnessで実測再生成した。hashの手動更新で代用していない。

## 実測結果

target/mariadb-snapshot-bootstrap-full-check.logでworkspace Rust287件＋独立experiment21件・Python96件、4つの所有DB fixture各39項目、HTTP・Clippy・Linux cross-check・license208観測・全current provenanceが成功し、全fixtureを停止・回収した。PHPは再実行せずWP45 API・310,363ケース／hooks572／dispatcher405を維持し、target/mariadb-bootstrap-wp-provenance-check.logでhashを照合した。

Maria専用runではsource_file=probe-bin.000001、A=4,246,641、C=4,248,068、D=4,248,466を実測。snapshot／最終11行、bootstrap3 COMMIT／strict1 COMMIT、shared_pool_commits=4と最後の返却、anchor／snapshot不変、全raw／binary index一致、4種拒否とunpublished_only=trueを厳密checkerへ照合した。sql_disconnect_resetsは空で、cleanup helperのreset回復は使われなかった。これらの位置は今回のfixtureだけの観測で、別run／別DBへ移せない。

新native純粋2件はanchor／table-map shapeを拒否し、既存19件と合わせ独立21件。新Python7件は実PID・単一bounded protocol line・23fieldの完全性／型・未知／重複field・位置順・version／transport・source漏れ／stale hash・mode排他を検査する。独立の相互レビューでもblockingな問題は見つからなかった。

専用fixtureだけなら次を実行する。

```powershell
python scripts/database_smoke.py --backend mariadb --mariadb-snapshot-bootstrap-probe
```

全4 fixtureの再実測は既存binlog／MySQL overlapとともにcheck.pyへ--with-mariadb-snapshot-bootstrapを渡す。実験modeはMariaDB専用で、別backendや同時に複数probeを選ぶ指定はDB起動前に拒否する。公開serverのCLI／endpointを追加するflagではない。
