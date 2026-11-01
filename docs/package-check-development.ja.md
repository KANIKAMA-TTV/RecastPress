# Bundle と receipt の one-shot 検査

後続の[sealed CSS／JavaScript](sealed-public-assets-development.ja.md)ではV3の受理を追加し、V3だけreport2（九MIME key）を返す。V1/V2のreport1七MIMEと既存CLI bytesは保持し、receiptはschema1のまま。以下のv1-v2／schema1説明と検証記録は導入時の履歴として残す。固定例は `examples/sealed-assets-bundle.json`／`examples/sealed-assets-receipt.json`。

2026-10-29 の開発用 CLI 候補。`recastpress check-package <bundle> <receipt>` は、明示した 2 ファイルの byte identity と既存 bundle admission を一操作で確認し、内容を含まない小さな report を返す。listener を開かず、ファイルの作成・書換え、公開切替、DB／PHP／Wasm の起動を行わない。既存 `check-bundle <path>` の受理と出力は変更しない。

対象 native 試験と実 CLI 6 群、下記の二段階 workspace 確認・継続 gate は完了した。最初の log には試験 fixture の誤りによる Python 失敗が残っており、最初から全成功した記録とは扱わない。native Linux 実行の成功は主張しない。

## 実行

リポジトリのルートで、準備済みの固定 2 ファイルを明示する。

```powershell
cargo run -p recastpress-server --bin recastpress --offline --locked -- check-package target/post-package-example/bundle.json target/post-package-example/receipt.json
```

既にビルドした binary では `recastpress check-package <bundle> <receipt>`。実行ファイル名を除く引数は command 名と 2 path の合計 3 個だけで、両 path は空文字を拒否する。`args_os` の native path を使い、UTF-8 String へ変換しない。引数から任意 option や別 command を実行する入口は追加しない。Unix の非 UTF-8 path 契約が存在しても、Windows 上の実行を native Linux の成功と扱わない。

## 読取りと検査順序

`bundle_receipt::check_verified` は、既存 `load_verified` と共有する読取り・identity 検査から、同じ既存 bundle report 集計へ進む。

1. receipt を 1 回開き、1 KiB ＋ 1 byte sentinel まで読み、超過と strict schema を検査する。不正 receipt は payload の読取り前に拒否する。
2. payload を 1 回開き、既存の 8 MiB ＋ 1 byte sentinel の reader で owned buffer へ読む。
3. receipt の payload 長と SHA-256 がその buffer と一致することを確認する。
4. 同じ payload buffer を既存 strict v1／v2 admission と report 集計へ渡す。一致後に payload path を再度開いて別の内容を検査しない。

receipt は既存 schema 1 の `format_version`・`payload_bytes`・小文字 64 桁 hex の `sha256` だけ。未知・重複・欠落 field、型や版の違いは拒否する。receipt の識別が一致しても、不正 bundle、空集合、重複／予約 target、MIME・status・redirect・binary body 等の既存 admission が失敗すれば成功 report は返さない。参照 asset や URL を解決せず、読む path は明示された 2 個だけ。

## 出力と失敗

成功は exit 0、stderr は空、stdout は末尾 LF 込み 1,024 bytes 以内の JSON 1 行。既存 `BundleCheckReport` の次の 6 field だけを返す。

| field | 意味 |
| --- | --- |
| `format_version` | report の版 1 |
| `accepted_bundle_version` | 受理した入力 bundle の版 1 または 2 |
| `pages` | redirect を含む page 総数 |
| `redirects` | 明示 redirect の数 |
| `body_bytes` | decode 後の本文 bytes 合計。redirect は 0 |
| `mime_counts` | 本文 page の固定 7 MIME 別件数。0 件のキーも含む |

固定 MIME は HTML、plain text、RSS、Atom、PNG、JPEG、WebP。内訳合計と redirects の和が pages に対応する。入力 path、target、Location、本文、入力 SHA 等は report に含まない。receipt を新規発行する command ではない。

引数・IO・receipt・identity・admission・report 構築の通常失敗は exit 2、stdout は空、stderr は `package check failed` と LF だけ。任意の OS／serde 診断や入力内容を連結しない。完成した report の stdout 書込み・flush 自体が失敗した場合も失敗扱いとするが、OS が既に受け取った途中の report bytes は撤回できない。stderr 自体の配送も保証しない。途中出力も集計 report の bytes であり、入力の raw 値を診断へ出す分岐は設けない。

## 保証の範囲

この成功は、読んだ payload buffer の byte identity と既存 loader の受理だけを表す。receipt は署名、producer 認証、公開承認、秘密情報除去、最新版や anti-rollback の証明ではない。両ファイルを置換できる者は一致する pair を作れる。

各ファイルの読取りは atomic な 2 ファイル snapshot ではない。読取り中の同時変更、filesystem identity／symlink 隔離、path の敵対的変更への耐性、永続化は新たに保証しない。byte bound と小さい report は RSS／peak memory／全 allocation 失敗回復／実行 deadline の保証ではない。CLI 自身に timeout は追加せず、試験 harness の有限 timeout と区別する。検査成功から既存サーバーの世代切替や HTTP 到達性を推定しない。

関連する既存契約は [bundle admission と report](bundle-check-development.ja.md)、[receipt identity](bundle-receipt-development.ja.md)、[新規 package の作成](post-package-development.ja.md)を参照。

## 対象検証

新 native 契約では、receipt 先行拒否、identity と同一 buffer の admission／集計、厳密な native 引数、成功 report の有限出力と IO 書込み失敗を確認した。Windows の対象実行は receipt 5 件（新規 3＋既存 2）と command 2 件（新規 2）が全成功。新規 Windows unit は合計 5 件で、Unix native path の追加 1 件は Windows では未実行。server 全 target Clippy（warnings 拒否）と対象 4 ファイルの fmt も exit 0。log は `target/check-package-receipt-tests.log`、`target/check-package-command-tests.log`、`target/check-package-clippy.log`、`target/check-package-fmt.log`。Linux cross-check は別工程であり、対象 native 実行の結果へ含めない。

`scripts/test_package_check_cli.py` の 6 群は、Cargo JSON で選ぶ現在の executable を有限時間の所有プロセスで実行し、全成功した（`target/package-check-cli-contracts.log`）。v1／v2 集計、既存 report との一致、identity 不一致と一致しても admission 不正な入力、strict receipt と両入力の byte 境界、引数・IO の拒否、内容を含まない固定診断、既存 command の回帰を確認した。

最初の `target/package-check-workspace-check.log` では workspace Rust 541 件成功（ignore 1）、workspace Clippy、server／WP 全 target Linux cross-check が成功した後、Python 324 件中 1 件が失敗した。新 CLI 試験が非 ASCII の redirect Location を受理例へ入れており、native の既存 admission は正しく拒否していた。試験 fixture の Location だけを ASCII へ修正し、Rust source は変更せず、最初の失敗 log を保持した。

継続の `target/package-check-continuation-check.log` は exit 0。全 Python 324 件は 318 成功・symlink 権限不足 6 skip、inventory／license 215 観測、生成 10・legacy 113（actual 88）・credential stage 33（actual 25）、fresh alpha MariaDB／MySQL 各 61 契約、HTTP／process smoke が成功した。全 DB fixture を回収し、専用 DB directory は空。最終既存 11 checker は `target/package-check-current-check-summary.json` で全 exit 0 を確認した。Linux は cross-check であり、native 実行ではない。

保持中のレビュー用 package 2 個も、現在の Cargo JSON artifact の新 CLI で再検査した。report はそれぞれ 2 pages／547 body bytes と 1 page／153 body bytes で、検査前後のファイル不変を確認した（`target/package-check-retained-examples.json` と同名 `.log`）。これは明示した保持済みファイルの検査であり、別の Wasm end-to-end 再検証や製品採用の成功へ読み替えない。
