# MySQLの重複履歴を用いる初期構築候補

記録更新: 2026-10-27 JST。独立した開発実験であり、製品の初期構築方式としての採用・完成を示さない。専用の第2回実DB試験が成功した。ログは`target/mysql-overlap-bootstrap-second-live.log`、証跡は`compatibility/evidence/mysql-overlap-bootstrap.json`。その後、全体gate `target/mysql-overlap-bootstrap-full-check.log`も成功。workspace Rust194件＋独立experiment19件、Python68件、通常MariaDB／MySQL各36項目と専用GTID MySQL36項目、HTTP・Clippy・Linux向けcross-check、registryのcurrent hash照合を確認した。WP41 API・274,683ケース／フック572／dispatcher405は変更しておらず、保存済み比較証跡の整合を維持する。3 fixtureとも停止・回収済み。

## 目的と考え方

MySQLの通常のglobal binlog位置は、別connectionのconsistent snapshotと同じ可視状態を示すとは限らない。既存の専用fixtureで、この不一致を観測している。本候補は正確なsnapshot位置を直接取得する代わりに、snapshotと重なった履歴を非公開の作業状態へ再生する。

1. snapshotを開始する前に、実行済みGTID集合Aを取得する。
2. 新しいread-only repeatable-read snapshot Bから対象の全raw行を読む。
3. 全行を読み終えた後にbinlog位置Cを取得する。
4. 保持された最初のbinlog fileのposition 4から、CRCと物理的な連続性を検査しつつ履歴を読む。Aに含まれる先頭transaction群は適用しない。
5. 最初のA外transactionを見つけたら、以後はAへの所属を問わずCまで全部再生する。INSERTは完全after行の上書き、UPDATEは旧主キーの削除と完全after行の上書き、DELETEは不在を許容する削除とする。
6. 完全なCOMMIT境界Cでraw行・索引を構成した後、通常のbefore-image照合を要求する差分適用へ切り替える。

途中の状態は公開しない。prefixを省略するdecoder状態は通常の`Advance`や連続性検査の緩和とは分離する。最初のA外transaction以降にA所属のtransactionが現れても再生するため、Aに穴がある場合に後続変更を飛ばして古い状態へ戻す問題を避ける。主キー移動・同じIDの再利用も、各キーの最後の完全な行イベントまで再生する前提で扱う。

これは限定条件から導く独自方式であり、MySQL公式のbootstrap手順ではない。根拠となるGTID公開の順序は[GTID Life Cycle](https://dev.mysql.com/doc/refman/8.4/en/replication-gtids-lifecycle.html)、完全な行イメージの設定は[binlog_row_image](https://dev.mysql.com/doc/refman/8.4/en/replication-options-binary-log.html#sysvar_binlog_row_image)を参照。Cはbinlog COMMIT prefixの論理状態であり、その瞬間のSQL可視状態、client ACK、OSの耐久状態を同一視しない。[binlogと回復](https://dev.mysql.com/doc/refman/8.4/en/binary-log.html)

## 最初の実験範囲

専用flag `python scripts/database_smoke.py --backend mysql --gtid-bootstrap-probe`で、新規に所有するMySQL 8.4.13 fixtureだけを起動する。通常のbinlog実験とは混在させない。GTID ON、enforce consistency ON、binlog order commits ON、圧縮OFF、ROW/FULL、単一origin、最初の単一file、purged集合空、安定schemaに限定する。既存DB・既存サービスの設定は変更しない。

元の`wp_postmeta`から専用表を作り、ID 0とuint64最大値を含む11行を保持する。A取得後・B開始前のINSERT、Bのページ読取り間の主キー移動、DELETEと同じIDの再INSERTを使い、BとCの差を作る。Cまでの再生結果を独立SQLの全raw行・バイナリキー索引と比較し、さらにC以降の1 transactionを通常の厳密な適用で検査する。非prefix Aへの耐性は実DBでは未実証。fresh・single-origin・AUTO・ordered commitsの正例で得られるAは通常prefixとなるため、後述の共通runtime selectorを通す純粋試験で別に確認する。

想定外のDDL、匿名・tagged GTID、別origin、rotation、必要ログの消失、source交換、設定の不一致は拒否する。RAM上の構成だけを対象とし、永続journal、製品公開、継続CDC、DB／OS障害からの回復は本実験の対象外。正式な対応版、保持・再構築方式、実メモリ予算との接続も未完了。

検証器は起動した実PID、厳密な整数／boolean、単一行のbounded JSON、Aの単一origin区間、floor→C→strictの前進、限定profile、source hashを確認する。保存先は`compatibility/evidence/mysql-overlap-bootstrap.json`。保存データだけの再検査は`python scripts/check_mysql_overlap_evidence.py`、全開発gateと専用実験は`python scripts/check.py --with-mysql-overlap`で行う。

## 専用実DBの成功記録（2026-10-27）

MySQL 8.4.13の新規所有fixtureで、GTID集合Aは単一originの1～132、先頭132 groupを適用せず通過した。`probe-bin.000001`をposition 4から連続読取りし、replay floorは2,149,380、Cは2,150,732、strict transactionの終了位置は2,151,162だった。この数値は第2回fixtureの観測値で、将来の固定期待位置にはしない。

A取得後・B開始前のT1はID100をINSERT。Bは3行ずつ4ページ、計12行のread-only RR snapshotとなる。ページ読取り間にT2で100→101へ主キーを移動し、T3で101をDELETEして別owner／key／valueの100を再INSERTしたが、BはT1後の行を維持した。非公開mapへ3 COMMITを再生したCの全raw行・binaryキー索引が独立SQLと一致し、T4のID100 UPDATEをCから通常のstrict before-image適用で1 COMMIT処理した後も12行と索引が一致した。元のWP表と通常projectionは変更していない。

`MYSQL_OVERLAP_BOOTSTRAP_RESULT`はnativeの21 fieldsに終了観測`sql_disconnect_resets`を加えた単一行JSONで、実起動PIDと照合する。第2回は`overlap_snapshot_reader: 1`を記録し、許可された終了箇所・厳密な整数値としてparserが検証した。SQL／COMMIT／読取り失敗を成功へ置き換えたものではない。所有fixtureは停止・回収済み。専用harnessのraw DB契約36項目と保存証跡checkerも成功した。

再現・再検査:

```powershell
python scripts/database_smoke.py --backend mysql --gtid-bootstrap-probe
python scripts/check_mysql_overlap_evidence.py
```

専用第2回liveの後、両normal DBとこの専用GTID fixtureを含むfull gateも成功した。`target/mysql-overlap-bootstrap-full-check.log`に全検証成功とregistry整合を記録した。通常probeへGTID ON設定を暗黙適用せず、正常終了cold restore、pre-COMMIT kill、同DB選別反映等の結果とも別に記録する。Linux向けcross-checkをnative Linux実行の合格にしない。

## decoder境界と純粋試験

全frameのCRCと物理位置連続性を確認し、最初のfile・空Previous-GTIDs・単一sourceの通常GTID順序・GTIDのtransaction lengthとgroup終端を検査する。A所属の先頭履歴を通過する状態と、first-not-A以後の完全after再生を分離している。`ReplayFloor::skip_group`は実runtimeと純粋試験で共用し、一度floorへ到達した後はA所属groupもskipしない。A={後続T2}でT1とT2の双方が再生されることを同じselector経由で試験する。これは非prefix Aの**純粋試験**で、実DBに非prefix集合を発生させた結果ではない。

FDEにはCRC32の4 bytesとは別にchecksum algorithm descriptorの1 byteがある。固定driverの`Event::read`はこれもdecoded dataから除くため、専用envelope helperはFDEだけdescriptor長を復元して検証する。他frameは従来のheader＋data＋CRC長で照合し、CRC必須と物理end位置の条件は維持した。第1回liveでこのFDE長を1 byte不足として拒否した失敗を、driver sourceと実fileのFDE長へ照合して修正した。診断には種別・位置・長さ・flags・checksum文脈を残す。第1回ログ`target/mysql-overlap-bootstrap-first-live.log`も履歴として保持する。

Cからのstrict seamは既存adapterを緩和せず、同sourceの次の通常GTIDが1件だけであること、GTID frameがCから始まること、checkedかつboundedなtransaction lengthが正確にDへ達することを追加検査する。bootstrapとstrictでgroup長helperを共用し、0・短すぎる値・上限超過・加算overflow・fence超過を純粋試験で拒否する。FDE長、runtime floor、GTID集合、非prefix countermodel、PK移動／ABAを含む当該moduleのunit6件とClippyが通過した。

## 未検証範囲と残件

この正例は、GTID公開遅延やengine未commitのXIDを意図的に作った試験ではない。Cはbinlogの論理COMMIT prefixとして扱う候補であり、任意の稼働sourceのSQL可視状態や耐久状態と一致すると証明したものではない。公式根拠のsource調査は8.4ブランチを含み、8.4.13タグの全実装監査とは区別する。

現profileではDDL、匿名／tagged GTID、別origin、rotation、非空Previous-GTIDs、purge、設定不一致等をfail closedとする。新modeでそれら全ての実DB拒否ケースを注入したわけではない。長時間snapshot、ログ保持競合、手動GTID、非logged変更、source交換・再起動、任意schemaや主キー形式、永続bootstrap journal、公開切替・連続CDC・メモリ予算との接続は未完了。prefix以前のDDLをAの既commit履歴として通過することを、通常のunknown／DDL classifierの緩和へ流用しない。

FTWRLを公開する本体APIの自動承認レビュー拒否は保留のままで、本実験はその再試行や別名でのserver-wide write／commit pauseを実行していない。正式な製品方式・版・権限方針の新採用ではない。
