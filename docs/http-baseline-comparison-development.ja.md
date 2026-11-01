# 保存済みHTTP baselineの構造比較候補

`python scripts/compare_http_baselines.py <left-dir> <right-dir>`は、既存の[手動HTTP baseline](http-baseline-development.ja.md)版1の2組をread-onlyで読み、限定した構造差をJSONで返す実験用CLI。ネットワークへ接続せず、URLを再取得せず、HTML／JavaScriptの実行・画面撮影・比較も行わない。baseline取得側が残した私情報をreportへ転記しない。

```powershell
python scripts/compare_http_baselines.py target/http-baseline-before target/http-baseline-after
```

両入力が検査を通れば、差があってもexit 0・JSON一行・標準エラー出力は空。入力不正、IO失敗、worker timeout等はexit 2、標準出力は空、標準エラー出力は`HTTP baseline comparison failed`と改行だけ。URL・path・header値・body・hashや任意のIO診断をerrorに含めない。完成reportの標準出力への書込み自体が途中で失敗した場合、既に書いたbytesを撤回できず、入力ファイルは保持する。

## reportの意味

reportは`format_version: 1`、`scope: manual_http_baseline_structural_comparison`に加え、次の8個の独立したboolを返す。

| field | 比較対象 |
| --- | --- |
| `body_equal` | それぞれ長さ・SHA-256検証を通った実body bytesの完全一致 |
| `status_equal` | 記録された最終status |
| `headers_equal` | 選択8種類のheader mapping。値listの重複・順序・空値を含む |
| `http_version_equal` | 記録されたHTTP version |
| `request_target_equal` | query spellingと空`?`を含む記録target |
| `url_equal` | 記録URLの文字列一致 |
| `transport_equal` | 記録されたHTTP／HTTPS |
| `tls_verification_equal` | 記録されたTLS検証mode |

`changed_header_names`は差があった選択header名だけを、lowercase・重複無し・昇順で返す。許可名はcontent-type、content-encoding、content-length、cache-control、etag、last-modified、location、varyの固定8個。header mappingのkeyの挿入順は比較せず、各値listの順序は比較する。header不在と値listの差も観測する。

`recorded_at_utc`は入力時に有効なUTC日時として検査するが、比較対象に含めない。reportへ元の日時、URL、target、path、header値、body、hashを出力しない。総合`equal`／`pass`、score、移行合否を作らない。異なるURLやTLS contextでも同じbodyならbody_equalはtrueであり、TLS modeの同値も証明書の再検証・取得先の真正性を意味しない。

bodyは取得時のtransfer framing除去後のentity bytesで、gzip decodeやHTML正規化をしない。文字code、DOM、画像、layout、動的な日時、移行前後の意味が同じかは判定しない。source metadataに記録されたSHA-256はbody identity検査用で、署名ではない。bodyとmetadataを書き換えられる者は整合するbaselineを作れる。

## 入力の限定契約

各dirは固定`body.bin`と`capture.json`の2ファイルだけを持つ。入力dirと固定fileの検出したsymlink／reparse point、通常fileでないleaf、欠落・余分なentryを拒否する。metadataは64 KiB、bodyは8 MiBのsentinel付きreadで上限を検査する。

metadataはUTF-8のstrict JSONで、BOM、重複key、float／NaN、未知field、必須field欠落を拒否する。既存captureの14fieldを要求し、version／scope／method／body_fileは固定値、statusは200～599、http_versionはHTTP/1.0／HTTP/1.1、body_bytesは0～8 MiBの整数としてboolと区別する。body_sha256はlowercase hex 64文字で、実body長・hashを検証する。

URLは取得側と同じ明示ASCII subset、request_targetはその字面から導いたtargetと一致、transportはscheme、tls_verificationはtransportに対応するmodeと整合する必要がある。日時は実在するUTC ISO日時（`Z`または`+00:00`）を要求する。headersは固定lowercase 8名から非空listへのmapping、値は空文字・Latin-1・obs-foldを含む文字列として保持し、勝手にtrimや統合をしない。Content-Lengthがあれば単一ASCII decimalを要求し、通常statusではleading zeroを除いた値がbody_bytesと一致する。204／304のbodyは空を要求するが、その記録Content-Lengthはbody長との一致条件から除く。

この検査はcapture形式の自己整合性であり、実際にその応答を取得した証明、HTTP全体のvalidation、公開許可、秘密情報除去ではない。不正入力を「差がある有効なbaseline」へ変換しない。

## workerとfilesystemの範囲

core APIは`http_baseline_compare.compare_http_baselines(left, right, timeout=20)`。検証・file read・比較を所有workerで行い、親は実captured PIDとworker envelope、reportの厳密なfield／bool／sorted header名とheaders_equalの整合を確認する。timeoutや不正protocolでcleanup retryを行い、失敗を固定errorへ閉じる。OS schedulingやfilesystem stallを含む厳密な終了SLA、敵対的なhostへの隔離は保証しない。

通常fileは個々のread前後にstat／open／fstat等で検査し、見えたlinkや変化を拒否する。比較は検証済みの所有buffer同士で行い、2ファイルや2組のbaselineを原子的にsnapshotしたものではない。metadata fileを読んだ後のin-place変更はdir mtimeを更新しないこともあり、必ず検出できるとはしない。信頼する安定した親pathとfileが前提で、report時点まで元fileが不変であるという認証ではない。入力を上書き・削除せず、比較用artifactや正式proofを新設しない。

## 限定試験

後続の全Python回帰は`target/http-baseline-and-comparison-python-suite.log`で275実行／272成功／symlink権限3 skip、exit0を確認した。既存の11証跡checkerも全exit0（`target/http-tools-final-check-summary.json`）。Rust／DB／PHP sourceを変えておらず、それらの全再実測を行った結果ではない。

`python scripts/test_http_baseline_compare.py -v`は独立したローカルcapture形式fixtureを作り、現CLI／workerを実行する。再取得用ネットワークfixtureは使わない。binary identity／差、UTC時刻の比較除外、headerのkey順と値list順、context差の独立bool、reportの私情報非出力、strict metadata／crossfield／body hash、8 MiB・64 KiBのexact／one-over、不正path／nonregular／引数、検出symlink拒否を確認する。

worker timeoutとPID／report改変拒否・cleanup retryは、有限mock契約として実process試験と区別する。symlinkを作れない環境ではその1試験だけ明示skipする。所有workerをreapしてから一時資料を削除する。

2026-10-29のWindows限定結果は`target/http-baseline-comparison-contract-tests.log`に記録。9試験中8成功・symlink権限不足1 skip、exit 0。取得済みcaptureの14fieldと互換な独立fixtureを使用し、実ネットワークへの重複取得は行っていない。全体Python gate、Linux実行、画面比較や移行合格までをこの結果に含めない。
