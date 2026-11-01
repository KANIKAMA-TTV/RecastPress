# α候補：保存した記事テキストから固定 HTTP 世代まで

現在地の照合（2026-10-30）：[実装と上位設計・重要順報告](implementation-design-reconciliation-2026-10-30.ja.md)を参照。以下のcheckpoint／件数／currentという記載は各観測時点の履歴であり、直近Cookie本体取り込み後の全体合格を示さない。現在は対象Rust21／Python12・fmt／Clippyまでで、正式比較・DB・依存台帳等の再検証は未実行。今回の文書更新で製品コード開発は再開していない。

最新checkpoint：	arget/credential-stage-and-pipeline-full-check.logで全8 fresh fixture各57、Rust498＋独立29／Python222／PHP68 API453,064・別枠stage33、Clippy・Linux cross-check・license215・実HTTPが成功。最終11checkerがcurrent source/binaryと一致し、全fixtureを回収した。以下の48項目gateは過去の検証履歴である。

更新日：2026-10-29（日本時間）。この経路は、所有する使い捨て DB の記事テキスト保存を、明示選択した固定 snapshot、HTML bundle、receipt、別々の public process へつなぐ有限実験である。scope は `candidate_owned_post_text_to_sealed_http_only`。製品の認証付き記事編集・公開フローの完成を示すものではない。

`target/legacy-password-and-user-lookup-full-check.log`で、legacy検証・user lookupを含む全体gateが成功した。workspace Rust458件（ignore1）＋独立29件、Python194件、正式PHP64 API・437,198ケース／hooks572／dispatcher405、別枠の生成10ダミー相互検証とlegacy64ケース、8つのfresh fixture各48 DB項目、Clippy・server/WP Linux check・license215観測・実HTTPが通過し、専用DB directoryは空になった。α evidenceも両backendの実測で再生成しcurrent source hashを照合した。以前の437／170、443／182件gateは履歴である。evidenceの日時や成功flagだけで将来の変更後も成功とは判断せず、追跡source変更時は再実行する。

## 保存と snapshot の境界

入口は [database_smoke.py](../scripts/database_smoke.py)。固定の Windows 開発用 MySQL 8.4.13 または MariaDB 11.8.9 を、新しい所有ディレクトリと loopback port で起動する。既存 DB や本番資格情報は使用しない。Cargo が返す lib test executable を直接捕捉して実行し、結果 marker 内の実 PID と捕捉した子 PID の一致、現在の62個の必須DB contract（旧観測の57件とは区別） を確認する。

[DB contract](../crates/recastpress-db/src/database_tests.rs) の `alpha_post_inputs` は、専用 ID 1001 が未使用であることを確認し、テスト controller が新しい一行を作成する。元からある 4 記事と 11 postmeta を先に保持する。記事の title/content/excerpt を CAS で A に保存し、`Updated` を確認した後、fresh snapshot から読み直す。同じ操作で B を保存して読み直す。期待値は前の読み取り結果に基づく。CAS は限定された記事テキスト更新であり、WordPress の `wp_update_post`、hook、revision、権限処理の同等性を主張しない。

取り出す公開入力は、各世代について `target`、`title`、`body_text` の三つだけである。target は controller が固定した `/alpha-post`。status、password、slug、author、meta、日時から公開対象や URL を自動決定しない。元記事の title/content/password、author 値、meta の key/value が選択したテキストへ混入していないことも有限 fixture で検査する。これは一般的な機密検出・個人情報除去の保証ではない。

**専用 ID 1001 は A/B の取得後、HTTP を始める前に削除される。** 残った posts と postmeta が元の 4 記事・11 行と一致することを検証してから、固定 A/B 入力を Python 側へ返す。したがって後続 HTTP は保存済みの A/B snapshot を配信する実験であり、その時点の DB 行に同期する view ではない。DB を生存させて HTTP を実行することも、現在の DB 内容の反映や live publication を意味しない。

## prepare、receipt、固定世代の観測

[alpha_publication_path.py](../scripts/alpha_publication_path.py) は次の順序で操作する。

1. A/B の入力 shape と差分を確認し、それぞれ `prepare-posts` を別の捕捉した子プロセスで実行する。固定 literal-text layout の v1 bundle が一ページで、target/status/content type が期待どおりかを照合する。HTML、shortcode、block、PHP、Wasm/theme として本文を実行しない。
2. bundle の実 byte 数と SHA-256 を独立計算し、`bundle-receipt` の出力と照合する。入力と bundle wire の上限は 8 MiB。render 後の本文にも cap を適用し、bundle admission を通す。詳細は [prepared posts](prepared-posts-development.ja.md) を参照する。
3. A の bundle と receipt を指定して public process を起動し、GET の完全な body 一致、HEAD の長さと ETag、`If-None-Match` の 304 を観測する。
4. A の入力 bundle ファイルを B の bytes に書き換えても、稼働中の A が同じ body と ETag を返すことを確認する。書き換えた bundle と古い A receipt の新候補は、`PayloadDigestMismatch` により listener 告知前に非ゼロ終了する。既存 A は維持される。さらに A の bundle ファイルを削除しても A の応答が維持される。
5. A の停止と回収を確認してから、B を別 process として起動する。B の body と ETag は A と異なり、A の旧 `If-None-Match` は B の 200、旧 `If-Match` は 412 になる。B も停止・回収する。

receipt は payload の長さと SHA-256 による **byte identity** の照合である。署名、producer 認証、公開権限、最新世代の証明、rollback 防止ではない。ETag も選択された表現の validator であり、DB の更新番号や認可情報を代替しない。A 停止後に B を開始する観測には停止区間があり、live switch、zero downtime、公開取り消し期限の保証は含まない。

## 環境と回収

public と prepare/receipt 子へ渡す環境から、大小文字を区別せず `RECASTPRESS_DB_`、`MYSQL_`、`MARIADB_`、`RECASTPRESS_ALPHA_` で始まるキーと `DATABASE_URL` を除く。注入した canary も filter されることを確認する。この scrub は特定の環境変数を渡さない境界である。別 OS user、container、file permission、同一ユーザーによる他ファイルアクセス、ネットワーク権限の隔離を確立するものではない。

[smoke.py](../scripts/smoke.py) は public 子を Popen 直後から強参照で保持する。起動 reader の失敗や timeout も回収対象とし、停止は terminate、kill、有限 wait、reader join を段階ごとに試す。停止や reader 終了を確認できなければ pipe を閉じず handle を保持し、成功と扱わない。`reap_servers()` は全保持子を再試行し、一件の失敗で他の回収を飛ばさない。元例外があれば cleanup 情報を notes に残す。

DB harness の finally は DB の停止処理後にも public と CLI probe の registry 回収を試みる。成功時は専用 fixture ディレクトリを削除し、`--keep` 指定や失敗時は診断用に保持する。有限 cleanup と再試行は、OS が強制停止を永久拒否した場合にも回収できるという保証ではない。

## 実行と evidence

固定ツールと参照 schema の準備が済んだ Windows 開発環境で、リポジトリ root から実行する。各 backend は別の fresh fixture を起動する。冒頭の全体gateでは次の両backend profileも実行した。

```powershell
python scripts/database_smoke.py --backend mariadb --alpha-publication-demo
python scripts/database_smoke.py --backend mysql --alpha-publication-demo
python scripts/check_alpha_publication_evidence.py
```

`--alpha-publication-demo` は cleanup exercise、binlog、GTID bootstrap、snapshot bootstrap、rotation、server restart の各 probe と同時指定できない。これは独立した fixture profile である。全体 gate へ組み込む入口は次のとおり。

```powershell
python scripts/check.py --with-alpha-publication
```

この gate は通常の Rust/Python 検査に続いて両 backend を実測し、最後に evidence checker を実行する。他の reference、process smoke、Linux cross-check などはそれぞれの明示オプションで要求する。cross-check の成功だけで native Linux 上の試用完了とはしない。

成功した有限実験は `compatibility/evidence/alpha-publication-mariadb.json` と `alpha-publication-mysql.json` を出力する。checker は DB を起動せず、両ファイルの scope、正確な fields/type、48 必須 checks、固定 backend version、UTC 日時、PID の必要関係、終了 code、8 MiB byte 上限、hash、強い quoted ETag、A/B の差、各観測 flag、limitations、追跡ソースの現在 hash を照合する。重複 JSON key、未知 field、古い source hash は拒否する。順次終了した process の PID 再利用はあり得るため、全 PID の永久一意性は要求しない。

この evidence は当該 fixture 実行の記録である。認証後の管理画面からの保存、一般的な WordPress visibility policy、theme 実行、production CDC、自動 DB 同期、公開操作の認可、native Linux 配布・運用の検証は、この経路の範囲に含まれない。
