# WASIX C contextの有限実証

PHP Fiberの移植に入る前に、固定Cプログラムでcontext切替と生存中のC stackを検証するP0実験。PHP・WP・実DBは実行せず、製品runtimeも選定しない。C fixtureとRust実行経路は実装済みで、Linuxでの正実行は専用CI待ちである。

## 確認する動作

main/A/Bの3つの実行状態を作り、mainとA/B各4段の再帰frameにvolatile配列を置く。8周の固定切替でcanary・順序・初期の同時生存9範囲の非重複を確認する。context IDはu64のまま保持し、pointerへ変換しない。

続いてBの再帰frameをsuspendしたままAの4frameを通常unwindし、mainの深いstack書込み後にもBの値が保たれることを要求する。Aの終了済みframeのpointerは参照せず、初期range記録と現在の生存状態を区別する。その後inactive Aのdestroy、Bの追加2周・通常unwind・destroyを確認する。callbackの通常return、ENOTSUP、errnoの差分、stack重複、変数破損は失敗である。

destroyは取消しの予約であり、直後の全資源解放を主張しない。C++例外・任意位置中断・blocking hostcall・PHPのfinally/Fiber適合も別の検証対象である。

## ビルドと実行

```text
python experiments/php-wasix-context-probe/build.py --validate-only
```

Linuxの[専用workflow](../../.github/workflows/php-wasix-context.yml)は、[既存CGIのprepare](../php-wasix-cgi-build/prepare.py)で4資材のサイズ・SHAを照合して準備し、PHP本体やdepsのcheckout/buildをせず、このCだけをcompileする。legacy EH sysrootに合わせたraw出力を、validationを有効にしたBinaryenでformal EH/Asyncifyへ変換する。compile/optは各120秒、raw/final moduleは各64MiB以下で、timeout時は所有process groupを回収する。

```text
python experiments/php-wasix-cgi-build/prepare.py --record
python experiments/php-wasix-context-probe/build.py --record
python experiments/php-wasix-host-probe/context_check.py --record
```

Rustは固定`--context-contract`だけを受け付け、同jobのreceipt・C source・manifest・実Wasmを照合して1回compileする。公開APIによる非同期WasiRunnerを使い、host環境・host directory・networkを渡さず、stdinは閉じ、両出力を各64KiBに制限する。外側checkerは所有hostに60秒の期限を設け、失敗時にも取得できたraw JSON・出力・host errorを保持する。LLVM jobはCビルド後に別系統のhost LLVM22.1を準備し、`--llvm`で同じ条件を実行する。

[manifest](manifest.json)のSDK commitはcontext APIを読んだ参照sourceであり、取得済みsysrootがそのsourceからbuildされたattestationではない。4資材の取得物SHA、参照API source、今回Cのbuild/実行を分けて記録する。バイナリはGitやCI artifactへ再配布しない。
