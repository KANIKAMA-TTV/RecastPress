# 編集用post_name／post_typeのtext query候補

全体checkpoint（2026-10-29）：`target/sanitize-title-and-post-name-retry-full-check.log` exit0。Rust529＋独立29（ignore1）、Python248実行（247成功・1skip）、正式71 API466,887、全8 fresh DB60、Clippy・server/WP Linux check・license215観測・実HTTPが成功。両backendで新3契約も実行し、最終11checker全exit0（`target/sanitize-title-current-check-summary.json`）、全fixture回収済。初回のmeta baseline11誤期待をこのcall時点の6へ訂正し、直接両backend60成功後に全gateを再実行した。元4posts／6metaの全行Eq復旧は維持している。native Linuxは未実行。

新`EditingSnapshot::posts_by_name_text_page(name: &str, post_type: &str, after: Option<u64>, limit: NonZeroU16)`は、既存のprivate poolとREAD ONLY repeatable-read snapshot内で、呼出し側が選んだ2つのUTF-8 text keyをSQLへbindする低level reader。返値は既存の全23field `PostRow`のVecで、`POST_FIELDS`と既存decoderを再利用する。新writer、WPのnormal dependency、公開HTTPやURL resolverは追加しない。

## SQLと固定sourceの境界

queryは次の形。識別子は検証済み`TablePrefix`と固定posts table／列から作り、両key、cursor、limitは値parameterとして渡す。

```sql
SELECT <existing 23-field projection> FROM <validated posts table>
WHERE post_name = ? AND post_type = ?
  [AND ID > ?]
ORDER BY ID ASC LIMIT ?
```

空key、case、accent、trailing space、percent、quote、NULをtrim・sanitizeせずに渡す。`&str`のtext bindと列のcollationで比較し、binary bindや`CAST(column AS BINARY)`のWHEREへ置換しない。column width 200／20を検索入力の長さ上限へ転用せず、長い入力も同じtext queryへ渡す。PostRowの既存String／日付String／整数のstorage契約を維持し、任意binaryのdirty textを無条件に復号できるとは扱わない。

固定WP 7.1.3の`wp-admin/includes/schema.php:159-188`は、ID unsigned primary key、`post_name varchar(200)`、`post_type varchar(20)`と非uniqueのname索引を定義する。`class-wp-query.php:2182-2185`はname query前に`sanitize_title_for_query`を適用し、name条件を加える。このreaderはその外側の入力処理を取り込まず、nameとtypeの明示text条件だけを提供する別SQL subsetである。WP_Query全体と同じSQL・結果順・status選別を再現したAPIではない。

status、post_parent、password、author等で行を落とさない。複数の一致行を返し、最小IDの1件だけを採用したり、slug一意・型優先・parent優先・公開可否・認可を決めたりしない。pageのID順は明示したreader契約で、WordPressのURL解決順の推測ではない。cache、rewrite、pagenameの階層、canonical URL、空slugのfallback、sanitize_title、hook、テーマにも未接続。

`after=None`はID0を含む開始位置、`Some(0)`は0より後、`Some(u64::MAX)`は最大IDより後という意味。signed型や0 sentinelへ狭めない。page limitは正のu16行数で、本文のbyte数・RSS・全siteサイズ・共有memory予算の上限ではない。

## profileと閉じたerror

各query前に、postsがInnoDB、connectionのclient／connection／results charsetがutf8mb4かつcollation_connectionがutf8mb4_unicode_ci、両検索列がvarchar・charset utf8mb4・collation utf8mb4_unicode_ci・幅200／20であることを検査する。`CHARACTER_MAXIMUM_LENGTH`は`CAST(... AS UNSIGNED)`でmetadataのwire型を統一する。既存user lookupで観測したMySQLのsigned BIGINT／MariaDBのunsigned差を避け、幅の値やNULLの意味を変えない。

table名のmetadata照合はbinary spellingを使うため、metadataに格納されたtable名と一致するprefixが必要。lower_case_table_namesによる別caseの自動補正までこのguardで保証しない。

metadataと実SELECTは同時DDL／engine置換に対して原子的なattestationではない。安定した固定schemaとcollationが前提で、全23列のDDLをこの2列guardだけで認証したとは扱わない。reader接続のcharset等を変更する呼出し側はprofileを満たす必要があり、readerが不一致を黙って補正しない。

`PostNameLookupError::{ProfileMismatch, QueryFailure, RowDecodeFailure}`はpayloadの無い閉じた分類。前提不一致、metadata／行queryの失敗、既存PostRow decoderの失敗を分け、key・row・SQLx driver error・任意のsource文字列を保持しない。Debug／Display／Error sourceへkeyやdriver診断を含めず、新たなSQL statement loggerを設けない。返すPostRowは既存のediting DTOで、公開用DTOやidentity承認ではない。

## 有限fixtureの追加契約

既存57項目に次の3項目を追加し、総60項目とする。DB実行はrootの所有disposable fixtureで行う。

- `post_name_text_collation`: text-bound独立control SQLのID列と一致し、既存page readerの全23fieldと同値。name／type両keyのcase・accent・pad・quote・percent・NUL・empty・長い入力、private／draft／custom statusとraw内容を保持する。
- `post_name_text_pagination_snapshot`: ID昇順の1行pageを全件と比較し、既存ID0／MAXとcursor境界を検査。別connectionによる専用行の変更後も旧snapshotは旧結果を保ち、fresh snapshotだけが新name／titleと残る全fieldを返す。READ ONLY書込み1792拒否も維持する。
- `post_name_text_profile_rejection`: connection不一致を検査後に復旧。owned cloneでMyISAM、両列のbinary collation／不正幅、missing tableを拒否し、不完全projectionのquery failureとdirty binary textのdecode failureも閉じた分類へ分ける。

controllerは専用post ID1001～1004の未使用を確認してから作成・変更・削除する。新規投稿INSERT機能ではなく、public dummyだけの試験操作。この呼出し位置の元4posts・6postmeta全raw snapshotを取得し、専用行削除後に完全一致を確認する。cloneは削除し、既存行・metaや他checkを変更しない。任意の既存DB、global pause、FTWRL、既存proofの手動restampは対象外。

作成時点では実DBを起動していない。native compile／private契約と後続の実MariaDB・MySQL測定、全8fixture／全体gateの成功を区別し、未実行結果を先取りしない。

2026-10-29の限定native確認ではprivate 2件とDB crate全targetのClippy（`--offline --locked -- -D warnings`）、担当Rust fileのrustfmtが成功し、新しいignored実DB契約もtest targetとしてcompileされた。Clippy初回のmetadata tuple型の複雑さ指摘は固定type aliasへ整理して解消。実DBのcollation結果・decoder failure・snapshot・復旧の成功は後続実測を待つ。

初回全体gateの`target/sanitize-title-and-post-name-full-check.log`はMariaDBのquery前baseline件数assertで途中失敗した。ここは初期meta 5行にID0を加えた6行で、後続meta-view契約による5行追加後の11行と混同していたため、呼出し位置に合う4posts／6metaへassertを訂正した。取得済み元行集合との全raw Eq復旧比較、query本体、case順は維持し、この途中失敗を実DB契約の成功として扱わない。
