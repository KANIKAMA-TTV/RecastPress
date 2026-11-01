# 明示 raw 値の上限付き unserialize 候補

2026-10-29。新 WP helper と、その helper を既存の raw DB reader へ test-only で接続する開発候補。`target/bounded-unserialize-full-check.log` は exit 0 で、全 8 fresh DB 構成が各 62 契約を通過し、全体 gate と最終 11 checker が成功した。通常 DB／公開 server の WP 依存境界は維持し、全 DB fixture を回収した。native Linux 実行や製品の option／meta 取得 policy の完成は主張しない。

## API と境界

API は `recastpress_wp::serialization::maybe_unserialize_bytes_with_limits(raw, limits)`。`UnserializeLimits::new(max_input_bytes, max_array_entries, max_owned_string_bytes, max_array_depth)` の 4 値を caller が明示し、depth は 64 以下とする。既定値・`get_option`・autoload 選別・cache・認証／公開 policy は追加しない。

復元する型は canonical な Null、bool、signed 64-bit integer、任意 byte string、順序付き array。object・reference・float は復元しない。既存の strict recognizer が serialized と認識しない値は、外側の空白も含めた元 bytes のまま返す。認識後の未対応・不正・非canonical な値は明示的なエラーとなる。一層だけ復元し、返った文字列を再び decode しない。

入力上限は外側の PHP trim 対象 bytes も含めて先に検査する。非serialized値のコピーは元の全長、serialized値は文字列 payload と byte key の合計を所有長へ数える。array entry は入れ子全体の key/value pair の累計、depth は array の入れ子だけを数え、0 はscalarのみを許す。node・capacity の領域はこの文字列上限へ含まない。新経路は完全な canonical 再serialize buffer を追加せず、整数・長さ・キー変換・重複等を局所検査する。旧APIの最終再serialize照合は維持する。

拒否は `InvalidLimits`、`UnsupportedValue`、`InputLimit`、`EntryLimit`、`OwnedBytesLimit`、`DepthLimit`、`AllocationFailed` を区別する。各コピー・array追加の前に該当上限とfallible reserveを検査し、失敗時に部分的な値を返さない。逐次解析なので上限拒否が後続の構文不正より先に出る場合がある。重複キーの照合は明示entry上限の範囲でも二乗時間になり得るため、線形CPU保証ではない。

入力 byte、array の累計 entry、返却値の byte string と byte key の累計所有長、array depth の制限は、RSS／peak memory／時間の上限ではない。SQL 読取り自体の転送量や、既に取得した DB raw row の確保上限とも別である。対応値の型、canonical serialization、拒否と既存 `maybe_unserialize` の差は新 helper の最終 source と試験で確認する。旧 API の意味を変更した結果として扱わない。

返却する `PhpValue` の Debug は redacted ではない。caller は private な decoded 値を診断へ流さず、この DB 契約でも入力・decoded 値を含まない固定の失敗表示を使う。typed limit／unsupported／allocation error は raw bytes や key を保持しない。

## DB の有限 test-only 接続

`crates/recastpress-db/src/database_tests.rs` に `option_meta_selected_decode_pipeline` の 1 label を追加し、DB 契約は 61 から 62 になる。WP の正式 API 比較件数へこの DB 関係を加算しない。既存 `recastpress-wp` dev-dependency を使い、DB／server の通常 runtime 依存へ WP を追加しない。

call は `post_name_preprocess_lookup_pipeline` の直後、後段 `meta_view_database_tests` が 5 行を追加する前。開始時は postmeta 6 行、options 4 行を全 raw field ごと保存する。前者は初期 5 行と保存 ID 0、後者は初期 3 行と ID 0 であり、後段の postmeta 11 行と混同しない。

controller は raw row ID を明示して次を区別する。

- 未存在 ID と SQL NULL は別状態。SQL NULL を serialized null とみなして helper へ渡さない。
- 空 byte string は空の Bytes。専用行 `N;` は decoded Null、`b:0;` は false。
- 既存 canonical array は key `key` に NUL 入り 3 bytes `a\0b` を持つ。明示した options ID 1／2／3 と postmeta ID 3 の同値を確認し、autoload の yes／auto-on／off は変換しない。
- 同じ meta key の ID 3 と ID 4 は別の raw 行として保持する。前者の array と後者の文字列 `second` を、first／last の取得 policy にまとめない。
- 専用の object 風 encoding は typed refusal。失敗を Null や false へ置換しない。

専用 postmeta ID 1001／1002／1003 が未使用であることを全 raw snapshot で確認してから、公開用固定 dummy の `N;`／`b:0;`／`O:1:"X":0:{}` だけを挿入する。既存行は更新せず、読み取った値の decode と各上限の exact／1 不足拒否を検証する。同じ read-only snapshot と fresh snapshot の全 raw 値が一致することを確認する。

通常の返却エラーでも専用 ID の削除と元 postmeta 6／options 4 の全 field 比較を試行する。これは任意の process kill／panic／SQL 障害後までの自動復旧保証ではなく、所有した disposable DB controller の cleanup 契約である。任意の既存 DB へ実行する手順ではない。SQL NULL にならない options 列へ NULL を格納したり、utf8mb4 text 列へ任意の不正 UTF-8 を格納できると仮定したりしない。

## 証跡と検証結果

DB collector は既に DB／core source に加え WP Cargo と全 WP source を追跡する。新 helper と DB test source の変更後に Python の必須 label を 62 へ更新し、全 8 fresh DB fixture を実再測定した。旧 61 件の結果を新 source の成功へ付け替えていない。

対象 WP 実行は lib 26 件（新 private 1 を含む）、新 bounded public 6 件、既存 serialization 11 件が成功した。WP 全 target Clippy と担当ファイル fmt check も成功。最初の新試験は `b" N; "` の実長 4 を 5 と数えた試験期待の誤りで停止し、limit を 3 へ訂正した。初回失敗 log を保持し、production source をその修正理由で変えていない。

先行する DB 確認は `cargo test -p recastpress-db --lib --no-run --offline --locked` と `cargo clippy -p recastpress-db --all-targets --offline --locked -- -D warnings` がともに exit 0。log は `target/bounded-unserialize-db-compile.log` と `target/bounded-unserialize-db-clippy.log`。担当 DB test の file 限定 rustfmt も完了した。この先行段階では DB を起動せず、live 契約と区別した。

その後の `target/bounded-unserialize-full-check.log` で workspace Rust 548 件（ignore 1）＋独立 29 件、Python 324 件（318 成功・symlink 権限不足 6 skip）、Clippy、server／WP の Linux cross-check、license 215 観測、HTTP／process smoke が成功した。生成 10、legacy 113（actual 88）、credential stage 33（actual 25）も実再測定した。全 8 fresh DB profile が各 62 契約を通過し、専用 DB directory は回収後に空である。

正式 PHP 比較は従来 72 API／482,951 件、hooks 572／dispatcher 405 のまま維持した。旧件数・入力・PHP／Rust 両結果 SHA の保存を `target/bounded-unserialize-old72-preservation-check.log` で実照合し、今回の bounded helper や DB 関係を新しい正式 WP API 件数へ加算していない。

最終既存 11 checker は `target/bounded-unserialize-current-check-summary.json` ですべて exit 0。`target/bounded-unserialize-normal-dependency-check.json` でも公開 server と通常 DB の WP 依存境界を確認した。これらは有限 fixture と source に束縛した開発確認で、任意 DB／任意 serialized 値の受理、RSS／CPU 上限、cache／get_option／get_post_meta／認証・公開判断、native Linux の成功を示さない。
