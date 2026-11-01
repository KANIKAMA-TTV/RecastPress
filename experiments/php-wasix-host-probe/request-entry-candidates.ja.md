# PHP要求入口の比較メモ

2026-11-01。version-onlyのRust直接実行後に必要な、PHPの標準HTTP意味を保つ入口の調査。以下の入口は当Rustホストでは未実行であり、製品方式の決定ではない。PHP sourceはすべて`f4bfae79d5c19028349082f707930cf0951d23b9`に固定する。

| 入口 | 既存部品 | 次に必要な実証 |
| --- | --- | --- |
| CLI server SAPI | 取得済みCLIに[php_cli_server.c](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/sapi/cli/php_cli_server.c)が入り、header・POST・cookieのcallbackを持つ。[公式テスト](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/wasix-test-server.sh)も同じCLIを`-S`、Wasmer LLVMで使用する | 固定HTTPだけを通すpure-memory loopback、要求終了、継続serverの停止・回収。既存CGI parserへHTTP wireを直接渡さない |
| CGI | [configure](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/wasix-configure-eh.sh)は明示的にCGIを無効化するが、後置`WASIX_EXTRA_CONFIGURE_FLAGS=--enable-cgi`が最小変更候補。[CGI出力](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/sapi/cgi/Makefile.frag)は`sapi/cgi/php-cgi` | `make cgi`と[既存build script](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/wasix-build-eh.sh)と同じWasm後処理を適用する新ビルド。標準CGI env・有限stdin・完成後stdoutの契約へ最も直接接続できるが、成立は未確認 |
| libphp/embed | 同公式runに[static archive＋headers](https://api.github.com/repos/wasix-org/php/actions/runs/39649844632/artifacts)がある。[設定](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/wasix-configure-libphp-64.sh)はZTS/static/embed | Wasm guest側のC SAPI shim、リンク・export・メモリ受渡しABIが必要。static archiveをhost Rustへnativeリンクはできない |

embedは[初期化時](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/sapi/embed/php_embed.c)に`headers_sent/no_headers=1`、POST callbackはNULLであり、完成したHTTP request APIではない。Rust側でWPのsuperglobal・header機能を一から模倣する近道にはしない。

pure-memory loopbackの候補は実lockの`virtual-net 0.702.1`にある公開safe API、`LoopbackTcpListener::new/connect_to`と`TcpSocketHalf::split`である。`VirtualNetworking`の`listen_tcp/bind_tcp`だけを固定addrへ許可し、guestのconnectと他の能力を既定Unsupportedへ保つ案。`LocalNetworking`やhost-vnetを全面許可する必要はない。ただしbuffer上限と異常回収は別途確認が必要で、`WasiProcess::terminate`だけでthread終了を保証しない。

CLI serverはHTTPの`Connection: close`等を返す。CGI parserのhop-by-hop拒否を弱めて兼用せず、HTTP transportとして別に検証する。最初のP0として既存artifactのSAPIを試す案と、要求ごとの正常終了に直結するCGI新ビルド案を比較する。どちらもNode.js・native PHPを製品実行依存に追加しない。

## CGIビルドの実験化

[独立CGIビルド](../php-wasix-cgi-build/README.ja.md)を用意した。configure差分は`--enable-cgi`だけとし、固定source・同梱静的ライブラリ・4つの取得ツール・stub用parserのcommitを照合する。ビルド用native PHPは版と実行ファイルSHAを記録する。実物取得・展開・CGIビルドはCIで別に観測し、ビルド成功をPHP要求の実行成功やFiber修正へ置き換えない。
