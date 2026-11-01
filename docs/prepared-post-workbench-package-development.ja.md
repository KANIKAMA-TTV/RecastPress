# 公開用記事ワークベンチの ZIP 書出し

2026-10-30 の開発記録。既存の [JSON 入力ワークベンチ](prepared-post-workbench-development.ja.md)に、現在の記事から固定レイアウトの bundle と receipt を作り、単一 ZIP でダウンロードを開始する操作を追加した。main 採用済みで、Node・native・所有 HTTP・ブラウザーの対象確認と全体 gate は成功した。最終11checkerも全exit0で、controller49832は出力回収後exit0を確認した。

## 利用の流れ

入口は [`tools/prepared-post-workbench/index.html`](../tools/prepared-post-workbench/index.html)。既存の編集、JSON import、JSON ダウンロードは引き続き使える。「ZIP を準備」を選び、準備できたら「ZIP をダウンロード」を選ぶ。予定 filename は `prepared-post-package.zip`。ZIP には `bundle.json` と `receipt.json` の２ファイルだけを含める。

ブラウザーで保存先を確認し、ZIP を明示したローカルディレクトリへ解凍する。その後、リポジトリのルートで解凍先に合わせて次を実行する。

workbenchの固定ZIPは、OSでの解凍の代わりに新しい [限定import CLI](package-unpack-development.ja.md)でも準備できる。明示したZIPを検査し、まだ存在しない出力先へ２fileだけを書き出す。importの対象確認・全体gate・最終11checkerも成功した（`target/package-unpack-current-check-summary.json`）。ZIP生成側の上記完了記録とは別工程で、詳細は限定import文書に記録する。

```powershell
cargo run -p recastpress-server --bin recastpress -- unpack-package prepared-post-package.zip target/prepared-post-package
```

いずれの方法でも、解凍先の対を次で確認する。

```powershell
cargo run -p recastpress-server --bin recastpress -- check-package target/prepared-post-package/bundle.json target/prepared-post-package/receipt.json
```

これは同一 bytes の照合と既存 strict bundle admission を行う [one-shot 検査](package-check-development.ja.md)で、listener を起動しない。ZIP 自動展開や公開切替を行うコマンドではない。ZIP の準備・ダウンロード開始は保存完了や公開成功を確認したことにならず、OS の保存先や名前はブラウザーが決める場合がある。

暗号機能を利用できない場合や生成を拒否した場合は固定の案内を表示し、編集中の入力を残す。既存 JSON をダウンロードして native `prepare-post-package` で準備する方法も使える。暗号処理を自作した fallback は設けない。

## 生成する内容

[`publication-package.js`](../tools/prepared-post-workbench/publication-package.js) は browser global `PreparedPostPackage` と CommonJS で同じ `preparePackage(inputBytes, cryptoProvider)` を提供する。実 UI は完成した JSON wire の UTF-8 snapshot を渡し、実際の隣接 `input-parser.js` で strict schema 1 を検査する。main に draft／target の module fallback はない。

caller が匿名公開用に選んだ target・title・body_text のみを扱う。target を trim・正規化・自動採番せず、一覧順を維持する。native `PreparedPosts` と同じ固定 HTML、status 200、`text/html; charset=utf-8`、bundle schema 1 を作る。title は title 要素と h1 に出力し、本文は `white-space:pre-wrap` の div に出力する。`& < > " '` を escape し、CR を `&#13;` として保持する。入力の HTML・script・entity・shortcode 風文字列を実行・解釈せず、WP formatter や theme は呼ばない。

入力は既存 parser の UTF-8／surrogate／target／NUL 条件と８ MiB 制限に従う。escape 後の各 HTML と集計、完成 bundle JSON＋末尾 LF にも別途８ MiB 上限を適用する。receipt は schema 1 の `format_version`、`payload_bytes`、lowercase SHA256 のみで最大 1,024 bytes。確定した payload bytes を WebCrypto SHA-256 へ渡し、同じ bytes を ZIP へ格納する。receipt は署名・認証・公開承認・anti-rollback ではなく、両ファイルを差し替えられる者は別の一致する対を作れる。

ZIP は固定 ASCII filename の STORE ２ entry。CRC32、central directory、固定 DOS 日時を出力し、圧縮・暗号化・任意 path・extra・comment・Zip64 は扱わない。全長上限は８ MiB＋1,024＋220 bytes。汎用 ZIP reader／writer や新しい製品 package 形式の採用ではない。単一 archive は二つの独立 download の混在を減らすが、保存完了や解凍後の改変を防ぐものではない。

## 非同期処理と所有

準備開始時に immutable な入力値と payload を確定する。編集・追加・削除・Undo・import の選択／取消／適用・新しい準備・pagehide は以前の完成 ZIP を失効させる。記事の選択だけでは入力値を変えないため失効させない。古い成功・失敗結果は現在の準備状態や案内を上書きしない。

WebCrypto は abort できないため、入力変更で失効しても既存 job が settle するまでは次の ZIP 準備を開始しない。その間も編集と JSON 書出しは使える。Blob は返却された archive bytes をそのまま使用し、Object URL は１秒後または pagehide に解放する。既に開始した download の撤回は保証しない。

文字列・model・HTML・JSON・payload・digest・archive・Blob が同時に存在し得る。各出力長の上限はタブ RSS、allocation 合計、実行時間、秘密消去の保証ではない。外部 fetch、storage、clipboard、自動送信、自動保存、DB、認証、session、principal、公開 API や自動 publish を追加しない。

## 確認結果と未確認範囲

- main の Node ３ suite は29件成功。input６、UI13、package10。UIは旧７契約と新６契約で、旧 input６＋UI７も保持する。実 Node WebCrypto、snapshot／stale success・failure、single job、JSON fallback、正確な１ ZIP Blob、receipt SHA、CR を確認した。DOM mock は browser layout や保存配送の試験ではない。
- [`test_prepared_workbench_package.py`](../scripts/test_prepared_workbench_package.py) の retry は４件すべて成功（32.290秒、`target/workbench-package-native-contracts.log`）。現 Cargo JSON の唯一 server artifact を選び、12種類の公開 dummy に対して JS ZIP の２ファイルと native `prepare-post-package` の出力 bytes が一致した。独立 `zipfile`／CRC／offset 検査、`check-package`、改ざん拒否、固定所有 server の GET・HEAD・conditional・Host・query・sealed 応答も確認した。
- `target/workbench-package-native-observation.json` は所有 process／reader の回収と所有 directory 削除を記録する。起動後に両入力 file を変更しても既に受理した応答が同一だった。これは連続公開や一般的な撤回保証ではない。初回の hex transport timeout 記録は `target/workbench-package-first-hex-transport-timeout.log` に保持し、retry 成功と区別する。
- Windows の実ブラウザーで WebCrypto による準備完了と desktop／狭幅表示を確認した。画像は `target/workbench-package-desktop.png` と `target/workbench-package-mobile.png`。要求390×844時の実 CSS 幅は375、document client／scroll width は375／375で、button が画面内に収まった。実機 mobile／touch は未確認。
- 今回、OS の ZIP 保存／download 操作は実行していない。旧 JSON download の過去観測を ZIP 保存の成功へ読み替えない。Linux native、一般 theme／HTML sanitizer、公開内容の資格・PII除去・認可は未確認／未実装。

Node log は `target/workbench-package-ui-main-contracts.log` と `target/workbench-package-main-contracts.log`。source freeze guard付き全体 gate は exit0（`target/workbench-package-full-check.log`、controller session21914は出力回収済み）。Rust610（ignore1）、pure313（62 artifact／features空／normal WP1）、Python419（412成功・skip7）、認証生成10／legacy113（actual88＋refusal25）／credential stage33（actual25＋refusal8）、Clippy・server/WP Linux cross compile・license218・process smoke が成功した。

この UI 工程では fresh DB fixture を起動していない。WP正式79 API／519,014件、hooks572／dispatcher405も再比較しておらず、この開発 UI をAPI数へ加算しない。最終11checkerは全exit0（`target/workbench-package-current-check-summary.json`）、controller49832も出力回収後exit0。保持対象23fileのうち実再観測した認証3証跡を除く20fileは開始baselineとbytes同一で、WP3proofと全DB証跡を保持した（`target/workbench-package-retained-identities.json`）。選択source10fileも変更無しを実assertした（`target/workbench-package-source-freeze-identities.json`）。`target/workbench-package-checkpoint.json` はこの文書freeze後にrootが生成する別操作で、ここでは生成済みと扱わない。
