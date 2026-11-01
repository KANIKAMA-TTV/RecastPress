# MySQL COMMIT前server終了・同datadir復元の限定候補

2026-10-27（日本時間）。実装・対象試験・初回の専用liveと全gateが成功した。MySQL8.4.13のfresh owned fixtureだけを対象とし、既存DB・OS・サービス・電源へ操作しない。

`target/protocol-outer-and-mysql-server-restart-full-check.log`でworkspace389／独立29 Rust・Python133、全6 fixture各39項目、Clippy・Linux cross-check・license208観測・実HTTPを確認した。cleanup補修後も専用再起動検査が成功し、最終観測はC1=4,281,650／候補=4,282,091、旧DB PID32180／新DB PID25732、旧DBとpending childの終了codeはともに1。最終serverログは`target/development-logs/mysql-server-restart-04e6ac0f7f6ab53db7f8ad9b4047720c.log`、全所有processとdirectoryを回収した。WP current hashも別途確認し、60 API／401,980比較・hooks572／dispatcher405を維持する。

`target/mysql-server-restart-first-live-check.log`で39 baseline項目と検査専用20 fieldsを確認した。C1=4,281,654／未COMMIT候補=4,282,095で、11行・全old tuple・再構築index・source T2の保持と候補非採用が成功した。両serverと全childを回収し、専用directoryも削除した。`target/development-logs/mysql-server-restart-3dbd5f3c63449c97833eedf7b299acdd.log`に実serverのStarting crash recovery、uncommitted transactionsのrollback開始・完了を保存した。ログは補助観測であり、成功判定はSQLの全tuple照合とrow lock取得に基づく。結果は`compatibility/evidence/mysql-server-restart.json`へ実測から生成した。

## 検証する境界

正常producerが終了した後のcold restore、稼働DBを保ったclientのCOMMIT前kill、今回のDB process終了・同datadir再起動は別の試験である。既存normal probeや通常decoderの契約を変更しない。snapshot anchorを得るためのwrite pauseや再起動には使用せず、FTWRL・global write pause・追加grantsを導入しない。

初回起動だけinitialize、bootstrap SQL、WP参照schemaを適用する。専用server.iniにinit-fileを残さず、最初のcommandだけへbootstrap引数を渡す。再起動は検査した共通4引数commandに限定し、initialize・init-file・schema・producer・prepareを再実行しない。init_fileはstartup時にSQLを実行するため、同じ設定ファイルを無条件で再利用しない。[MySQL init_file](https://dev.mysql.com/doc/refman/8.4/en/server-system-variables.html#sysvar_init_file)

同じ版、正規化datadir、port、server UUID、GTID OFF／ROW FULL／CRC32／compression OFF、log_bin=1、sync_binlog=1／innodb_flush_log_at_trx_commit=1／innodb_force_recovery=0を前後で照合する。共通iniとTLS3ファイルのhashも照合し、証明書を再生成しない。このUUID一致は同じ所有fixtureの有限な観測であり、cloneや交換に耐える製品source identityではない。[binlogのdurability設定](https://dev.mysql.com/doc/refman/8.4/en/replication-options-binary-log.html#sysvar_sync_binlog)

## READYと所有processの順序

通常producer→別processの通常restoreで保存projectionを確認し、既存prepare modeが専用11行cloneとjournalを準備する。cloneは元wp_postmetaを変更せず、ID0／MAXを保持する。sourceのT1とT2は両方COMMIT済み、journalのC1をCOMMITしてからC2候補を別SQL transactionへ書き、own SELECTで全tupleを照合する。READYにはnonce、実child PID、保持SQL connection ID、epoch、file、位置、完全old tupleをbounded canonical JSONで出す。READY後はpending awaitを保持し、COMMITや正常成功経路へ進まない。

親は既存bounded readerでREADYを検証し、不正markerやtimeoutではDB killを行わない。guard後に、捕捉した所有DBのPopen handleへkillを要求し、非zero終了をwaitで確認してからpending childをkill/reapする。出力中のPIDやconnection IDをkill対象にはしない。DB kill後にclientがread errorを返したことや自発終了したことは合格条件にしない。

停止確認後、同じdatadirの新Popenを起動して前述のguardを再実施する。PID fileは各Popenの実PIDと一致させる。証跡checkerはproducer、before restore、READY、verify、confirmation、旧DB、新DBの7 PIDを互いに異なる値として要求する。この有限fixture上の拒否規則のため、OSが短時間にPIDを再利用した正しい実行も拒否され得る。PID値の永久一意性は主張しない。

## 検査専用mode

`--verify-postmeta-server-restart`は既存の所有datadir／版／TLS guard後、stdinの既存READYと`RECASTPRESS_SERVER_RESTART_UUID`を照合する。正常終了の`--restore-postmeta`や、S1→S2をbinlogから再生する`--resume-postmeta-crash`へ置き換えない。

専用singletonをSELECT FOR UPDATEで取得し、epoch／file／from／through／receipt／row_dataの全bytesをold tupleへ照合してROLLBACKする。session lock timeout2秒と5秒deadlineを設ける。接続可能になったことだけではbackground rollbackの完了は示せないため、対象row lock取得で未COMMIT transactionのdrainを待つ。[InnoDB crash recovery](https://dev.mysql.com/doc/refman/8.4/en/innodb-recovery.html)

C1の保存行はFIRST、正本cloneはCOMMIT済みT2のSECONDである。保存全行を現在のsource全行へ単純一致させない。保存行とmarkerのC1、source全11行とC1のid3 valueだけSECONDへ変えた期待値、C1から再構築したindexと更新されていないkeyに対する独立SQL結果を別々に照合する。検査後のjournal全tuple不変も必須とする。opaque receiptは非空の既存bounded shapeと完全bytesを確認するもので、保存payloadからwire CRCやsource真正性を再検証したとはしない。

検査専用modeではprojection DML／COMMIT、DDL、binlog購読を行わない。SQL transactionはrow lock取得のためread-write transactionとして始め、ROLLBACKで閉じる。「SQL READ ONLY transactionを使う」という意味ではない。通常wp_postmetaと通常projectionの保存値は、その後の別fresh processの通常restoreで確認する。

## 結果・失敗・証跡

結果markerは`MYSQL_SERVER_RESTART_RESULT=`、厳密20項目。profileは`mysql_8_4_13_unpublished_server_precommit_restart_restore`、実PID／prepared PID／nonce／source UUID／file／C1／candidate位置／11行に加え、old tuple、候補非採用、rollback drain、baseline行、index、source COMMIT保持、store不変、receipt shape、projection書込み無しを要求する。SQL終了resetは空またはcontrol_final:1だけを別観測とし、接続・read・ROLLBACK・その他SQL失敗を成功へ変換しない。

親の別記録は、前後の通常restore raw結果、bounded READY、全22 lifecycle項目、実Popen PID／非zero exit、同じini／datadir／port／TLS／UUID／設定、初回initialize／bootstrap各1回と再実行無し、tracked source hashesへ結び付ける。全raw protocol lineの長さ、改行終端、余分な行・重複・未知項目、boolと整数型の違いをcheckerで検査する。設計・結果形式の予定は本番storage protocolではない。

全captured server handleを逆順にreapし、1handleのcleanupが失敗しても他handleと別のwait試行へ進む。元の例外へcleanup失敗を併記し、READY reader停止、子pipe回収、log閉鎖を行う。所有者停止を確認できない場合はデータを削除しない。失敗fixtureはexact専用directoryをretainし、既存サービスや別PIDへ対象を広げない。成功記録やdirectory削除は所有handleの回収後に行う。

初回live後の独立レビューで、子のkill失敗時にPopen contextの終了処理が無期限waitへ進み得る既存経路を検出した。fixture子の起動はcontext／communicateへ依存せず、明示handle・nonblocking pipe worker・期限付きwait／EOF確認へ変更した。未回収handleはstrong registryへ残し、外側finallyがDB停止後にreap_owned_probesで再試行する。無関係な呼出元exceptの例外をcleanup元エラーと誤認しないよう、各実行の失敗をlocal変数から明示的に渡す。262KiBの双方向入出力、stdin非消費、reader失敗、KeyboardInterrupt、kill拒否・再試行、ambient exceptの対象試験と独立レビューを通過した。これらはprocess helperの契約試験であり、追加のDB crash実証として数えない。

純粋なhelper/lifecycle試験とnative/parserの試験は実DB crash recoveryの実証と区別する。専用liveも同じ限定profileの有限観測である。COMMIT発行中の曖昧な結果、OS／電源断、disk flush保証、server再起動後のbinlog再接続・rotation／retention、任意schema、continuous CDC、public世代の更新・撤回、backup、製品のcrash耐久保証は対象外である。
