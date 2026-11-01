# 明示選択した移行元ソースのinventory候補

`prepare_migration_source_inventory.py`は、callerが選んだsource fileの相対path・byte長・SHA-256と未確認メモを、新規のローカルmanifestへ記録する実験用CLI。directoryの再帰走査、依存fileの推測、コード実行・変換・copy、network／AIへの送信は行わない。plugin／themeの移植完了、必要fileの網羅性、ライセンス適合、秘密情報除去の証明ではない。

## 明示selectionと起動

利用者が既存source-rootとUTF-8のselection JSONを用意する。選択例は次のとおり。これは自動抽出した依存一覧ではない。

```json
{
  "format_version": 1,
  "files": ["plugin/main.php", "theme/style.css"],
  "unconfirmed": ["必要fileの網羅性は未確認", "ライセンス確認は未実施"]
}
```

親dirが既存で、出力dirが未作成である場合に実行する。

```powershell
python scripts/prepare_migration_source_inventory.py path/to/owned-source selection.json target/source-inventory-example
```

成功はexit 0、標準出力に版1のsummary JSON一行、標準エラー出力は空。summaryは`format_version`、`file_count`、`total_bytes`、`manifest_sha256`の4fieldだけで、相対file名・未確認メモ・絶対root・コード・任意IO診断を転記しない。SHA-256は実際に保存したmanifest bytesのfingerprintであり、暗号学的な秘匿性を保証する出力ではない。

失敗はexit 2、標準出力は空、標準エラー出力は`Migration source inventory failed`と改行だけ。成功summaryの出力自体が途中で失敗した場合は、既に書いたbytesを撤回できず、完成manifestを保持する。

## 受入れと保存範囲

selectionは64 KiBまでのstrict UTF-8 JSONで、exact `format_version: 1`、`files`、`unconfirmed`を要求する。未知field、重複JSON key、float／NaN、boolを整数とする扱い、BOM等を受け付けない。filesは1～256個のrelative POSIX pathで、各pathはUTF-8で1,024 byteまで。absolute、空、`.`／`..` segment、backslash、colon、制御文字、空segment、重複pathを拒否する。

Windowsではnormcaseの結果が重複するpathを拒否し、各segment末尾のdot／spaceもOS alias防止のため拒否する。この追加条件をLinuxの名前規則へ一律に適用する製品policyではない。Unicodeや拡張子から役割・依存性・実行可能性を推測せず、file内容はinvalid UTF-8やPHP風のbytesもopaqueなsource bytesとしてhashする。

unconfirmedは0～64個の文字列で、各1,024 UTF-8 byteまで、NUL不可。空文字、改行、重複メモを含め、入力順・文字列を無加工で保持する。メモが未確認であることを一覧へ残す機能であり、未確認を解決済み・診断済みと扱わない。

source-root自身と、選択した相対pathの各directory／leafで検出したsymlink／reparse pointを拒否する。leafは通常fileのみで、read前にresolved pathがresolved root内にあることを確認し、stat／open／fstat／read後の可視変化も検査する。Windows device名を独自の一覧で推測して許可せず、通常fileでない対象は閉じて拒否する。選ばれていないfileは、巨大・不正コード風であっても走査・hashの対象にしない。

1 fileは8 MiBまで、成功した選択file長の合計は32 MiBまで。aggregate上限は次fileを最大8 MiB読んでから検査するため、実I/Oが常に32 MiB以下であるという保証ではない。入力・中間確保、peak／RSS、CPU、共有memory、OS cache全体の予算でもない。

全fileの観測・hashと完成manifestの64 KiB上限を確認してから、指定new-dirを作る。出力はcreate-newの`inventory.json`だけで、write／flush／fsync／closeを終えてからworker receiptを返す。既存のdir（空でも不可）、file、symlinkや親dir不在はsource read前に拒否する。途中write／sync／close失敗では、不完全dirやpartial manifestを保持し、自動削除・上書きを行わない。atomic保存・親dir耐久性・電源断復旧は保証しない。

## manifestと信頼境界

inventory.jsonのexact fieldは`format_version: 1`、`scope: manual_selected_migration_source_inventory`、UTC `recorded_at_utc`、`files`、`unconfirmed`。filesはcallerの選択順に`{path, bytes, sha256}`を持つ。絶対source-rootを自動記録せず、source codeもcopyしないが、相対file名・未確認メモ自体に私情報が含まれる可能性がある。manifestを公開許可済み・secret-freeと扱わない。

個々のfile readで見える変更を拒否し、validated bytesのhashを記録する。全fileを原子的にcollection／snapshotしたものではなく、先に読んだfileの後続in-place変更やABAを必ず検出するものでもない。信頼する安定したsourceと親pathが前提で、敵対的filesystem raceへのsandboxではない。manifestとhashを置き換えられる者に対する署名・真正性の保証も無い。

core APIは`inventory_migration_sources(source_root, selection_file, new_dir, timeout=20)`。検証・file read・hash・保存を所有workerへまとめ、親はcaptured PID、envelope、summaryの型／範囲を検査する。timeoutや不正protocolではcleanup retryを行い、固定errorへ閉じる。deadlineはOS scheduling・filesystem stall・cleanup失敗を含む厳密な終了SLAではなく、未回収workerを成功扱いしない。

## 有限fixture契約

後続の全Python回帰は284実行／280成功／Windows symlink権限4 skip、exit0（`target/migration-source-inventory-python-suite.log`）。既存11証跡checkerも全exit0（`target/source-inventory-current-check-summary.json`）。production Rust／DB／PHP sourceを変えておらず、それらの全再試験ではない。

`python scripts/test_migration_source_inventory.py -v`は、所有temp sourceと独立selectionを作り、実CLI／workerとpublic APIを確認する。選択順、Unicode相対path、opaque binary／PHP風source、未選択fileの除外、原文メモ、summary／manifest hash、sourceとselectionの非更新を対象とする。

拒否・境界契約にはselection schema／型／JSON／path、UTF-8 note長、file／note件数、selection 64 KiBと独立manifest上限、file 8 MiBとaggregate 32 MiBのexact／one-over、既存出力の非上書きとread前拒否、nonregular／欠落、検出link拒否を含む。private write seamでpartial出力保持・worker成功receipt無しも検査する。timeout／PID／summary改変とcleanup retryはprocess無しのmock契約として区別する。

symlink作成権限が無い環境ではその試験だけ明示skipする。新しいツールの導入、外部site、DB、Cargo build、正式proof、proseテンプレートの機械試験は行わない。実行結果は確認後に追記し、文書の作成を全体gateや移行成功へ読み替えない。

2026-10-29のWindows限定結果は`target/migration-source-inventory-contract-tests.log`に記録し、9試験中8成功・symlink作成権限不足1 skip、exit 0。real CLI／workerで256 file・64 note、selection 64 KiB、file 8 MiB、aggregate 32 MiBの境界と、selection内に収まるが完成manifestが64 KiBを超える拒否も確認した。workerをreap後に一時source／manifestを回収した。Linux、一般filesystem、全体Python gate、移植・ライセンスの成功をこの結果へ含めない。
