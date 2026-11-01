# MySQL実binlog切替の非公開RAM候補

最新の専用live（2026-10-27、日本時間）：`target/mysql-rotation-owned-cut-second-live-check.log` が成功した。同じ稼働中の所有MySQL fixtureで、blocking dumpを実Rotateの検証直後に終了させ、旧C1を保ったまま別接続から再試行した。DB契約39項目と新結果6項目を含む厳密39 fieldsが通過した。続くtarget/byte-entity-decode-and-rotation-retry-native-check.logで独立Rust27件・Python112件・既存4 fixture各39項目と全current provenanceの検査が成功した。workspace Rust319件、Clippy、Linux cross-check、HTTP、license観測と固定PHP55 API・366,019ケース／hooks572／dispatcher405も別の先行gateで成功。専用liveと既存4回の合計5 fixtureを停止・回収し、db-runsが空であることを確認した。

再試行の初回liveはtarget/mysql-rotation-owned-cut-first-live-check.logで初期FDE検査に失敗した。接続終了・ID不在確認後、再接続のFDEはend0／size123／flags0／checksumありで届いた。物理ファイルのend127を初期通知にも期待していたためであり、初期通知のvalidatorだけを実測end0へ限定して修正した。この初回の試験全体は失敗として保持し、物理bridgeのCRC・位置連続性は変更していない。失敗fixtureは所有process0確認後、server.logを保存して専用ディレクトリだけ回収済み。

2026-10-27時点。固定MySQL8.4.13、GTID OFF、ROW／FULL、CRC32、transaction compression OFFの専用使い捨てfixtureで、実際のbinlog切替を検証した。**人工Rotate用phaseの修正後、独立した専用live試験と、既存4 fixture・全体検査のcombined gateが成功した。** 既存の1ファイルstrict decoder、core、Positionは変更していない。

## 実験の境界

`experiments/binlog-probe/src/mysql_rotation_handoff.rs` は通常decoderから独立した有限の切替候補である。専用SQL cloneの11行を扱い、T1のCOMMIT後に試験制御接続だけが `FLUSH BINARY LOGS` を一度実行する。既存のWP12表と通常projectionは変更しない。RELOAD権限は新しい所有fixtureの専用制御ユーザーに限定し、本番CDC readerや製品の権限方針を決定しない。FTWRLやserver全体の書込み停止は追加しない。

通常Positionの比較は同一session・同一ファイル内の契約を維持する。候補の外側にepoch／file／offsetを持つ別checkpointを置き、file1のT1位置C1から、切替headerだけでは進めない。別ファイルのoffsetが偶然同じでも、通常Positionの同一性へ流用しない。

現在の専用liveで実施した有限手順は次のとおり。

1. file1の開始位置からblocking dumpと通常strict decoderでT1を適用し、C1を保存する。dump接続のhandshake IDとSQLの`CONNECTION_ID()`は変換前に一致を確認する。
2. 同じstreamの実RotateをCRC・宛先・物理位置・phaseで検証し、driverの旧TableMap消去も確認する。制御接続とは異なる同accountのIDで、`SHOW PROCESSLIST`のCommandが`Binlog Dump`であることを確認してから、そのIDだけに`KILL CONNECTION`を実行する。
3. controller自身の行が引き続き可視であることを確認しながら、2秒のdeadline内で対象IDが不在となるまで待つ。以後は旧streamを一度も読まず破棄する。旧view・索引・file付きC1 checkpoint・全pool予約を保持し、試行内のBridge・descriptor・driver TableMap・raw状態を捨てる。
4. 同じ制御接続のsource/profileと保持2ファイルを再照合し、新IDのstreamをfile1 C1から開始する。初期の人工RotateとFDEを別preambleとして厳密検証してから、freshなBridgeで実Rotate・次file人工通知・物理FDE・空PreviousGTIDsを改めて検証する。T1や`FLUSH BINARY LOGS`は繰り返さない。
5. file2のheader末尾H2から新しいstreamと通常strict decoderを開始する。T2の成功callback内だけで、RAM viewと外側checkpointをfile2のC2へ入れ替える。
6. C2から再接続し、T3を通常strictで適用する。各COMMIT後にraw行とbinary SQL検索に対する索引を比較する。

これは一つのChangeBufferがファイルをまたぐ実装ではない。journal、再起動・source交換の識別、保持期間競合、任意schema、連続CDC、公開・cache失効、本番rotation/bootstrapは未完成である。

## 最初の失敗と診断

`target/mysql-rotation-handoff-first-live-check.log` の初回は通常decoderのrotation拒否結果を調べる箇所で失敗した。診断情報を追加して別のfresh fixtureを実行したところ、`target/mysql-rotation-handoff-diagnostic-live-check.log` ではevent2に `type=4 / end=0 / size=47 / flags=32` の人工Rotateが届いた。先行する実Rotateで既にfaultとなった通常decoderへ再び渡したため、二度目のエラーがprior-fault拒否へ変わっていた。checkpointはC1=4247950を維持していた。

物理ファイルから採取したmetadataでは、実RotateはC1=4247950からEOF=4247997までの47bytes、flags0。file2のFDEはstart4／size123／end127／flags0、空PreviousGTIDsはstart127／size31／end158／flags128だった。人工通知は物理ファイルには保存されていない。これらは当該診断実行だけの観測値である。

修正では、通常decoderで実Rotateを一度だけ拒否することを確認し、その後の人工通知を候補側の専用phaseで厳密検証するようにした。人工通知は実Rotateの直後、実FDEの前に1回だけ要求し、宛先`probe-bin.000002`、payload位置4、header end位置0、raw flags32、envelope47bytes、CRC32を照合する。この通知で物理cursorの4も外側checkpointのC1も進めない。欠落、重複、別phase、別file、位置・flags・長さの不一致を具体的なfaultとして拒否する。実FDEと空PreviousGTIDsは、その後に新ファイルの物理位置として検証する。

未知eventの黙認や通常decoderの制約緩和は行っていない。初回・診断の失敗fixtureは、絶対パスと所有DB process不在を確認してからログを保存し、専用ディレクトリだけを回収した。診断用metadataはtarget配下に保存し、SQL payloadや認証情報は採取していない。

## 以前の人工phase専用liveとcombined gate（履歴）

`target/mysql-rotation-handoff-artificial-phase-live-check.log`で、修正後の専用fixtureが成功した。raw DB契約39項目に加え、file1のA=4247536／C1=4247950／EOF=4247997、file2のH2=158／C2=575／T3終端D=994を記録した。位置の大小はファイル内だけで比較する。file2のoffsetがfile1より小さくても後退とは判定しない。

実Rotate1件、人工Rotate1件、実FDE1件、3 COMMITと1回のouter RAM handoffを確認した。raw全11行とbinary SQL検索に対する索引が一致し、3回のcallbackで行・索引・raw変更bufferの論理予約を同じpoolに保持した。旧WP行は不変で、最終owner破棄後の共有予約は0だった。通常decoderによる実rotation拒否と、metadataのみでは外側checkpointを進めない条件も成功結果に含まれる。

当時の結果はnative32 fieldsに`sql_disconnect_resets`を加えた**厳密33 fields**で、`artificial_rotate_events: 1`を実Rotate件数と分けて記録する。実起動PID35500とattempt nonceを照合し、終了時の`control_final: 1`だけを既知のTLS切断resetとして受理した。SQL読取り・更新・COMMITやstream失敗をcleanup観測で成功扱いしたものではない。証跡は`compatibility/evidence/mysql-rotation-handoff.json`。

専用phaseを含むnativeのfocused4契約、Clippy、結果checkerのPython8契約が事前に通過した。この段階のisolated25契約・通常4 fixture・全体検査は後述のcombined gateで成功した。これらは接続終了・再試行を追加する前の履歴である。

## 予算と拒否試験

候補のview行、索引、保持raw row framesだけを既存の一process論理poolへ接続する。各callbackで3分類が同時に有効であることと、最終owner破棄後の返却を確認する。driver／入力／比較コピー／位置metadata・allocator overhead・peak／RSS・cross-process予算は対象外。

実Rotate／FDEのコピーへCRC破壊を加えた拒否、同じprivate verifierへの未知file・header欠落・open transactionの明示注入、typed apply cap0、pool0を個別に検査し、理由と旧行・索引・外側checkpointの保持を確認した。generic errorだけで負例合格としない。metadata順序と人工通知の正負unit、通常adapterの二重Rotate拒否unitは、実wireでの確認と区別する。サーバーが実際にtransaction途中でrotateしたり、未知fileを送信したりした実証ではない。

## 再実行と証跡

専用live試験は `python scripts/database_smoke.py --backend mysql --mysql-rotation-handoff-probe`。成功後の静的検査は `python scripts/check_mysql_rotation_handoff_evidence.py`。全gateへ含めるoptionは `scripts/check.py --with-mysql-rotation-handoff`。

結果marker、実際に起動した子PID、attempt nonce、固定DB版・TLS条件、file別位置、厳密field集合、tracked source hashを結び付ける。sourceやharnessを変更した場合は以前のhashを書き換えて追認せず、対応する実fixtureを再実行する。通常MariaDB／MySQL、MySQL GTID overlap、MariaDB snapshot bootstrapとは別の5番目のfixtureである。

`target/unicode-entities-and-mysql-rotation-full-check.log` でworkspace Rust313件（通常ignore1）＋独立experiment25件・Python111件、既存4 fixture各39項目、HTTP・Clippy・Linux cross-check・license208観測が成功。別の専用切替liveを含む全5 fixtureは回収済み。専用切替を全gate内で重複起動せず、source freeze後の実live証跡を `target/unicode-entities-rotation-final-provenance-check.log` で他のnative／WPと共に最終照合した。固定PHP比較54 API・358,373ケース／hooks572／dispatcher405は別の正式比較ログで成功している。Linux native実行や本番採用を示すものではない。

## 所有接続終了・再試行の専用live成功

`target/mysql-rotation-owned-cut-second-live-check.log`ではDB契約39項目が成功し、file1のA=4247539／C1=4247953／EOF=4248000、file2のH2=158／C2=575／T3終端D=994を記録した。実起動PID35740とattempt nonceを照合し、終了時の既知SQL resetは`control_final: 1`だった。全体gateの残stageは実行中で、このliveの成功に合算しない。

結果は従来33項目に次の6項目を加えた厳密39 fieldsである。checkerはboolと整数を区別し、欠落・余分な項目や再接続回数の違いを拒否する。

| 項目 | 必須値 |
| --- | --- |
| `owned_dump_terminated` | `true` |
| `bridge_reconnects` | `1` |
| `retry_checkpoint_preserved` | `true` |
| `retry_preamble_verified` | `true` |
| `same_owned_server_verified` | `true` |
| `rotation_event_count_scope` | `"successful_bridge_only"` |

`real_rotate_events: 1`／`artificial_rotate_events: 1`／`real_fde_events: 1`は、成功した再試行のBridge内だけを数える。終了した最初の試行の実Rotateや、再接続時の初期preambleを加算しない。T1～T3の共有pool callbackは3回であり、raw全11行・索引の一致、旧WP行不変、最終owner破棄後の共有予約0も通過した。

初期preambleの人工Rotateは、file1名・payload位置C1・header end0・raw flags32・size43・checksum無しを正確に要求する。これはCAと名前を検証したTLS上のtransport通知であり、CRC検証済みの物理Rotateへ数えない。続く初期FDEはend0／size123／flags0、binlog v4／header19、CRC32とenvelopeを要求する。物理fileのFDE end127とは明確に区別する。初回失敗を受けたpure unitは、初期FDEのend0受理・end127拒否を検証する。次fileの人工Rotateは従来どおりsize47・CRC32必須であり、物理bridgeの連続性や通常adapterの拒否規則を緩めていない。

locked mysql_asyncのclose結果やDropだけを終了の証拠にしない。切断対象は自然EOFを避けるblocking dumpであり、同accountのlive ID確認、指定IDへのKILL、controller可視性を維持した不在確認を実際に行った。受信済みbufferにはeventが残り得るため、KILL後は旧streamを読まず破棄する。「readが切断errorを返した」という観測ではない。

再試行前後の同一性確認は、同じ稼働中の所有fixture内で、制御接続ID・account・version・正規化datadir・port・server UUID・GTID/ROW/FULL/CRC/compression profileが一致し、file1/file2のサイズが期待どおり保持されていることに限定する。`same_owned_server_verified`は再起動・clone・source交換に耐えるepoch証明ではなく、保持期限競合や再起動後の復旧方式も未対応である。不一致やファイル欠落を別の位置への飛越で回避しない。journal・durable handoff・本番再接続方式は採用していない。

MySQL8.4では追加のCONNECTION_ADMIN／SUPERなしでも自身のthreadは終了可能だが、KILLの戻りは終了完了を待たない。そのため試行IDの不在確認まで必要になる。[公式KILL仕様](https://dev.mysql.com/doc/refman/8.4/en/kill.html)。nonanonymous accountはPROCESSなしでも自身のthreadを観測できる。[公式SHOW PROCESSLIST仕様](https://dev.mysql.com/doc/refman/8.4/en/show-processlist.html)。他account・既存DB・server／OS processを終了する試験、追加grants、FTWRLやglobal write pauseは含めない。

接続ID・preambleの正負を含むnative focused6契約、fmt／Clippy、39 fieldsに同期したPython8契約は通過済みである。native全体27契約と既存4 fixtureを含む残gateも、冒頭のログで成功した。
