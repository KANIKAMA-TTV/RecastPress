# v0.5 P0：PHP 8.5.12 WASIXのRustホスト比較

独立workspaceの候補実験。Wasmer 7.2.0とwasmer-wasix 0.702.0を固定し、[公式成果物](../php-runtime-candidate-probe/artifact.json)のサイズ・SHA-256を照合してから、固定した`-n -r`だけを実行する。PHP_VERSION・PHP_INT_SIZE・固定echoのJSONを観測し、例外・WP・DB・製品互換性の成功へ広げない。

## Windowsでの実測

2026-11-01、Rust 1.96.0／Windows x64でCranelift候補のビルドは成功した。実行はmodule compile段階で`cranelift-codegen-0.133.0`のlowering内にindex out of boundsのpanicが起き、exit 101となった。PHPの版を確認するところには到達していない。[失敗証跡](evidence/windows-cranelift.json)を保持し、対応済みとは扱わない。

この実測とは別に、[Wasmer 7.2のMSVC EH実装](https://github.com/wasmerio/wasmer/blob/v7.2.0/lib/vm/src/libcalls/eh/mod.rs)にもpanicのstubが残る。コンパイラのpanicを直せば完全互換になる、という根拠にはしない。

```text
cargo fetch --locked --manifest-path experiments/php-wasix-host-probe/Cargo.toml
python experiments/php-wasix-host-probe/check.py --record
```

checkerはビルド済みartifactをCargoのJSONから選び、所有する子プロセスに外側300秒の期限を設ける。失敗・異常終了・結果なし・期限切れは失敗として記録し、checker自身も非zeroで終わる。hostのcache絶対パスは診断抜粋から除く。

## Linux LLVM比較

[独立workflow](../../.github/workflows/php-wasix-candidate.yml)は、公式と同じWasmer 7.2.0のLLVM backendを使う。host LLVMは22.1系で、PHPを作るwasixcc側のLLVM 21.1.206とは別。[llvm-toolchain.json](llvm-toolchain.json)で取得物のサイズ・SHA-256・版・配置を固定する。

```text
python experiments/php-wasix-host-probe/check.py --llvm --record
```

CIはupstream PHP artifactへの読取アクセスとCargo.lockの取得を先に確認し、その後に大きなLLVMを取得する。Rustは1.94.0、ビルドは2 job、全体は45分まで。必要なtokenはPHP取得stepだけに渡し、外部へ継承・出力しない。uploadするのは証跡だけで、PHP・LLVMバイナリは再配布しない。CI段階ごとの失敗も`ci-stages.json`に残す。

## 実験の制御範囲

host環境・home・host directoryをguestへ渡さず、networkはUnsupportedVirtualNetworking、HTTP clientは無し。stdinは閉じ、stdout/stderrは各64KiBを超える書込みをpipeへ入れる前に拒否する。EOF回収にも期限を設ける。

メモリ・CPU・実行内の期限の制御は未達。外側の所有プロセス期限と、virtual threadingの上限を、悪意あるPHPや全process spawnの制御の証明にしない。`unsafe_code=forbid`を保ち、任意のguest入力・製品listener・実DBへ接続しない。Node.js・native PHPをホスト実行の代替に使わない。

PHP_INT_SIZEは実行値を記録し、保存済みnative基準の8Bと別項目で比較する。名前の`-64`やWasm32という情報だけで合否を決めない。版の基本実行が通った後にも、例外・Fiber・権限照会・WP・要求I/O・RPC・異常回収・全配備先・依存と配布の監査が必要である。
