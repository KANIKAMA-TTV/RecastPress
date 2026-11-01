# PHP WASIX Fiber CGI 合成候補（v0.5）

既存の Fiber CLI 候補と CGI 候補で使った二つの固定 patch を、独立した PHP 8.5.12 checkout に適用する。目的は、同じ Wasm 内の CGI 要求処理で Zend Fiber を比較できる候補を用意すること。既存の CLI・CGI 候補、その patch、過去の実証記録は変更しない。このビルダー自体はゲスト PHP を実行しない。

`manifest.json` は実行前の設計と入力の固定値を記す。実行結果は `evidence/build.json` に分け、設計の `claims` を成功値に書き換えない。PHP 本体は `wasix-org/php@f4bfae79d5c19028349082f707930cf0951d23b9`、依存 archive は既存 CGI manifest に固定した 28 個、stub parser は PHP-Parser 5.6.1 の固定 commit を使う。

## 手順と範囲

ソースは `target/php-wasix-fiber-cgi-probe/source/php`、依存は同じ親の `php-wasix-deps`、parser は `source/php/build/PHP-Parser-5.6.1` に、CI がそれぞれ指定 commit を checkout する。ビルダーはネットワークからソースや parser を取得しない。4 個のホスト用ツールは既存 `php-wasix-cgi-build/prepare.py` が検証・配置したものだけを継承する。

最初に両 patch の全変更対象の元 bytes・SHA-256・Git blob を確認し、Fiber patch、CGI prefork 除外 patch の順に `git apply --check` と `git apply` を行う。対象ファイル集合は重複しない。変更後も全ファイルの固定値と Zend 公開 Fiber header の不変性を確認する。新しい assembly は既存の固定 fixture と一致する必要がある。Git index は更新しない。

configure は `wasix-configure-eh-64.sh` に `--enable-cgi --enable-rp-wasix-fibers --disable-zts` を追加する。元スクリプトの `-flto` は保持し、検証済み wasixcc の `WASIXCC_COMPILER_POST_FLAGS=-fno-lto` で C/C++/assembly の最後に上書きする。PHP 専用の五つの export は `make -j2 PHP=<host-stub-php> cgi` の環境にだけ渡す。configure の conftest と後段の optimizer には渡さない。

raw 出力は `sapi/cgi/php-cgi`。SDK の shell script 出力を無効化し、Wasm header を確認して `target/php-wasix-fiber-cgi-probe/php-fiber-cgi.raw.wasm` に有限コピーする。成功した Fiber CLI と同様に、Binaryen の検証を有効にし、`-O3` を指定せず formal EH と asyncify を適用する。最終出力は同じディレクトリの `php-fiber-cgi.wasm`。この profile は link map を収集しない。既存 CGI の依存 archive 監査は別の実験である。

Windows でも定義だけを検証できる。

```powershell
python experiments/php-wasix-fiber-cgi-probe/build.py --validate-only
```

Linux x86-64 の独立 checkout と固定ツール配置後に、次を実行する。

```sh
python3 experiments/php-wasix-fiber-cgi-probe/build.py --record
```

native `/usr/bin/php` は `gen_stub.php` 用のビルドツールとして version、tokenizer、実行ファイル hash を記録する。製品の実行 fallback ではない。ビルダーが実行する各コマンドには deadline と実行中の合計 stdout/stderr 16 MiB 上限を設ける。失敗時は所有 process group を停止し leader を reap する。configure / make / optimizer はそれぞれ 600 / 1200 / 300 秒、raw / final Wasm は各 128 MiB 以下に限定する。これらはビルド用の制約であり、ゲストの CPU・RSS 制限の実証ではない。

## 検証 API と記録

`input_hashes()` は新ビルダー・helper・manifest、既存 Fiber の manifest / patch / assembly / PHP fixture、既存 CGI の manifest / prefork patch / ツール準備定義などを返す。既存入力を改変せずに参照する。`verify_build()` は読み取り専用で、同一 job のソース HEAD、依存 archive、適用後 source、生成設定・stub header、コマンド完了、raw / final module、ツールの元 archive と展開済み実行ファイルを再確認する。

build receipt の schema は `recastpress.php-wasix-fiber-cgi-build.v0.5`、variant は `nts-fresh-wasi-zend-fibers-cgi-v1`。成功時も outcome は `fiber_cgi_candidate_built_not_executed`、phase は `build_complete` とする。`source_patches` は Fiber → CGI の二つの記録を持ち、`path` は repository 相対。`php_wasm`、`raw_wasm`、`linked_php_output` は生成物の bytes / SHA-256 を保持する。検証 API は final module と build receipt の SHA-256、両 patch の SHA-256、完全な `input_files` などを返す。

この段階では最終 Wasm の stackless adapter 命令形をビルダーは検査しない。ゲストを実行する側が既存の厳格な postopt inspector を先に通し、native CGI の同じ六つの Fiber ケースと比較する必要がある。比較用 `fixtures/fiber_cgi_contract.php` は元の CLI fixture の入口 guard だけを固定 CGI 要求用に変え、ケース本体と期待値を維持する。差分の完全一致は native oracle helper が別途検証し、manifest とビルド入力集合にも新旧両 fixture の hash を記録する。成功後も全 PHP・WordPress 互換性、継続利用する guest、ZTS、stack の free / reuse、取消しと PHP オブジェクト寿命、quarantine 増加、全資源制限、製品採用、依存ソースとライセンスの全確認を証明したことにはならない。

## CGIのnative参照とRust比較

`native_contract.py --record` は固定Windows PHP 8.5.12の`php-cgi.exe`で、完成したCGI応答とFiber6ケースを観測する。元ZIPと展開済み78ファイルを実行前後で照合し、所有する一時コピー・10秒の外側期限・終了とreap・stdout/stderr EOFを記録する。これはnativeの比較基準であり、Wasm実行や製品fallbackではない。Linux側は`validate_receipt()`で入力・実行条件・完成応答を読み取るだけで、native PHPを起動しない。

CGIではCLI用の`argc === 1`という入口条件をそのまま使えない。新しい`fixtures/fiber_cgi_contract.php`はCGI SAPI、GET、固定query、argv生成なしを要求するguardにだけ変更する。`verify_fixture()`は既存CLI fixtureからこの一箇所を置換したbytesとの完全一致を要求する。6ケースの本文、値、警告と余分な出力の期待値は同じままにする。

固定queryは`rp_fiber_cgi=1`。Windows CGIはqueryに`=`がない場合に起動オプションを処理しないため、空queryをこの比較には使わない。nativeとWasmの両方に`-n -d fiber.stack_size=2097152 -d register_argc_argv=0`を渡し、PHP 8.5でdeprecatedとなったquery由来のargv生成を止める。`error_reporting`を下げる変更は加えない。

native基準ではstatus 200、`X-Powered-By: PHP/8.5.12`と`Content-type: text/html; charset=UTF-8`、3523BのJSON本文を観測した。6ケースはnative CGI上ですべて一致した。これだけで結合Wasmの受入にはしない。

```sh
python3 experiments/php-wasix-fiber-cgi-probe/fiber_cgi_check.py --record
python3 experiments/php-wasix-fiber-cgi-probe/fiber_cgi_check.py --llvm --record
```

Rustの`--fiber-cgi-contract`は引数を追加できない専用入口である。同じjobのbuild receiptと生成物を確認し、最終Wasmに対する既存のstackless adapter命令検査を通してから、一度だけcompileする。固定PHPファイル一つをreadonlyの`/fixture`に渡し、新しいWASI実行環境で3要求を順に動かす。外部ネットワーク・host環境の転送・home mappingは渡さない。各要求の完了、64KiBの出力枠、EOFを確認した後に既存CGI parserで応答を解析し、nativeのheader・body・全6ケースに一致することを求める。

外側のcheckerはRust buildを1200秒、compileを含む3要求を600秒で監督し、所有子processの終了と回収、sourceとmoduleの実行前後の不変性を確認する。これらの期限は製品の要求処理期限・CPU制御の成立を意味しない。

追加時点のローカル検証ではRustの26単体テスト、追加引数3形の実行前拒否、検証記録の正常形と不正14変異の拒否を確認した。最後の記録検証は合成メタデータの検査であり、結合Wasmをビルド・実行した結果ではない。結合候補のLinux実行結果は専用CIとそのreceiptで別に判断する。
