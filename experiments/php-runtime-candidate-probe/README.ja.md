# v0.5 P0：PHP 8.5.12候補の実物検査と比較基準

製品候補のビルド成果物を固定して構造を調べ、通常PHPの比較基準を作る独立実験。PHPのWasm実行や、製品runtime採用の成功を示すものではない。

## 固定した成果物

[公式成功run 39649844632](https://github.com/wasix-org/php/actions/runs/39649844632)の`php-wasix-64`を取得した。sourceは`f4bfae79d5c19028349082f707930cf0951d23b9`。ZIPのAPI宣言SHA-256と取得バイトを照合し、展開後の`php.wasm`も[artifact.json](artifact.json)で固定する。署名URL・token・バイナリはGitへ保存しない。

- ZIP：10,202,404B、SHA-256 `7596001e48be840a3d0cf4f6ad33b0535c46a44e7537267fe9da908996ff4f39`
- Wasm：39,727,549B、SHA-256 `e64850fde91d98dff0bce2b95ed467591376586469b1f4d8bad5c8759c4edbe4`

CI設定の`WASIXCC_WASM_EXCEPTIONS=legacy`だけから最終バイナリの形式を推測しない。[公式build script](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/wasix-build-eh.sh)は`--emit-exnref`を適用する。実測でもlegacy命令は0、`try_table`は8,331、`throw_ref`は6,176で、標準EH形式だった。

`wasix_32v1`と`wasi::thread-spawn`をimportし、`env::memory`はshared・wasm32・初期403 pages・最大65,536 pagesのimportである。`_start`はexportするがmemoryはexportしない。既存Wasmtime 36のWASIp1ホストへそのまま入れ替えることはできない。Wasm32のメモリ幅からPHP_INT_SIZEを推測しない。

## 再実行

GitHub接続のartifact取得API等で、上記ZIPを`.reference/php-wasix-8.5.12/php-wasix-64.zip`へ取得してから実行する。期限付き取得URLは設定ファイルに残さない。

```text
python experiments/php-runtime-candidate-probe/prepare_fixture.py
python experiments/php-runtime-candidate-probe/check.py --record
python experiments/php-runtime-candidate-probe/native_contract.py --record
```

最初のcheckerはCargoのoffline・lockedでRustの構造検査器を動かす。結果は[evidence/artifact-profile.json](evidence/artifact-profile.json)に記録し、`php_executed=false`・`engine_validated=false`を維持する。

native helperは既存のWindows PHP 8.5.12配布ファイル全件とWPフック3ソースをハッシュ照合してから、基本値・例外・Fiber・WPフック・通常IO・readonly IOの6ケースを観測する。成功基準は[evidence/native-contract.json](evidence/native-contract.json)。通常の制限実行では権限照会と読取り可否に差が出たため、同じfixture・期待値を変えず、必要な実行権限で6ケース成功した観測を保存した。実行環境を混同しない。

native PHPは比較用であり、RustホストからPHPを動かせなかった場合の製品fallbackではない。scratchはworkspace内に限り、所有ファイルのreadonly属性を戻して回収する。

## 次の実行比較

[WASIXホスト実験](../php-wasix-host-probe/README.ja.md)は、この実物をWasmer 7.2.0から実行する。Windows Craneliftでの失敗と、Linux LLVMでの検証を分ける。成功・未実行・失敗を一つの合格表示にまとめず、本体MSRV・既存仕様書・既存WASIp1実験を変更しない。
