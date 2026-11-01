# wpdb → Rust RPC のdriver縦断候補・準備案

2026-11-02。P0の接続候補1を小さく実証するための準備であり、P1の実装完了、標準wpdb互換の採用、製品driverや公開ABIの決定ではない。既存のediting側SQLx経路は変更しない。

[driver-plan.json](driver-plan.json) と [mysql_async用patch案](patches/0001-mysql-async-wpdb-handshake.patch) は、patch案を作成した時点の準備状態を固定する。planの未適用・未コンパイルというstatusとfalseの実証範囲は書き換えない。その後のhelperによる所有stagingへの適用・依存補充・2 unitの実測は、専用evidenceで別に記録する。元registry source、root/binlogのCargo manifest/lock、editing側の依存は変更せず、DB processや実接続も作成しない。

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

両positive実測で、release test binaryを実際に生成し、新規2 testだけを列挙して `2 passed / 0 failed / 0 ignored` を確認した。実行前後の63/85全source inventory、元registry archive/source、元root/binlog lock、最終実測のhelper入力の不変と所有childのreap/cleanupを確認した。成果binaryはignored target内に留める。raw handshake・SQL bytes・raw ERR・実DB・native wpdb比較・製品採用はすべて未実証のままである。
