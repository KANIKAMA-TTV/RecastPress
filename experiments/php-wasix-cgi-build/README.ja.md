# PHP 8.5.12 WASIX CGIの独立ビルド実験

Rustで直接ホストするCGI候補を作れるか確認する、v0.5の独立P0実験。ここで行うのはビルドと由来の記録までで、生成したguest PHPは実行しない。製品用ランタイム・CGI方式の採用、HTTP互換性、Fiber互換性は確定しない。既存CLI候補のFiber差分を修正したと扱わない。

ビルドを担当する`build.py`の範囲と、後続の固定要求checkerを分ける。workflowはビルド後にRustホストの3要求も試す構成へ進めるが、`build.json`のguest未実行という履歴は変更しない。

## 固定する入力

[manifest.json](manifest.json)にPHP source `f4bfae79d5c19028349082f707930cf0951d23b9`、依存source `b77ecbede97aba2df32c86eaeb54ff9f5e44f543`、PHP-Parser source `f103601b29efebd7ff4a1ca7b3eeea9e3336a2a2`を固定する。GitHub checkoutはcommitを指定し、ビルド開始前にHEADとtracked worktreeを確認する。依存sourceに同梱された`lib-eh`と`pgsql-eh`の静的ライブラリは再ビルドせず、サイズ・SHA-256も記録する。

ホスト用取得物は、wasixcc v0.4.4、WASIX sysroot v2026-07-03.1のlegacy EH版、guest compiler LLVM 21.1.206、Binaryen 130の4点。[prepare.py](prepare.py)が公式release URLから取得し、固定サイズとSHA-256を検証した後だけ展開・使用する。asset IDも記録し、別mirrorや未検証の代替物へ切り替えない。これはRustホスト実行用LLVM 22.1とは別のツールチェーン。

手動配置には`WASIXCC_SYSROOT`、`WASIXCC_LLVM_LOCATION`、`WASIXCC_BINARYEN_LOCATION`を使い、`wasixccenv install-executables`で専用binにwrapperを作る。`download-*`、自動installer、npm/pnpmは呼ばず、HOMEも変更しない。展開先は`target/php-wasix-cgi-build`配下に限定する。既存の展開先は再利用せず失敗するため、CIの新しいworkspaceで準備する。

## 公式手順との差分

[公式configure](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/wasix-configure-eh.sh)を`wasix-configure-eh-64.sh`経由でそのまま使い、末尾の`WASIX_EXTRA_CONFIGURE_FLAGS`へ`--enable-cgi`だけを追加する。CLIのconfigure設定も残すが、ビルド対象は`make -j2 cgi`だけとする。

[初回run 41298369729](https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/41298369729)ではツール検証・configureが成功し、`cgi_main.c:2035`のprefork用`fork()`が未宣言でコンパイルに失敗した。[保存済みの元証跡](evidence/ci-41298369729/provenance.json)は未修正ソースの失敗として維持する。`PHP_FCGI_CHILDREN=0`は実行時の条件なので、このコンパイル対象を除去できない。固定CGI configureにはpreforkだけを無効化する設定もない。

以後は[明示パッチ](patches/0001-wasi-disable-cgi-prefork.patch)で、WASI専用の単一process分岐を追加する。WASIのFastCGI modeで正の`PHP_FCGI_CHILDREN`を指定した場合はエラーを出して失敗し、preforkを利用可能と扱わない。childrenが0なら既存と同じ`parent=0`とsignal初期化へ進む。CGIの単一要求経路、native Unix/Windowsのprefork実装には変更を加えない。fork宣言・成功stub・警告抑制は追加しない。

変更するPHPソースは`sapi/cgi/cgi_main.c`だけ。[patch_source.py](patch_source.py)は固定commitと適用直前のtracked worktree clean確認後に、パッチSHA、元ファイルのサイズ・Git blob ID・SHA-256を照合する。標準の`git apply --check --whitespace=nowarn`で確認してから`git apply --whitespace=nowarn`で適用し、indexは更新しない。各呼出しはshellを使わず30秒の期限を設ける。Gitがoffsetを受理しても、適用後のファイル全体SHA-256が固定値と一致しなければビルドを停止する。再適用も元SHAの不一致で拒否する。manifestと新しいbuild証跡にsource/patch/対象前後の対応と`php_source_patch=true`を記録する。このパッチによる再ビルドと実際のprefork拒否は別途検証が必要。

`sapi/cgi/php-cgi`へリンクした後、[公式optimizer手順](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/wasix-build-eh.sh)と同じflagsで`php-cgi.wasm`を生成する。公式の`--no-validation`も維持するため、Wasmヘッダ検査とハッシュだけでmodule validation済みとは扱わない。CGIのconfigure/link、最終Wasm ABI、Rustでの実行は新規に実証が必要。

## stub生成用のnative PHP

Ubuntuのaptから入れるnative PHPは`gen_stub.php`を動かすビルド用ツール。対象guestの代替としてHTTPやWordPressを実行しない。ビルド時にPHPの版・SAPI・tokenizer有効性・実行ファイルSHA-256とapt package版を記録し、PHP 7.4以上とtokenizerを要求する。OS imageやaptの更新版は完全固定しておらず、bit単位の再現ビルドを証明するものではない。

[固定gen_stub.php](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/build/gen_stub.php)はPHP-Parser 5.6.1が無ければdigestを確認せずarchiveを取得する。この経路を使わず、[5.6.1 tagの参照先commit](https://api.github.com/repos/nikic/PHP-Parser/git/tags/ff81abb785a814e3279bd7a331658792092ad22e)を`build/PHP-Parser-5.6.1`へ先にcheckoutする。ビルド開始前にcommit・必要ファイル・generatorの参照先を確認する。mtimeを細工してstub生成を省略せず、再生成が必要なら固定parserと記録済みnative PHPを使う。`*_arginfo.h`のビルド前後ハッシュと変更一覧も保存する。

## 検証と保存範囲

静的確認は、実験ディレクトリの`prepare.py --validate-only`と`build.py --validate-only`で行える。取得・展開・ビルド・guest実行を行わない。[専用workflow](../../.github/workflows/php-wasix-cgi-build.yml)は当実験・共通Rustホスト・自身の変更をdev/v0.5 pushで起動し、手動起動も定義する。

CIは準備後に`build.py --record`で由来、ツール検証、configure生成物のハッシュ、CGI Wasmのサイズ・SHA-256をこの実験の`evidence/*.json`へ保存する。失敗もJSON証跡に残す。PHP/LLVM等のバイナリ、archive、source tree、ビルド生成物そのものはuploadしない。実行checkerのJSON追加は後述のとおり、builderの記録と区別する。

[run 41300641538](https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/41300641538)では、source `98215a2`の[明示patch付きビルド](evidence/ci-41300641538/provenance.json)が成功した。最終Wasmは40,002,084B、SHA-256 `6fb1d3c02e57aef7fbd9f06b315a93eab1d985453d104aab9147bc34c6837c43`。これは当該jobの生成物hashで、次回buildが同じbyteになるとの宣言ではない。Rustでの要求実行はこのrunには含まれない。

後続workflowでは、別系統のhost LLVM22.1/Rust1.94を準備して[固定CGI checker](../php-wasix-host-probe/cgi_check.py)を同jobで実行する。生成物をuploadして受け渡さず、builderのguest未実行receiptと実行checkerのreceiptを分ける。uploadは両実験の該当JSONだけで、バイナリは含まない。GET/POST等が通っても、WP bootstrap・DB・Fiber・製品採用を合格にしない。

[run 41302992818](https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/41302992818)はsource `7c57fad`でbuildとRust内の3要求・native比較に成功した。[原記録と元source照合](evidence/ci-41302992818/provenance.json)を保存した。今回のWasmは同じ40,002,084BだがSHAは`2879787251cea507843d6ab98c0f0fd4eea5005b5fe74120c8c512541a73ed59`であり、bit単位の再現性は成立確認していない。実行したバイトは同jobのbuilder receiptと前後照合されている。

RPC追加後のsource `e9a2e6c`も[run 41305668059](evidence/ci-41305668059/provenance.json)でbuildと3要求のnative比較が成功した。このrunの実物hashとraw記録を別保存し、以前の結果で代用しない。

source `a0cbade`からはhostをmatrix化し、[Cranelift](evidence/ci-41310853946-cranelift/provenance.json)と[LLVM](evidence/ci-41310853946-llvm/provenance.json)で同じ3ケースがnativeと一致した。CGIのsource/configure/patchは共通、host LLVM archiveはLLVM jobだけに取得する。各job生成物は個別hashで結び付け、両job間のbit一致や再現ビルドは未確認のまま保持する。
