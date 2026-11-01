# 固定 ZIP から公開用ファイルを準備する

2026-10-30 の開発記録。`unpack-package <zip> <new-dir>` を main に追加した。対象 Rust８件と command２件、実CLI７件、server Clippyに続き、今回の全体gateもexit0。最終11checkerも全exit0で、直前の [workbench ZIP 書出し](prepared-post-workbench-package-development.ja.md)の完了結果を今回へ付け替えない。

## 明示した ZIP と新規ディレクトリだけを扱う

workbench から保存した `prepared-post-package.zip` を明示し、まだ存在しない出力先を指定する。親ディレクトリは先に用意する。例はリポジトリのルートで実行し、ZIP の path を実際の保存先に合わせる。

```powershell
cargo run -p recastpress-server --bin recastpress -- unpack-package prepared-post-package.zip target/prepared-post-package
cargo run -p recastpress-server --bin recastpress -- check-package target/prepared-post-package/bundle.json target/prepared-post-package/receipt.json
```

成功時の標準出力は `package unpacked` と LF。新規ディレクトリに `bundle.json` と `receipt.json` の２ファイルを保存する。空ディレクトリを含む既存出力先、file、link等は拒否し、上書きしない。失敗時は exit2、標準エラーへ固定の `package unpack failed` と LF を返し、入力内容・path・digest・任意のIO診断を表示しない。標準出力の write／flush 失敗も失敗扱いだが、既に書かれた標準出力の撤回や完成ディレクトリの削除は行わない。

このコマンドは listener、DB、PHP、Wasm、fetch、公開切替を起動しない。`check-package` は [同一bytes確認とstrict admission](package-check-development.ja.md)を改めて行う独立操作である。これらの成功は認証、公開承認、署名、最新版・anti-rollback の保証ではない。

## 受理する ZIP の範囲

[`package_zip.rs`](../crates/recastpress-server/src/package_zip.rs) は workbench が出力する固定２ STORE entry の envelope だけを読む。entry は順に `bundle.json`、`receipt.json`、version20、flags0、圧縮method0、固定DOS日時、extra／comment無し。local header と central directory の名前・CRC・長さ・offset、EOCDの件数・位置・末尾までを照合し、entry bytes のCRCも検査する。名前はhost定数としてのみ書き出し、archive内の任意pathを保存先へ連結しない。

圧縮、暗号化、data descriptor、Zip64、任意file追加、異なるtimestamp等を受け付ける汎用 ZIP 展開器ではない。ZIP field は [PKWARE APPNOTE](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT) の local／central／EOCD、CRC32/IEEE は [RFC 1952 Appendix](https://www.rfc-editor.org/rfc/rfc1952.html#section-8) を参照した限定実装である。一般の ZIP ツールで再保存しただけでも固定 envelope から外れれば拒否する。archive上限は８ MiB＋1,024＋220 bytes、payloadは８ MiB、receiptは1,024 bytes。archiveは上限＋sentinelまで一度読み、完成bufferのsliceを使って検査する。

ZIP envelope を通っても成功とはしない。実 `bundle_receipt::verify_payload` が同じbuffer内のpayload／receiptのSHA256・byte数と既存strict bundle admissionを確認してから、出力先を作る。payloadは既存v1/v2領域をそのまま扱い、UIが現在v1だけを生成することを理由にv2を拒否しない。一致するreceiptを付けても不正なbundleは拒否する。CRCやSHAは出所の認証ではない。

## ファイルと失敗の境界

入力leafは検出できるsymlink／Windows reparse point／nonregularを読取前に拒否する。親pathの競合や悪意ある入替えを防ぐsandbox、完全なTOCTOU対策ではなく、信頼した安定したworkspaceを前提とする。

全archive／receipt／bundleの検査を通過してから `create_dir` と固定２fileの `create_new` へ進み、各fileへ `write_all`、`sync_all`、dropを行う。書込み途中で失敗したディレクトリは調査用に保持し、自動削除しない。そのpathを再利用しても既存出力として拒否する。原子的な２file commit、親ディレクトリのdurability、close失敗のACK、全allocation／RSS／実行時間上限は保証しない。

## 対象確認

main対象Rust８件とcommand２件、server全target Clippyが成功。`scripts/test_unpack_package_cli.py` の実CLI７件も成功した（4.086秒、`target/package-unpack-cli-contracts.log`）。現Cargo JSONから選んだ唯一のserver artifactと、実JS moduleが生成したZIPを使った。

正例の２file bytes一致と `check-package`、v2 payload、既存出力拒否、header／CRC／offset／末尾／path／上限の拒否、valid envelope内のreceipt不一致・不正bundle、正確な引数とopaque errorを確認した。v2のtext本文はvalid UTF-8を使い、一般binary textの受理を誤って期待しない。

別の所有HTTP fixtureでは、実JS ZIP→新CLI→`check-package`→既存sealed public serverのGET／HEAD／304／Host421／query404を確認し、起動後に両入力fileを変更しても受理済み本文とETagが変わらないことを観測した。選択sourceと実binaryの前後SHAが一致し、実PID・reader回収・所有directory削除を `target/package-unpack-native-observation.json` に記録した。これは連続公開や一般的な撤回・file監視の実装ではない。OSのZIP download／保存、Linux native、auth／theme／DB接続は今回も未実行／未追加。

guard付き全体gateはexit0（`target/package-unpack-full-check.log`、session14751は出力回収済み）。Rust620（ignore1）、pure313（62 artifact／features空／normal WP1）、Python426（419成功・skip7）、認証３種actual、fresh alpha MariaDB／MySQLの２mode各62、Clippy・server/WP Linux cross compile・license218・HTTPが成功した。今回全８DBをfresh再測定したとは扱わない。残る６DB modeは既観測を保持し、mode数と証跡JSON file数は区別する。baseline23証跡・依存fileのうちalpha２＋認証３の５fileだけが更新され、残り18fileは同一bytesを保持した（`target/package-unpack-retained-identities.json`）。選択source６file不変も実assertし（`target/package-unpack-source-freeze-identities.json`）、DB fixture directoryは空を確認した。

server path dependencyを持つWasmtime publicationは別の依存検証としてnative６件と所有HTTPを再観測した。初回sandbox10013の診断を保持し、許可された同じ所有fixtureのretryで成功、コード修正はない。`target/package-unpack-wasm-http-observation.json` はproducer38340／server38492、body153／bundle281／receipt117 bytes、post_return完了とcopy後guest memory変更、capture２回／比較８boolすべてtrue・changed header無し、全子／reader回収を記録する。Windows停止exit1は回収時の観測である。公開GET中のWasm実行、任意guest／SDK／本番Linuxの検証へ読み替えない。logは `target/package-unpack-wasm-native.log`、`target/package-unpack-wasm-http.log`、初回診断は `target/package-unpack-wasm-http-first-failure.log`。

最終11checkerは全exit0（`target/package-unpack-current-check-summary.json`）、controller34301も出力回収後exit0。checkpoint `target/package-unpack-checkpoint.json` はdocs freeze後にrootが生成する別操作で、ここでは生成済みと扱わない。
