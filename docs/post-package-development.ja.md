# 投稿テキストのオフラインpackage候補

作成済みの固定 2 ファイルは [`check-package <bundle> <receipt>`](package-check-development.ja.md) で、公開プロセスを起動せず identity と既存 admission を検査できる。作成や公開を行う command ではない。

全体checkpoint（2026-10-29）：`target/offline-post-package-full-check.log` exit0。workspace Rust504（ignore1）、Python228実行（227成功・symlink権限不足1 skip）、Clippy・server/WP Linux check・license215・実HTTPとfresh alpha DB2構成各57が成功。比較binary再build後に既存生成・legacy・stage関係を実再測定し、最終11checker全exit0（`target/post-package-current-check-summary.json`）。WP/DB/core本体は無変更、正式68 API453,064・独立29・他6DB構成は直前実測とsource一致を維持。全fixture回収済。native Linuxでの実行は未実施。

`prepare-post-package <input> <new-dir>`は、既存の投稿入力から固定レイアウトのbundleとbyte identity receiptを一度に準備する実験用CLI。DB、HTTP、認証、テーマ、公開世代の切替へ接続しない。新規記事のDB INSERTやWordPressの投稿保存APIを実装したものではない。

入力は既存`prepare-posts`と同じ版1のJSONで、`format_version`と`posts`を持ち、各記事は`target`・`title`・`body_text`だけを受け取る。private、password、status等の追加項目から公開可否を推測せず、未知項目として拒否する。本文は文字列としてエスケープし、HTML・block・shortcodeとして解釈しない。入力と完成bundleの上限はそれぞれ8 MiBで、escape後の本文上限・既存bundle admission・重複target・予約routeの検査も維持する。

完成bundleとreceiptの生成・検証をメモリ上で終えてから、新しい出力ディレクトリを作る。固定名`bundle.json`と`receipt.json`をcreate-newで作成し、receiptは実際に書く同じbundle bufferの長さとSHA-256を記録する。既存ファイルを読み直してから別のbytesへreceiptを発行する経路ではない。

入力読取り・render・receipt生成の確保は共有メモリpoolへ未接続。最終byte上限はRSS・peak memory・同時処理全体の容量上限や、全allocation失敗の回復保証ではない。

既存の出力ディレクトリ（空でも不可）、ファイル、出力位置のsymlinkを上書きしない。親ディレクトリは事前に必要。管理されたローカル作業領域での利用を想定し、敵対的な親ディレクトリの同時変更を防ぐfilesystem sandboxやpath traversalの隔離保証ではない。

両ファイルの`write_all`・`sync_all`とhandleのdropを終えた場合だけ、exit 0で標準出力へ`post package prepared`と改行を返し、標準エラー出力は空にする。失敗はexit 2、標準出力は空、標準エラー出力は`post package preparation failed`と改行だけにし、入力本文・target・filesystem pathや任意のIO診断を含めない。

標準出力が空という拒否契約は入力検証・package作成の失敗を対象にする。完成後の成功メッセージ自体の書込みが途中で失敗した場合、既に書いた標準出力を撤回できない。コマンドは失敗として扱うが、完成したpackageは保持する。標準エラー出力の配送成功も外部出力先の状態に依存する。

ディレクトリ作成後の書込み・同期失敗では、不完全なディレクトリやファイルを残す。完了したpairの存在を失敗時に保証せず、同じ出力先への再実行も拒否する。失敗した出力先は成功品として配信せず、内容を確認して別の新しい出力先を使う。2ファイルのatomic commit、handle closeのエラーACK、親ディレクトリの同期、電源断後の耐久性を保証しない。`sync_all`の成功だけでこれらが成立するとは扱わない。

receiptはbyte identityだけを示し、署名、承認、秘密情報除去、最新版、巻戻し防止の証明ではない。bundleとreceiptを置換できる者は整合するpairを作れる。コマンド完了は公開を意味せず、利用者が明示的に既存`public --bundle ... --receipt ...`へ渡した場合にだけ、その別プロセスが起動時に読み込む。既存の`prepare-posts`、`bundle-receipt`、管理ready 503、公開GET/HEADの挙動は維持する。

## 実行例

リポジトリ直下で実行する。`target`は既存、`target/post-package-example`は未作成であること。

```powershell
cargo run -p recastpress-server --bin recastpress -- prepare-post-package examples/prepared-posts-input.json target/post-package-example
cargo run -p recastpress-server --bin recastpress -- check-bundle target/post-package-example/bundle.json
cargo run -p recastpress-server --bin recastpress -- public --host localhost:8080 --bundle target/post-package-example/bundle.json --receipt target/post-package-example/receipt.json
```

公開プロセス起動後は`http://localhost:8080/prepared-post`を参照する。コマンド自身はlistenせず、既存公開プロセスへ更新通知を送らない。

## 検証範囲

`python scripts/test_post_package_cli.py`はCargoのJSON出力から現binaryを特定し、有限時間のowned childで実CLIを呼ぶ。成功時の固定2ファイル、既存`prepare-posts`とのbundle全byte一致、独立した長さ・SHA-256と`bundle-receipt`の一致、既存`check-bundle`の受入れ、同入力の決定性と本文A/B差を確認する。

拒否契約は、不正JSON・未知private/password/status・重複・予約route・NUL・入力上限・escape膨張、引数・入力IO・親pathの不備、既存dir/file/input pathの保全、完了先への再実行を対象にする。symlink作成権限がある環境ではdir/file/dangling symlinkの非上書きも確認し、作成できない環境ではその試験だけ明示skipする。入力検証失敗で出力dirが作られないことも確認する。

途中write失敗の分岐はnativeの決定的fault契約へ分け、process試験で不安定なfilesystem raceや権限変更を誘発しない。これらの有限契約は全filesystem・敵対的同時変更・障害復旧の証明ではない。実行結果はゲート記録で確認し、この説明だけで未実行の試験を成功扱いしない。

2026-10-29の限定実行では上記Python suiteがexit 0、6試験中symlink作成権限を要する1試験を明示skipした。残る5試験群は現binaryで成功。Windowsでのsymlink非上書きの実測や、この変更後の全体ゲート成功までをこの限定結果に含めない。
