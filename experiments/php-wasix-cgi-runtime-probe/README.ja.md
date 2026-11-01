# v0.5 CGI runtime比較の固定native基準

結合PHP CGI moduleの次のP0確認用に、通常IO、readonly IO、WordPressのpure hooksを含むnative基準を保存する。native PHPは比較対象であり、製品またはWasm実行の代替にはしない。

## 固定fixture

`fixtures/runtime_cgi_contract.php` は既存 `experiments/php-runtime-candidate-probe/fixtures/runtime_contract.php` の冒頭guardだけをCGI用に置換したもの。`verify_fixture()` は元fixtureのSHA-256と、固定置換後のファイル全体をbyte比較する。basic、exceptions、fiber、wordpress_hooks、scratch_io、readonly_ioの6ケース本文は変更しない。

CGI guardは `PHP_SAPI=cgi-fcgi`、`REQUEST_METHOD=GET`、`QUERY_STRING=rp_runtime_cgi=1`、`argc` が未定義または0であることを要求する。引数は `-n -d fiber.stack_size=2097152 -d register_argc_argv=0`。2MiBは実験の明示設定であり、製品の既定値ではない。

Windows CGIはqueryに `=` がない場合にオプション解析を省略するため、固定queryには `=` を含める。`register_argc_argv=0` により非推奨のqueryからargvへの派生を使わない。警告の抑制設定は追加しない。

## native観測

Windows x64の固定PHP 8.5.12配布ZIPと展開済み78ファイルを実行前後に確認し、`php-cgi.exe` を1回実行する。プロセスの実行期限は10秒。stdout/stderrのEOFと所有プロセスのreapを確認し、出力は終了後にそれぞれ64KiB以下か確認する。プロセスRSS、CPU上限、出力蓄積中のメモリ上限を証明するものではない。

専用の所有ディレクトリに次を作る。

- 読取専用属性を付けた固定CGI fixture。
- 既存WordPress 7.1.3 manifestで検証した `plugin.php`、`class-wp-hook.php`、`class-wp-filter-sentinel.php` の読取専用コピー。
- 空の `ordinary.txt` と、Windows readonly属性を付けた固定内容の `readonly.txt`。

`RP_WP_ROOT` と `RP_SCRATCH` は所有コピーの実パスを指す。公開レシートの環境欄ではその値を明示したプレースホルダーで記録する。PHP出力は元fixtureが定義済みの `/wp-core`、`/scratch`、`/fixture/runtime_contract.php` へのパス置換だけを使用する。結果のcase、型、警告、エラー文言を追加で書き換えない。

実行後はPHPが返した値だけでなく、通常ファイルの最終12byte、readonlyファイルの不変な17byte、コピーした全ソースのSHAとreadonly属性、所有エントリ集合を確認する。終了・reap後に既知ファイルの元属性を戻して削除し、空の所有ディレクトリだけを取り除く。不明なエントリを再帰削除しない。

## 比較とレシート検証

native期待値は既存runtime nativeレシートを固定SHA-256で参照し、期待側のSAPIだけを `cgi-fcgi` にする。実際のCGI観測値にはSAPIの正規化を行わない。6ケース全体とreadonlyの `Permission denied` 警告まで、型も含めて厳密に比較する。readonly属性はWindowsの当該固定操作の基準であり、Unixのmode bitや製品の権限境界を検証したとは扱わない。

`evidence/native-cgi-runtime-contract.json` はbody、CGIヘッダー、完全stdoutのhexとSHA、実ファイル確認結果を保存する。`wordpress_pure_hooks_executed=true` と `full_wordpress_bootstrap_executed=false` を分けて記録する。実DBは使用しない。

```powershell
python experiments/php-wasix-cgi-runtime-probe/native_contract.py --record
```

Linux側では `validate_receipt()` がソース8入力、固定fixture派生、配布物情報、実行期限・cleanup、WPコピー情報、全CGI出力と6ケースの一致を検証する。この関数はnativeパッケージの存在確認やnative PHP実行を行わない。過去のWindows観測に対する検証であり、Linux上のnative実行を主張しない。

Wasm側で `is_writable()`、実際の書込拒否、警告が一致するかは別の実行結果で確認する。不一致を警告削除やerrno文言置換で合格扱いにしない。完全なWP bootstrap、実wpdb/SQL、製品の権限・CPU・メモリ管理はこのnative基準の対象外。


## 同じWasmとRust側の実IO比較

`prepare.py`は、既存manifestに固定したWordPress 7.1.3 archive全体の35,368,461BとSHA-256を確認する。全archiveのパスと型・展開枠を確認し、フック用3ファイルだけを`target/php-wasix-cgi-runtime-probe/wp-core`へ新規コピーする。全WPツリーやhostディレクトリをゲストへ渡さず、PHPも起動しない。取得はCIの期限付きcurlに分け、preparer自体はネットワークを使用しない。

Rustの`--cgi-runtime-contract`は、CGI＋Fiberと同じbuild receipt・最終Wasm・命令検査を使う専用入口である。1回compileしたModuleを3回cloneし、各要求に新しいWASI環境とscratch FSを与える。WP source FSは不変の3ファイル、書込可能なfixtureは`/scratch/ordinary.txt`だけとし、4KiB上限を設ける。readonly fileとnamespaceの変更はFSのopenとhandle両方で拒否する。仮想root全体をreadonly化したとは主張しない。

CGIの正常終了、出力枠、両pipeのEOFを確認してから完成応答を解析し、nativeの全6ケース・header・bodyと比較する。Rust側でもscratch初期値、書込byte数、readonly変更の拒否数、最終内容、scratch handle数を照合する。FS metadataにはUnix modeに相当するフィールドがないため、`is_writable()`の値をFS単体テストから推定しない。PHPからの実際の照会と書込拒否がともにnativeに一致する場合だけ、この固定比較を成功とする。

```sh
python3 experiments/php-wasix-cgi-runtime-probe/prepare.py --record
python3 experiments/php-wasix-cgi-runtime-probe/runtime_check.py --record
# LLVM hostは --llvm を追加
```

[結合候補の同じworkflow](../../.github/workflows/php-wasix-fiber-cgi.yml)で、拡張Fiber6ケースの比較を維持したまま独立stageとして追加する。いずれかの比較が失敗すればCIも失敗のままとする。JSONのみを保存し、PHP・WP source・tool・Wasm binariesはuploadしない。

追加時点ではnative全6ケース、要求専用FSの4 unitを含むWindows30 unit、合成した検証記録の不正16変異の拒否を確認した。最後の検査はcheckerのメタデータ検証であり、このruntime corpusのWasm実行結果ではない。Linuxの実PHP観測は次のCIで別に判定する。
