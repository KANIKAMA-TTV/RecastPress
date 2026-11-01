# v0.5 P0：RustからPHP WASIを直接実行する実験

既存のWAT実験とは別に、実際のPHPインタプリタのWasmをRustで動かす。独立workspaceで実証し、本体の依存や既存仕様書は変更しない。2026-11-01、Windows x86_64で17項目と失敗後の正常要求6回を検証し、[実行証跡](evidence/native.json)へ保存した。製品P0の通過を示すものではない。

## 実行

```text
python experiments/php-wasi-host-probe/prepare_fixture.py
python experiments/php-wasi-host-probe/check.py --record
```

準備時のみネットワークとcurlを使う。PHP成果物は[fixture.json](fixture.json)のURL・サイズ・SHA-256で固定し、Git対象外の`.reference/php-wasi-8.2.6`へ保存する。Rust側でも長さとハッシュを照合してからコンパイルする。検証自体はCargoのoffline・locked・releaseで実行し、Node.js・ネイティブPHP・外部DBを起動しない。Wasmは一度コンパイルし、各要求に新しいStoreを作る。

## 検証するもの

- 本物のPHP 8.2.6による基本処理、CGI入出力、binary本文、Cookie、include。
- 同じ要求内のPHP object共有と、要求ごとのglobal・Cookie・Rust側cache状態の分離。
- PHPが要求した時点でRustを呼び、cache更新・読取りと固定DB fixture結果をPHPへ返す往復。
- 読取り専用の公開fixtureディレクトリだけを渡し、外部ファイル・書込み・親環境の参照を拒む。
- fuel・epoch・Wasmメモリ・応答出力の上限、PHP fatal、未知RPCの失敗と、各失敗後の正常要求。
- 実行が完了する前の出力を成功結果として取り出せないこと。

RPCは、PHPがstdoutへコマンドを出してflushし、Rustが書き出す専用の読取り専用replyファイルをPHPが読む、固定入力用の実証経路である。取得したCGI成果物では`php://stdin`と`php://stderr`を開けなかったため、この実験ではファイル経由を使う。PHPの出力バッファがコマンドを捕捉する構成には使えず、製品のABIへ転用しない。実DB、SQL処理、標準wpdbは未実装で、別テーマ・別プラグインの完全な隔離も提供しない。

## 判定の境界

PHP成果物は[upstreamの2023年リリース](https://github.com/vmware-labs/webassembly-language-runtimes/releases/tag/php/8.2.6%2B20230714-11be424)の古い検証用fixtureであり、製品へ同梱する版ではない。Wasmtime・WASIの版も実験として固定したもので、製品への採用、ライセンス整合、配布許諾を確定しない。取得物の再配布は行わない。

`evidence/native.json`は実行したソース・lock・fixture設定・実行ファイルのハッシュと観測範囲を記録する。`evidence/dependencies.json`は解決された依存のライセンス宣言であり、監査の完了を意味しない。`--record`を使って成功した場合だけ生成する。

メモリ上限はWasm linear memoryに対するもので、Rust・コンパイラ・WASIの割当てやprocess全体のRSSを含まない。epochはguest実行を中断するが、停止しないhost呼出しの監督には別の仕組みが必要となる。この実験は逐次実行であり、同じengineのepochを要求ごとに進めるタイマーは並列workerへ転用できない。Storeの破棄はDB rollbackやRSS返却の証明ではない。

CGIのPHP fatalは終了コード255を使い、この版のWASI側では有効な終了コード範囲外としてエラーになる。これも失敗として回収し、正常応答へ変換しない。製品でのPHPエラー・HTTP status対応は追加の検証が必要となる。

製品用PHPの再現可能なビルド、ABI、依存・配布確認、WP起動、実DB、実HTTP、三配備先は引き続き未達。これらを製品P0〜P1の受入条件から取り除かず、質問待ちで止めずに独立して検証できる項目から進める。
