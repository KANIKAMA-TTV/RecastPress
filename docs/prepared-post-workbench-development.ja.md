# 公開用 JSON 入力ワークベンチ

現在地の照合（2026-10-30）：[実装と上位設計・重要順報告](implementation-design-reconciliation-2026-10-30.ja.md)を参照。以下のcheckpoint／件数／currentという記載は各観測時点の履歴であり、直近Cookie本体取り込み後の全体合格を示さない。現在は対象Rust21／Python12・fmt／Clippyまでで、正式比較・DB・依存台帳等の再検証は未実行。今回の文書更新で製品コード開発は再開していない。

2026-10-29 時点の開発用静的補助画面。入口は [`tools/prepared-post-workbench/index.html`](../tools/prepared-post-workbench/index.html)。記事の文字入力から、既存 CLI に渡す JSON ファイルを作る。管理認証、編集用 DB、公開切替へ接続する画面ではない。

2026-10-30 の拡張として、現在の記事から固定 bundle と receipt を作る [単一 ZIP 書出し](prepared-post-workbench-package-development.ja.md)を追加した。既存の JSON 書出し・読込みは維持する。ZIP を保存・解凍した後は `check-package` で最終受理を確認する。今回の全体 gate と最終11checkerはすべて成功（`target/workbench-package-current-check-summary.json`）。OS の ZIP 保存／download は未確認である。

## 開く

`index.html` をローカルファイルとしてブラウザーで開くか、リポジトリ直下で次を実行する。

```powershell
python -m http.server 8765 --bind 127.0.0.1 --directory tools/prepared-post-workbench
```

その場合は `http://127.0.0.1:8765/` を開く。静的サーバーは開発用で、確認後に終了する。loopback への bind は管理認証を追加するものではない。

HTML・CSS・JavaScript は同じフォルダーのファイルだけを使う。アプリケーションは fetch、外部 font／CDN、clipboard、localStorage／sessionStorage を使用せず、入力を送信・自動保存しない。HTTP で開く場合の静的ファイル取得と、利用者が開始するブラウザーのダウンロードは別である。タブ内の入力は再読込み・終了で失われる。書出し後のファイルはブラウザーと OS が保持する。

## 編集と書出し

初期値は `examples/prepared-posts-input.json` と同じ公開用固定サンプル 1 件。記事を追加・切替・削除でき、最後の 1 件は残す。「直前の削除を取り消す」は 1 段だけの削除取り消しで、削除した記事を元の位置へ戻す。文字編集の Undo はブラウザーの入力欄に任せる。

新規記事の target は空欄で、利用者が明示する。title と body_text は空欄を許す。JavaScript は値を trim・Unicode 正規化・slug 自動生成しない。入力欄は textarea であり、ブラウザー自身の改行処理を超えた任意 byte 入力保持は保証しない。プレビューは textContent だけで構築し、HTML・script・shortcode 風の本文を実行・解釈しない。target も移動できるリンクにしない。

書出しは UTF-8 の compact JSON と末尾 LF。内部記事 ID を除き、一覧順に次の 3 field を含める。

```json
{"format_version":1,"posts":[{"target":"/chosen","title":"","body_text":""}]}
```

Blob と所有 Object URL を使い、`prepared-posts-input.json` という名前でダウンロードを開始する。Object URL は時間経過または pagehide で解放する。ブラウザーは保存先や名前を変更する場合がある。開始表示は保存完了、CLI の受理、公開成功の確認ではない。

## 保存済み JSON を読み込む

ファイル選択から、明示したローカル UTF-8 JSON 1 ファイルを File API で読み込める。検査成功時も現在の記事一覧を直ちに変更せず、件数と先頭最大 5 件の target を一時表示する。「適用」を明示的に選ぶと全記事を置き換える。追加・merge は行わず、適用前の記事一覧へ戻す機能はない。適用時は削除 Undo もリセットするため、残す入力は先に書き出す。

読込み失敗では現在の作業を保持し、一時表示を消す。キャンセルも現在の作業を保持し、進行中の読込みの結果を以後適用しない。キャンセルは File API の I/O 自体を abort する機能ではない。連続して別のファイルを選んだ場合は最新の選択だけを採用する。

専用 `input-parser.js` はブラウザーと CommonJS で同じ `parsePreparedInput(Uint8Array)` を使う。8 MiB 以下の実入力 byte を fatal UTF-8 で decode し、BOM、不正 UTF-8、孤立 surrogate、JSON 文法違反を拒否する。固定 schema の object／array／string と integer literal `1` だけを読み、decoded key の重複、未知・欠落 field、null や別の型を拒否する。全記事は必須 3 field のみ。native の入力 parse 自体は空配列を許すが、この画面は少なくとも 1 件を保つため空配列を拒否する。件数の独自上限は追加せず、サイズと既存 target／NUL 条件を検査する。

読み込んだ CR／LF 等は model の文字列へそのまま保持する。記事を選択したり別 field を編集しただけでは、未編集 field の元文字列を変更しない。本文入力欄そのものを編集すると、その欄にはブラウザーの textarea 改行正規化が適用される。入力 field の byte 表示・プレビューと export は保持中の model を使用する。

1 ファイルの 8 MiB 制限と、書出す JSON の 8 MiB 制限は別々に評価する。ちょうど 8 MiB の入力を読み込めても、再シリアライズや末尾 LF により書出し上限を超える場合がある。これはタブ全体の RSS／メモリ上限ではない。編集中の model と一時読込み、parse、プレビュー、Blob 等が同時に存在し得る。文字編集・追加・削除・適用後は、現在の内容をまだ書き出していない旨を表示し、以前のダウンロード開始表示を現在の状態へ流用しない。

## 補助チェックと CLI の境界

target は native `valid_target` に合わせた次の部分集合を検査する。

- UTF-8 で 8,192 bytes 以下、単一の `/` で始まる。`//` 始まりは拒否する。
- C0 制御文字、ASCII space、DEL、`#`、バックスラッシュを拒否する。
- 完全一致の target 重複を拒否する。case・percent escape・query 順を正規化しない。
- 最初の `?` より前が `/health/live` または `/health/ready` なら拒否する。

全 field の孤立 UTF-16 surrogate を、TextEncoder による置換より前に検出して書出しを止める。title／body_text の NUL も拒否する。実際に出力する末尾 LF 込み JSON の UTF-8 サイズが 8 MiB を超える場合は書出しを止める。field ごとの byte 数と JSON サイズを表示する。

これらは native renderer／bundle admission の完全な再実装ではない。HTML escape 後の本文量や完成 bundle の制約を含め、リポジトリのルートで既存 CLI に最終受理を確認する。入力ファイルのパスを実際の保存先へ合わせ、まだ存在しない出力先を指定する。

```powershell
cargo run -p recastpress-server --bin recastpress -- prepare-post-package prepared-posts-input.json target/prepared-post-package
```

bundle と receipt、既存出力の拒否、不完全ディレクトリの保持などは [package の契約](post-package-development.ja.md)に従う。画面は status／password／visibility による記事選別、URL 安全性、unique slug、公開承認を判断しない。

## 実際に確認した範囲

Windows のブラウザーで desktop の 3 領域表示を確認した。新規記事の target が空であること、切替時の入力保持、重複 target・予約 path・NUL の拒否を操作した。狭幅は要求 390px／実際の CSS viewport 375px で横方向のあふれがないことを確認した。画像は `target/prepared-workbench-desktop.jpg` と `target/prepared-workbench-mobile.jpg`。実機 mobile／touch 操作は未確認。削除・取り消しは source review と後述の Node 契約で確認し、実ブラウザー操作の実績へ加えない。

ブラウザーが実際にダウンロードした `C:/Users/nodew/Downloads/prepared-posts-input.json` は 215 bytes、SHA-256 は `23ac9fee40679553807ed890172dcb21b352e18d7677e322a92be98ec10aa357`。次の公開用 dummy 2 件を含めた。

- `/chosen?b=2&a=1`：タイトルは `Title 日本 & < > " '`。本文には先頭 LF、script 風の文字、tab、`&copy;`、日本語、🦀 を含めた。
- `/empty`：title と body_text は空文字。

この実 download を変更せず、既存 `prepare-post-package` を所有プロセス PID 21644 で実行して成功した。成果物 `target/prepared-workbench-native-roundtrip/` の bundle は 776 bytes、receipt は 117 bytes。独立 HTML parse で title／heading／body の文字一致、先頭 LF、空欄、Unicode、query の字面と順序保持を確認した。

既存 sealed loader を別の所有プロセス PID 35656 で起動し、2 件の GET 本文がそれぞれ 355／192 bytes で期待値と一致、HEAD が空本文かつ全体 Content-Length であることを確認した。逆順 query は 404、異なる Host は 421。起動したプロセス・reader と両 registry は回収済み。詳細は `target/prepared-workbench-native-roundtrip.json` と同名 `.log` に記録した。これらはローカル開発観測で、署名された証跡や実サイト移行の成功判定ではない。

画面確認用の所有静的サーバー PID 7916 も exit 0 で終了し、thread join とブラウザー tab 2 の終了を確認した。JavaScript は `node --check` を通過し、DOM／privacy の独立 source review に blocking finding はなかった。孤立 surrogate・8 MiB 境界を含む全 UI 経路をブラウザーで操作したという主張ではない。専用 Python 試験は追加していない。旧 backend／Cargo／DB／probe／証跡 source は変更していない。

## 読込み機能の追加検証

独立 Node 契約は次で実行する。追加ライブラリーは使用しない。

```powershell
node --test scripts/test_prepared_workbench_input.js
node --test scripts/test_prepared_workbench_ui.js
```

parser 6 群は、Unicode／CR／空 field 保持、escaped key 重複、未知・欠落 field、深い別構造の拒否、不正 UTF-8／BOM／surrogate、target の byte 境界、実入力 8 MiB ちょうどと 1 byte 超過を含み、全成功した。UI 結合 7 群は実 app／parser と最小 DOM・実 Blob・遅延 File の fixture で、一時表示と明示適用、失敗時保持、キャンセル・最新結果優先、削除 Undo、未編集 CR の export 保持、古い download 表示の解除を確認した。実ブラウザーの代用による全画面動作保証ではない。結合 log は `target/prepared-workbench-ui-integration.log`。

別の有限 native parity 73 ケースでは JS が 32 件、既存 `prepare-post-package` の最終出力までが 30 件を受理した。残る受理差 2 件は HTML escape 膨張と最終 bundle overhead／出力 cap による意図した最終拒否で、41 件は双方が拒否した。受理ケースは返却 field の SHA、独立 native HTML parse、receipt identity も照合した。詳細は `target/prepared-workbench-import-native-parity.json` と同名 `.log`。既存 isolated server binary と parser の hash は前後一致、所有プロセス・一時ディレクトリを回収した。native parser 単体と同一受理集合であるという主張ではない。

追加ブラウザー確認では、公開 dummy `target/prepared-workbench-browser-expectation.json` を選び、2 件の target が一時表示されても編集中の初期サンプル 1 件と値が残ること、キャンセルで一時表示が消え入力が残ることを操作した。適用ボタンは実ブラウザーでは押していない。画像は `target/prepared-workbench-import-desktop.jpg`（一時表示）と `target/prepared-workbench-import-mobile.jpg`（キャンセル後）。狭幅は要求 390px、CSS viewport と scroll 幅はいずれも 375px。ファイル選択待ちに環境上 1,333 秒を要したため、この観測から読込み性能を評価しない。

追加確認の静的サーバー PID 38092 は exit 0 で停止し、thread join と tab 3 の終了を確認した。キャンセル表示修正前の所有サーバー PID 25760 も exit 0 で停止済み。これらの限定確認は認証、DB、公開操作、実サイト移行、全ブラウザー互換性の成功を意味しない。
