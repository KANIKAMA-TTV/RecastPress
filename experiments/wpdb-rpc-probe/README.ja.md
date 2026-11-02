# wpdb → Rust RPC のdriver候補と実DB・native基準試験

2026-11-02。P0の接続候補1を小さく実証するための準備であり、P1の実装完了、標準wpdb互換の採用、製品driverや公開ABIの決定ではない。既存のediting側SQLx経路は変更しない。

[driver-plan.json](driver-plan.json) と [mysql_async用patch案](patches/0001-mysql-async-wpdb-handshake.patch) は、patch案を作成した時点の準備状態を固定する。planの未適用・未コンパイルというstatusとfalseの実証範囲は書き換えない。その後のhelperによる所有stagingへの適用・依存補充・2 unitの実測は、専用evidenceで別に記録する。元registry source、root/binlogのCargo manifest/lock、editing側の依存は変更しない。準備helperとwire buildはDBを起動・接続せず、実接続は専用の使い捨てDB harnessへ分ける。

## 固定する元sourceと証拠の範囲

既存 [binlog-probe/Cargo.toml](../binlog-probe/Cargo.toml) と [Cargo.lock](../binlog-probe/Cargo.lock) の `mysql_async 0.37.1`、`mysql_common 0.37.3` を参照する。ローカルcrate archiveの全byteのSHA-256をこのlockのregistry checksumと照合し、それぞれ63件・85件のarchive内通常ファイルをローカルcacheのsourceとbyte完全一致で照合した。archiveは読取りだけで、展開先を作らず、コードを実行していない。Cargo cache管理用の `.cargo-ok` 等はsource照合から明示的に除外する。

planにarchive名・byte数・SHA、既存lock自身のSHA、source file inventoryのcanonical SHA、patch自身のSHA、対象2ファイルの全体before/after byte数・SHAを記録する。63通常ファイル全体についても、対象2ファイルだけを置換した後の期待inventory SHAを固定し、他ファイルの変更を認めない。ローカルcacheの絶対pathや資格情報は記録しない。package内のVCS revisionはpackageが宣言する情報として記録しただけで、Git treeとの独立照合や完全なsource attestationではない。依存全体のlicense/notice確認も未完了。

## 2ファイルに限るpatch案

| 対象 | 変更 | 既存挙動との境界 |
| --- | --- | --- |
| `mysql_async/src/opts/mod.rs` | `Opts` のgetterと `OptsBuilder::multi_statements(bool)` / `local_files(bool)` を追加し、実handshakeを作るcapability maskへ反映 | 元の2能力の既定値はtrueを維持。既存 `client_found_rows` はfalseのまま。新URL parameterは追加しない |
| `mysql_async/src/conn/mod.rs` | `HandshakePacket::server_version_ref()` のbytesを保持する `Conn::raw_server_version()`、送信する最終HandshakeResponseの標準32-bit maskを返す `negotiated_capabilities()` を追加 | parsed version tuple、認証/TLS、既存protocol用maskを変更しない。型から全DB機能の成立を推測しない |

最初の候補は既存API `client_found_rows(false)` と追加案の `multi_statements(false)`、`local_files(false)` を明示する。DB serverとの能力intersectionと最終serialized maskの双方で、FOUND_ROWS / MULTI_STATEMENTS / LOCAL_FILES がoffであることを将来のwire試験で確認する。最終maskを別に保持するのは、HandshakeResponseの組立時にCONNECT_ATTRS等が調整され得るためである。MariaDB拡張capability wordや機能の意味までこのgetterで検証したとはしない。

MULTI_RESULTSはMULTI_STATEMENTSと別なので一括で無効化しない。local infile handlerやconnection attributesを設定せず、接続先・TLS・socket利用はhostが所有する。最初は `prefer_socket(false)` と固定したtransportで比較し、暗黙のsocket切替を入れない。2 unitは既定maskの保持と選択した3bitだけの除去を調べるために含めた。実handshake・DB結果の一致を示すunitではない。

## 既存public APIで通す縦断

SQLは `AsQuery` の `&[u8]` / `Vec<u8>` を `Queryable::query_iter()` へ渡し、COM_QUERY / TextProtocolを使う。SQLをUTF-8へ変換せず、prepared/binary routeを混ぜない。上流のquery tracingにはlossyな表示経路があるため、将来の候補ではtracing featureとSQL本文logを有効にしない。driver errorの表示を通常のSQL応答へ付け替えない。

行値は未消費slotを位置順に調べ、TextProtocolの `Value::NULL` をSQL NULL、`Value::Bytes` をそのままの非NULL bytesとして運ぶ。`Row::as_ref(index) == None` や `unwrap_raw()` 外側のNoneは「take済み/存在しないslot」であり、SQL NULLと見なさず異常にする。他のValue型が来たらbinary route等の前提違反として拒否する。空文字、NUL、0xff、数値の文字列表現、大きいunsignedやdecimalをJSON numberやRust Stringへ丸めない。

`QueryResult::columns()` / `columns_ref()` をrow消費前に取得して、0行でも列情報を失わないようにする。Columnの `*_ref()`、flags、character_set、column_length、decimals等を使い、同名列・列順も保持する。wire上のdeclared column_lengthとmysqliの `max_length` 等は同一と決めつけない。PHPの `get_col_info()` との対応は実比較で確定する。

接続は要求ごとに専有し、poolへ返さない。pending resultのdrainまたは失敗による再利用不可を確認し、`Conn::disconnect()` をawaitして次要求は新しい物理接続から始める。既存 `reset()` は最初の候補では使わない。Dropには背景清掃の経路があるため、Dropだけで切断・rollback・全回収が確認できたとは扱わない。失敗・timeout時も書込みを黙って再送せず、COMMITの結果不明をrollback済みや成功へ変更しない。

## PHP adapterとの責務境界

元のWP 7.1.3 `require_wp_db()` は `class-wpdb.php` の後に所有 `wp-content/db.php` を読み、drop-inが `$wpdb` を生成すれば既定接続を生成しない。この入口を使い、上流の `prepare()` / `get_var()` / `get_row()` / `get_col()` / `get_results()` / CRUD helper / prefix処理等をPHPに残す。

`wpdb::_do_query()` はprivateであり、それだけをsubclassでoverrideする案では動かない。`query()` 自身にもmysqliのerror・affected・insertid・fetch依存がある。最小PHP adapterで接続/query/_real_escape/charset・SQL mode/select/check_connection/close/db_server_info/load_col_infoとprint_error/bailのdriver分岐を対応させ、標準の戻り値・公開状態・query filter・error抑制・prefix・`num_queries`等を比較する。driver固有handle/resultをpluginへ公開しない。

escapeは `prepare()` が返すSQL文字列と実sessionのcharset/SQL mode双方に関係する。単なるaddslashes、全SQLのprepared化、結果件数の補正、SQLの正規表現による複数文判定、lossy変換で互換差を隠さない。PHP adapter、要求専有RPC、DB driverを別の契約として検証する。P0の固定 `DB_SELECT_FIXTURE` 成功を実DBや標準wpdbの成功として再利用しない。

## 次に行う小試験

1. 別のfresh所有source copyで原checksum/before確認→patch適用→全after確認→compileと2 pure unit。元registry cache、binlog実験、editing側SQLxは変更しない。
2. 有限の所有harnessでMySQL/MariaDBの実handshakeを確認。非TLS/TLSで3能力off、他能力の保持、raw server versionを独立したnative接続と比較する。
3. 同じ使い捨てschemaのnative mysqli/実wpdbと、候補のtext queryを比較する。同値UPDATEのaffected=0、変更UPDATE、insertid、0行列情報、NULL/empty/NUL/0xff/decimal/unsigned、SQL bytes、DB errorを含める。DDLや資格情報はharness/hostのみが所有する。
4. 要求内のBEGIN/rollback/session変数・切断と次要求のfresh stateを確認する。その後、所有db.phpを通す同じ固定PHP corpusで、prepare/escape、全get mode、公開状態・query数・charset・能力照会を比較する。

この段階で未成立の契約を省略して標準wpdb互換と呼ばず、候補の停止・見直し条件として記録する。full WP bootstrap、実サイト操作、永続cache、製品ABI、製品driverの採用は後続である。

## 所有sourceの準備と2 unitの監督

`prepare_driver.py` は固定archive・既存lock・原sourceと全inventoryを再照合し、ignored `target/wpdb-rpc-probe/workspace` だけへ2crateをコピーする。patchの全contextを元行へ厳密に照合し、再構成したunified diff自体が固定patchと完全一致すること、全63/85ファイルと2変更ファイルのwhole after SHAを検査する。既存stagingはsource・manifest・seed/resolved lockが完全一致するときだけ再利用し、不一致を消去・上書きして合格にしない。

`driver_check.py --record` は既存binlog lockを独立workspaceのseedとし、offlineで依存解決を試みる。その後は `--locked --offline` のCargo compile（1200秒）、実unit executableの2件列挙（10秒）、2 unitだけの実行（30秒）を所有childとして監督する。0件、想定外のunit、ignored、timeout、清掃未確認は成功にしない。成果はJSONだけで、driver binary/source stagingはignored targetに置く。

初回offline解決は、元manifestのoptional `hdrhistogram` がcache/indexにないためコンパイル前に失敗した。ベンチ用依存の削除やfeature/期待SHAの変更では迂回しない。明示した `--resolve-with-network` は、元crate内Cargo.lockの版/checksumを優先して公開registryの検証依存を補充するための入口である。生成するlockは独立staging内だけとし、確定SHAと追加package checksumを記録し、compile/列挙/unitはその後もlocked・offlineにする。初回失敗JSONを別に保持し、成功記録で消さない。

## 追加で塞がるraw ERRの公開境界

固定sourceの `mysql_async::Error::Server` はmessage/stateをStringで持ち、`error/mod.rs` の変換は `packet.message_str()` と `SqlState::as_str()` を使う。`mysql_common` 側には `ServerError::message_ref()` と `SqlState::as_bytes()` が公開されているが、driver内の元packetは `ConnInner::last_err_packet` へ非公開で保持され、現2ファイルpatchの公開APIから取得できない。非UTF-8のERR本文を公開Errorから復元できるとは扱わない。次の縦断までに、raw packet snapshotを公開する追加の小patchまたは同等の無劣化経路を検討・実測する。現在のStringをbytesへ戻す、lossy文字列を期待値へ採用する、通常のSQL errorとtransport失敗を同一視する方法では補わない。

## 今回の限定実測

2026-11-02、[元のoffline依存不足](evidence/windows-offline-missing-hdrhistogram.json) を保持したうえで、[依存補充後の初回2 unit実測](evidence/windows-network-refill-driver-units.json)、[最終helperでのlocked・offline再実測](evidence/windows-driver-units.json) を分けて保存した。前2記録は各実行時点のhelper hashを持つ原記録であり、最終helperの入力hashへ書き換えない。初回positiveの `additional_cached_registry_packages` はlock追加行の一覧で、全optional package archiveの取得を証明する項目ではなかったため、最終helperでは `additional_resolved_registry_packages` とし、compile/testのoffline設定も独立して明示した。

元crate内lockの `hdrhistogram 7.6.0` / checksumを維持し、独立lockは60,763 byte、SHA-256 `5a9fa8a5ce89f1abc15be8594eac30d2cdde2c79702c3cd8ad5242bdc3b49cb0`。追加解決99行を含むregistry package 253件のchecksumをseedまたはreceiptで固定した。feature無効のoptional依存もlockに含まれるため、253件すべてをコンパイル・取得したとはしない。

両positive実測で、release test binaryを実際に生成し、新規2 testだけを列挙して `2 passed / 0 failed / 0 ignored` を確認した。実行前後の63/85全source inventory、元registry archive/source、元root/binlog lock、最終実測のhelper入力の不変と所有childのreap/cleanupを確認した。成果binaryはignored target内に留める。この2 unitの記録だけではraw handshake・SQL bytes・実DB・native wpdb比較・製品採用は未実証であり、後述の実DB試験を別に行った。

## 実接続用の固定wire probe

[独立Cargo workspace](Cargo.toml) と [固定SQL corpus](src/main.rs) を追加した。接続先はloopback・固定schema/userに限り、毎試験の資格情報はstdinで受け取る。SQLや接続URLを外部から指定する入口は設けない。driverのtext protocolへSQL bytesを渡し、列名・列順・値をhexとSQL NULLで保持する。試験は60秒、接続ごとに明示disconnectし、次要求のfresh sessionを調べる。

[wire_check.py](wire_check.py) は元driverの全source確認後に `cargo build --locked --offline` を監督し、Cargoが報告した実executableのpath・SHAとsource/lock/helperの対応を記録する。[Windows build記録](evidence/windows-wire-build.json)では追加fetchなしでbuildが成功し、不正な接続先・secret形式の2件を接続前に拒否した。実接続harnessはこの記録・全staged source・実executableを再照合してから使う。build成功を実handshakeやwpdb互換の根拠にはしない。

## 所有DBでの実測結果

[database_check.py](database_check.py) は固定したMySQL 8.4.13／MariaDB 11.8.9のZIP全体・全通常ファイル・manifest・配置済みファイル集合を照合する。MySQLは固定公開鍵のfingerprintと切離し署名もofflineで再検証し、MariaDBは固定SHAによる照合として記録する。新しいrunごとにdatadir、port、48桁の管理用／制限user用secret、CAとIP SAN付き証明書を作る。バイナリ・秘密・設定・ログはignored targetだけに保持する。

管理userは捕捉したserverがprivate init-fileで作成し、管理接続は常にCA・接続名検証付きTLSを使う。固定版・datadir・port・pidfileと捕捉したPopen PIDの一致を確認してから、clientによるfixtureのschema／table／制限userを作る。制限userへ与えるのは専用schemaのSELECT/INSERT/UPDATE/DELETEだけで、TLS要求profileではREQUIRE SSLも使う。非TLSのMySQL比較CLIは所有serverからRSA公開鍵を取得する。この補助接続後の試験なので、初回完全認証・RSA鍵配布の安全性を製品向けに受入れたとはしない。

| 固定profile | wire corpus | 終了 |
| --- | --- | --- |
| [MySQL・非TLS](evidence/windows-mysql-disabled-wire-database.json) | 28ケース通過 | 管理shutdown、server exit 0、全child reap |
| [MySQL・TLS](evidence/windows-mysql-required-wire-database.json) | 28ケース通過 | 同上 |
| [MariaDB・非TLS](evidence/windows-mariadb-disabled-wire-database.json) | 28ケース通過 | 同上 |
| [MariaDB・TLS](evidence/windows-mariadb-required-wire-database.json) | 28ケース通過 | 同上 |

両物理接続で実serialized maskのFOUND_ROWS／MULTI_STATEMENTS／LOCAL_FILES off、TLS bitと実Ssl_cipher、raw server versionを確認した。SQLの非UTF-8 byte、非UTF-8列名、同名列と列順、0行の列metadata、NULL／empty／NUL・0xff／decimal／unsigned最大値、insert_id、同値UPDATE=0と変更UPDATE=1、1064/42000後の回復を照合した。明示rollbackに加え、未commitのままdisconnectした後の新しい接続でsession変数NULL、元row lockの取得、元値への復帰を確認した。試験後のDB内容は独立した管理接続でも調べた。

初回の[配布物サイズ上限](evidence/windows-mysql-disabled-tool-quota-failure.json)、[GPG import](evidence/windows-mysql-disabled-key-import-failure.json)、[初期root接続](evidence/windows-mysql-disabled-bootstrap-identity-failure.json)、[管理接続の認証](evidence/windows-mysql-disabled-bootstrap-authentication-failure.json)の失敗を保持した。604,549,120-byteの固定PDBもarchiveの2GiB上限内で全体照合し、公開鍵はagentを起動せずfresh keyringへdearmorする。rootへのTCP接続に頼らず、管理アカウントを所有serverでbootstrapする。管理接続をTLSへ固定した後も、wire corpusの非TLS条件は維持した。

MariaDBの管理CLIはsandbox内で[非TLS profile](evidence/windows-mariadb-disabled-schannel-failure.json)・[TLS profile](evidence/windows-mariadb-required-schannel-failure.json)ともSchannelのSEC_E_NO_CREDENTIALSで失敗した。同source・固定tool・証明書の手順をsandbox外で実行し、最終profileを通過した。これはWindows試験環境の観測であり、製品のTLS検証を緩める根拠にはしない。

MySQLのTLS初回は[28ケースの後のfresh disconnectで失敗](evidence/windows-mysql-required-initial-disconnect-failure.json)した。原因は未確定である。公開disconnectの固定error分類だけを追加し、受入条件を緩めずに再buildした最終のsandbox外実測は通過した。成功・失敗の差を確定原因の解消と呼ばない。失敗時の有限cleanupは捕捉したprocess handle/PIDだけを使い、残留handleを再試行する。通常終了での全child reapは確認したが、全descendantの独立inventory・全障害下の回収までは実証していない。

## 元wpdbの比較基準

[native fixture](fixtures/native_wpdb.php) は元WP 7.1.3のload.php、plugin.php、WP_Hook、filter sentinel、class-wpdb.phpを読み、stubや元sourceの変更を加えない。WP_SETUP_CONFIGでconstructorの接続を遅延し、suppress_errors後のdb_connect(false)で接続する。[native_check.py](native_check.py) はPHP 8.5.12の固定ZIPから全78ファイル、WPの固定tarから読込対象5ファイルを直接照合する。PHP実行時にもfixtureと元5ファイルだけというinclude集合を検査する。

[MySQL](evidence/windows-mysql-disabled-native-database.json)と[MariaDB](evidence/windows-mariadb-disabled-native-database.json)のfresh非TLS fixtureで各27ケースが通過した。get_rowの3形式、get_resultsの4形式、get_colとcached結果、get_varの数値文字列・NULL／empty→null、0行の列名、prepareの識別子とescape、prefix、num_queriesの増加と非増加、更新件数・SQL error後の状態・rollbackを元APIのまま確認した。文字列をhexで保持し、PHPのarray/object/keyとnull/bool/intを区別して記録する。正常shutdown・全child reap・元source不変も確認した。

この27ケースは次のPHP adapterを比較する基準である。**PHP→Rust→実DBの結合試験はまだ行っていない**。native TLS、非UTF-8 ERR、全列metadataのmysqli対応、全wpdb API、通常WP bootstrap、テーマ・plugin、永続cache、製品driver／公開ABIは未受入のままである。
