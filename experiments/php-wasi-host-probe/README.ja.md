# v0.5 P0：RustからPHP WASIを直接実行する実験

既存のWAT実験とは別に、実際のPHPインタプリタのWasmをRustで動かす。独立workspaceで実証し、本体の依存や既存仕様書は変更しない。2026-11-01、Windows x86_64で41項目、失敗後の正常要求11回、4件の並列要求、CGI解析の単体テスト9件を検証し、[実行証跡](evidence/native.json)へ保存した。41項目には実HTTPでの4群の検証を含み、PHP要求の期限切れ後のHTTP復帰も別途確認した。製品P0の通過を示すものではない。

## 実行

```text
python scripts/wordpress_reference.py fetch
python experiments/php-wasi-host-probe/prepare_fixture.py
python experiments/php-wasi-host-probe/check.py --record
```

準備時のみネットワークとcurlを使う。PHP成果物は[fixture.json](fixture.json)のURL・サイズ・SHA-256で固定し、Git対象外の`.reference/php-wasi-8.2.6`へ保存する。Rust側でも長さとハッシュを照合してからコンパイルする。検証自体はCargoのoffline・locked・releaseで実行し、Node.js・ネイティブPHP・外部DBを起動しない。Wasmは一度コンパイルし、各要求に新しいStoreを作る。

WordPress 7.1.3の参照ソースは既存の取得手順を使う。[wordpress-fixture.json](wordpress-fixture.json)で、実行する`plugin.php`・`class-wp-hook.php`・`class-wp-filter-sentinel.php`の長さとSHA-256を固定し、Rust側で照合してから読取り専用で渡す。

## 検証するもの

- 本物のPHP 8.2.6による基本処理、CGI入出力、binary本文、Cookie、include。
- 同じ要求内のPHP object共有と、要求ごとのglobal・Cookie・Rust側cache状態の分離。
- WordPress本体のfilter順序と追加引数、actionによるobject共有、nullとsentinel、要求ごとのhook状態の分離。
- PHPが要求した時点でRustを呼び、cache更新・読取りと固定DB fixture結果をPHPへ返す往復。
- 出力バッファ内からのRPC、分割書込み、descriptorの閉鎖・再オープン。通常出力のRPC風文字列とbinaryをそのまま本文に保持する。
- 公開fixtureとWPソースを読取り専用で渡し、外部ファイル・公開ソースへの書込み・親環境の参照を拒む。RPCは要求ごとの専用作業領域を使う。
- fuel・epoch・Wasmメモリ・応答出力の上限、PHP fatal、未知RPCの失敗と、各失敗後の正常要求。
- RPCのサイズ超過・未完了フレーム、不正CGI長・hop-by-hop header・header上限の拒否と復帰。
- PHPのstatus、個別のSet-Cookie、binary本文、redirect、204、304の解析。通常のContent-Lengthは実本文長と照合し、304の長さ情報は検証待ちのmetadataに分ける。
- 4要求でのPHP・WP・Rust状態の隔離、異なる期限の2要求の独立した中断、長要求中の正常要求。
- 実行が完了する前の出力を成功結果として取り出せないこと。
- loopback HTTPでのbinary本文・Cookie・method・URI/queryのPHP到達、201・複数Cookie・binary本文の返却。
- PHPが途中本文を出した実観測後から要求完了までのHTTP無送信、別要求の並行成功、失敗時の固定503応答と、その後のHTTP復帰。

RPCは専用ファイルのdescriptorへの書込みをRustが受け取り、同じファイルを通じて結果を返す固定入力用の実証経路である。公開stdoutは通信に使わない。`path_open`・`fd_write`・`fd_close`を必要な範囲で仲介し、その他の操作は元のWASI実装へ渡す。PHPメモリを共有する小さな[転送module](fixtures/wasi-forwarder.wat)を介し、元のWASIへ正しいcaller memoryを渡す。

RPC領域は最初のpreopen、fd 3に固定し、WASI側で書込み可能な要求専用領域を設ける。受信は最大128B・16 iovec、各フレームは改行を含め最大128B、最大8操作であり、終了時には未完了フレームを拒否してStore・通信ファイル・要求ディレクトリを回収する。これらは試験値で、任意PHPや悪意ある拡張向けの境界、SQL用の通信仕様、製品ABIとして採用しない。実DB、標準wpdb、別テーマ・別プラグインの完全な隔離は未実装。

## 判定の境界

PHP成果物は[upstreamの2023年リリース](https://github.com/vmware-labs/webassembly-language-runtimes/releases/tag/php/8.2.6%2B20230714-11be424)の古い検証用fixtureであり、製品へ同梱する版ではない。Wasmtime・WASIの版も実験として固定したもので、製品への採用、ライセンス整合、配布許諾を確定しない。取得物の再配布は行わない。

`evidence/native.json`は実行したソース・lock・fixture設定・実行ファイルのハッシュと観測範囲を記録する。`evidence/dependencies.json`は解決された依存のライセンス宣言であり、監査の完了を意味しない。`--record`を使って成功した場合だけ生成する。

メモリ上限はWasm linear memoryに対するもので、Rust・コンパイラ・WASIの割当てやprocess全体のRSSを含まない。engineごとに10ms周期の時計を共有し、各Storeが自分のInstant期限を判定する。要求ごとにengine全体を中断する方式は使わない。epochはguest実行を中断するが、停止しないhost呼出しの監督には別の仕組みが必要となる。Storeの破棄はDB rollbackやRSS返却の証明ではない。

CGI parserは完成後の出力だけを扱う。headerは16KiB・64件まで、不正な制御文字・折返し・重複Status・曖昧な長さ等を拒否する。Content-LengthとStatusをtransport用headerから除き、204の本文・長さ指定を拒否する。304のrepresentation長は、HTTP側で照合するまで送信しない。[RFC 9110 §8.6](https://www.rfc-editor.org/rfc/rfc9110.html#section-8.6)に合わせた取扱いであり、HEAD処理はまだ実証していない。

HTTP検証は本体と同版のAxumを使う。`127.0.0.1:0`だけで一時的に待ち受け、既知の3経路とmethodだけを実行する。入力本文は4096B、観測するHTTP応答は64KiBまで。HTTP側はPHPの完了結果を待ってからBodyを作り、失敗時には内部エラーや部分本文を返さない。終了時にserverと全PHP workerをjoinしてruntimeを停止する。本体の公開・管理listenerへの組込み、任意のrouting、HEAD/Range、製品の受入制御は未実装。

CGIのPHP fatalは終了コード255を使い、この版のWASI側では有効な終了コード範囲外としてエラーになる。これも失敗として回収し、正常応答へ変換しない。製品でのPHPエラー・HTTP status対応は追加の検証が必要となる。

製品用PHPの再現可能なビルド、ABI、依存・配布確認、WPの通常bootstrap、実DB、本体HTTPへの統合、三配備先は引き続き未達。今回実行したWPコードはフックAPIの範囲であり、実際のテーマや第三者プラグインの互換性を保証しない。これらを製品P0〜P1の受入条件から取り除かず、質問待ちで止めずに独立して検証できる項目から進める。[製品用runtimeの候補メモ](runtime-candidates.ja.md)に、PHP 8.5.12へ進むための一次情報と未実証事項を整理した。
