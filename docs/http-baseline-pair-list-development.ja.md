# 明示HTTP baselineペア一覧の比較候補

`compare_http_baseline_pairs.py <pairs.json>`は、利用者が明示した1～32組の保存済みbaselineを、既存の[構造比較](http-baseline-comparison-development.ja.md)で順番に比較する実験用CLI。URLの自動対応付け、glob／再帰探索、HTTP再取得、HTML／JavaScript実行、画面比較、移行合否の判定を追加しない。

例えば`target/pairs.json`へ次を保存する。relative pathはこのmanifestの親である`target`を基準とし、コマンドを起動したcurrent directoryを基準にしない。

```json
{
  "format_version": 1,
  "pairs": [
    {"left": "http-baseline-before", "right": "http-baseline-after"},
    {"left": "other-before", "right": "other-after"}
  ]
}
```

```powershell
python scripts/compare_http_baseline_pairs.py target/pairs.json
```

manifestは64 KiBまでのstrict UTF-8 JSONで、exactな`format_version: 1`と`pairs`だけを受け付ける。各pairはexactな`left`／`right`を持ち、値はnonempty UTF-8文字列、NUL不可。boolを整数へ読み替えず、未知field、重複JSON key、float／NaN、BOM、不正UTF-8／surrogateを拒否する。absolute pathも指定できる。通常のrelative `../`はmanifest親からの指定として扱い、Windowsのrooted／drive-relative等の曖昧なpathは拒否する。これはfilesystemのallowlistやsandboxではなく、利用者が選ぶpathの意味を固定する条件である。

## reportと失敗の意味

入力全体が有効なら、差があってもexit 0・report JSON一行・標準エラー出力は空。top-levelは`format_version: 1`、`scope: manual_http_baseline_pair_list_comparison`、`pairs`の3field。各pairはcaller順の1-based `ordinal`、既存の8個のbool、`changed_header_names`だけを持つ。

boolは`body_equal`、`status_equal`、`headers_equal`、`http_version_equal`、`request_target_equal`、`url_equal`、`transport_equal`、`tls_verification_equal`。bodyは検証済み実bytesを比較し、headerの値listは重複・順序を維持する。`changed_header_names`は既存選択8headerのlowercase名だけを昇順・重複無しで返す。UTC取得時刻は入力として検査するが比較対象ではない。

総合pass／equal、score、URLやpathによる自動並べ替えは行わない。reportには元のpath・URL・target・header値・body・hashを出力しない。bodyの同値を画面の同値やTLSの再検証、移行成功へ読み替えない。各baselineの版1 metadata、byte数／SHA-256、上限、link／通常file等の受入れは既存strict helperを再利用する。

後段pairだけが不正であっても全体を失敗させ、先頭pairの成功reportをpartial stdoutへ返さない。入力・file IO・timeout・protocol等の失敗はexit 2、標準出力は空、標準エラー出力は`HTTP baseline pair comparison failed`と改行だけ。どのprivate pathや値で失敗したかを任意診断へ転記しない。全体成功後のreport配送自体が途中で失敗した場合は、既に出力したbytesを撤回できない。

## workerと上限の範囲

core APIは`compare_http_baseline_pairs(manifest, timeout=20)`。manifest読取りから全pairの比較までを単一の所有workerで行い、既存`_compare_worker`を同process内で直接呼ぶ。pairごとにpublic APIを呼んでnested childを作らない。完成reportだけを送信し、親は実captured PID、strictなtop／pair field、ordinal、bool、sorted header名の整合を検査する。timeoutや不正結果はcleanup retry後に閉じたerrorへ変換する。

各pairのbody 8 MiB／metadata 64 KiBという既存上限に加え、manifestとworker reportはそれぞれ64 KiB、pairは32組まで。成功したbody dataの上限は最大512 MiB相当だが、同じpathの再読取り、metadata、sentinel、失敗readもあるため総I/O上限の保証ではない。body bufferはpairごとに解放し、reportだけを蓄積する。RSS／peak、OS cache、filesystem stall、cleanupを含む厳密な終了SLAを保証する予算ではない。

検証済みの所有buffer同士を比較し、2file、2baseline、全pairを原子的にsnapshotしない。個々のread内で見える変化を検査しても、先に読んだfileの後続in-place変更やABAを必ず検出するものではない。安定したfile・親path・hostが前提。HTTPやAIサービスへの接続は行わないが、filesystemのremote share／mount等の背後のIOはhostの選択に依存する。入力を上書きせず、新しいreport fileや正式proofを自動保存しない。

## 有限fixture契約

要求時Wasmの追加契約との共同全Python回帰は318実行／312成功／symlink権限6 skip、exit0（`target/wasm-request-and-pair-list-python-suite.log`）。最終11証跡checkerも全exit0（`target/request-pair-list-current-check-summary.json`）。Rust／DB／旧PHP sourceは無変更で、それらの全再試験ではない。

`python scripts/test_http_baseline_pair_list.py -v`は、独立した版1baseline fixtureとmanifestだけで実CLI／workerを確認する。equal／different混在のexit 0、caller順とordinal、private値非出力、input不変、manifest親へのrelative解決とcwd非依存、32／33組と64 KiB exact／one-over、schema／型／path／重複JSON、後段不正時のpartial stdout無しを検査する。

private workerから既存comparisonを直接2回呼び、nested process APIが呼ばれないことも検査する。PID／report／ordinal改変とtimeout cleanup retryはprocess無しのmock契約として区別する。symlink manifest拒否は作成権限が無い環境でその試験だけ明示skipする。実行後に限定結果を記録し、文書だけで全Python gate、Linux、画面・移行検証の成功を先取りしない。

2026-10-29のWindows限定結果は`target/http-baseline-pair-list-contract-tests.log`に記録し、8試験中7成功・symlink作成権限不足1 skip、exit 0。実CLI／workerでmixed diff、relative cwd非依存、32組、64 KiB境界、後段不正の全体拒否を確認し、workerをreap後に所有temp fixtureを回収した。network／Rust／DBの重複実行や正式proof更新は行っていない。この結果を全体回帰成功へ読み替えない。
